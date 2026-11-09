//! Reading the metadata stored inside an EPUB file.
//!
//! An EPUB is a ZIP archive with a fixed layout:
//!
//!   * `META-INF/container.xml` — points at the package document,
//!   * `<something>.opf`       — the package document; its `<metadata>` block
//!                               carries Dublin Core fields (title, creator,
//!                               date, language, identifiers...) plus optional
//!                               `<meta>` entries such as the modification date.
//!
//! So reading metadata is: open the ZIP, read container.xml, follow the
//! `rootfile@full-path` attribute to the OPF, then walk its metadata elements.
//! Everything here is blocking file IO and must run off the UI thread.
//!
//! `zip` reads the container, `roxmltree` turns the XML documents into a small
//! read-only tree we can walk (much easier, and safer, than matching tags by
//! hand).

use std::io::{Read, Seek};
use std::path::Path;

use roxmltree::Document;
use zip::ZipArchive;

use super::format::{format_utc, human_size};

/// Metadata pulled out of one EPUB, ready for the details page.
pub struct BookDetails {
    /// Shown as the page headline. Falls back to the file stem.
    pub title: String,
    /// Shown under the headline; `None` for files that name no author.
    pub creator: Option<String>,
    /// Every metadata field found, in document order, as (label, value).
    pub entries: Vec<(String, String)>,
    /// Facts about the file on disk (name, folder, size, modified...).
    pub file: Vec<(String, String)>,
}

/// Reads the metadata of the EPUB at `path`.
///
/// Errors are lowercase clauses meant to complete the sentence "Could not read
/// this EPUB: ...".
pub fn read_epub(path: &Path) -> Result<BookDetails, String> {
    let file = std::fs::File::open(path)
        .map_err(|error| format!("the file could not be opened ({error})"))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| format!("it is not a valid ZIP archive ({error})"))?;

    // The container always lives at this exact path.
    let container = read_entry(&mut archive, "META-INF/container.xml")
        .ok_or_else(|| "it has no META-INF/container.xml entry".to_string())?;

    // Normally container.xml tells us where the package document is. For files
    // that omit it, fall back to any `.opf` in the archive.
    let package = package_path(&container)
        .or_else(|| find_package_document(&mut archive))
        .ok_or_else(|| "it does not name a package (.opf) document".to_string())?;

    let package_xml = read_entry(&mut archive, &package)
        .ok_or_else(|| format!("its package document ({package}) could not be read"))?;

    let entries = parse_metadata(&package_xml)?;

    // The headline needs a title even when the file names none.
    let title = entries
        .iter()
        .find(|(label, _)| label == "Title")
        .map(|(_, value)| value.clone())
        .or_else(|| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "Untitled".to_string());
    let creator = entries
        .iter()
        .find(|(label, _)| matches!(label.as_str(), "Author" | "Creator" | "Contributor"))
        .map(|(_, value)| value.clone());

    Ok(BookDetails {
        title,
        creator,
        entries,
        file: file_facts(path),
    })
}

/// Reads one archive entry as text. EPUB XML parts are UTF-8, but a lossy
/// conversion keeps a badly encoded file readable instead of failing.
fn read_entry<R: Read + Seek>(archive: &mut ZipArchive<R>, name: &str) -> Option<String> {
    if let Some(text) = try_read_entry(archive, name) {
        return Some(text);
    }

    // ZIP entry names are case-sensitive, yet archives built on Windows often
    // disagree with the case used by the container/OPF. Try a tolerant match.
    let wanted = name.to_ascii_lowercase();
    let found = archive
        .file_names()
        .filter_map(|candidate| candidate.ok())
        .find(|candidate| candidate.to_ascii_lowercase() == wanted)?
        // Owned, so the immutable borrow of `archive` ends before the mutable
        // one needed to actually read the entry.
        .into_owned();
    try_read_entry(archive, &found)
}

/// Reads one archive entry, `None` when it is missing or unreadable.
fn try_read_entry<R: Read + Seek>(archive: &mut ZipArchive<R>, name: &str) -> Option<String> {
    let mut entry = archive.by_name(name).ok()?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Extracts the package document path from `META-INF/container.xml`.
fn package_path(container_xml: &str) -> Option<String> {
    let document = Document::parse(container_xml).ok()?;
    document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "rootfile")?
        .attribute("full-path")
        .map(str::to_string)
}

/// Fallback for malformed archives: the first `.opf` we can find, preferring
/// the conventional `content.opf`.
fn find_package_document<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Option<String> {
    // Collect first: `file_names` borrows the archive, `by_name` needs it mutably.
    let names: Vec<String> = archive
        .file_names()
        .filter_map(|name| name.ok())
        .map(|name| name.into_owned())
        .collect();
    names
        .iter()
        .find(|name| name.eq_ignore_ascii_case("content.opf"))
        .or_else(|| {
            names
                .iter()
                .find(|name| name.to_ascii_lowercase().ends_with(".opf"))
        })
        .cloned()
}

/// Pulls every metadata field out of the package document, in document order.
///
/// A package document that is not well-formed XML is an error; a well-formed
/// one without a `<metadata>` block simply has no fields.
fn parse_metadata(package_xml: &str) -> Result<Vec<(String, String)>, String> {
    let document = Document::parse(package_xml)
        .map_err(|error| format!("its package document is not valid XML ({error})"))?;

    let metadata = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "metadata");
    let Some(metadata) = metadata else {
        return Ok(Vec::new());
    };

    let mut entries: Vec<(String, String)> = Vec::new();

    for node in metadata.children().filter(|node| node.is_element()) {
        let name = node.tag_name().name();

        if name.eq_ignore_ascii_case("meta") {
            push_meta_entry(node, &mut entries);
        } else if name.eq_ignore_ascii_case("link") || name.eq_ignore_ascii_case("dc-metadata") {
            // `<link>` points at external records, `<dc-metadata>` is a wrapper
            // some broken tools add; neither carries a displayable value.
        } else {
            push_entry(label_for(name), element_text(node), &mut entries);
        }
    }

    Ok(entries)
}

/// Handles a `<meta>` element, which comes in two shapes:
///
/// * EPUB 2: `<meta name="..." content="..."/>`
/// * EPUB 3: `<meta property="dcterms:modified">2024-01-01T00:00:00Z</meta>`
///
/// `<meta refines="...">` entries only qualify another field (sort keys, title
/// types, ...), so they are skipped rather than shown as standalone rows.
fn push_meta_entry(node: roxmltree::Node, entries: &mut Vec<(String, String)>) {
    let attribute = |name: &str| {
        node.attribute(name)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    };

    if let (Some(name), Some(content)) = (attribute("name"), attribute("content")) {
        push_entry(label_for(name), content.to_string(), entries);
        return;
    }

    if attribute("refines").is_some() {
        return;
    }

    if let Some(property) = attribute("property") {
        push_entry(label_for(property), element_text(node), entries);
    }
}

/// Adds a row unless the value is blank or an identical row already exists
/// (EPUB 3 repeats, say, the same `dc:subject` in several refinements).
fn push_entry(label: String, value: String, entries: &mut Vec<(String, String)>) {
    let value = collapse_whitespace(&value);
    if value.is_empty() {
        return;
    }
    let row = (label, value);
    if !entries.contains(&row) {
        entries.push(row);
    }
}

/// All text below `node`, joined. `Node::text()` alone would stop at the first
/// text node, which loses values that contain inline markup.
fn element_text(node: roxmltree::Node) -> String {
    node.descendants()
        .filter(|child| child.is_text())
        .filter_map(|child| child.text())
        .collect::<String>()
}

/// XML pretty-printing leaves newlines and indentation inside text nodes; show
/// `some   value\n here` as `some value here`.
fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Turns a metadata element or attribute name into a human label:
/// `creator` -> `Author`, `dcterms:modified` -> `Modified`, `belongs-to-collection`
/// -> `Belongs to collection`.
fn label_for(name: &str) -> String {
    // Namespaced names (`dc:title`, `dcterms:modified`) are labelled by their
    // local part; `:` only appears in prefixed attributes/elements.
    let local = name.rsplit(':').next().unwrap_or(name);
    let words = local.replace(['-', '_'], " ");

    match words.to_ascii_lowercase().as_str() {
        "title" => "Title".to_string(),
        "creator" => "Author".to_string(),
        "contributor" => "Contributor".to_string(),
        "date" => "Date".to_string(),
        "publisher" => "Publisher".to_string(),
        "language" => "Language".to_string(),
        "identifier" => "Identifier".to_string(),
        "subject" => "Subject".to_string(),
        "description" => "Description".to_string(),
        "rights" => "Rights".to_string(),
        "source" => "Source".to_string(),
        "type" => "Type".to_string(),
        "format" => "Format".to_string(),
        "relation" => "Relation".to_string(),
        "coverage" => "Coverage".to_string(),
        "modified" => "Last modified".to_string(),
        _ => {
            // Capitalise the first letter, keep the rest as written.
            let mut characters = words.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
                None => String::new(),
            }
        }
    }
}

/// Facts about the file on disk, shown below the EPUB metadata.
fn file_facts(path: &Path) -> Vec<(String, String)> {
    let mut facts = Vec::new();

    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    facts.push(("File name".to_string(), name));

    if let Some(parent) = path.parent() {
        facts.push(("Folder".to_string(), parent.display().to_string()));
    }
    facts.push(("Full path".to_string(), path.display().to_string()));

    if let Ok(metadata) = std::fs::metadata(path) {
        facts.push(("Size".to_string(), human_size(metadata.len())));
        if let Ok(modified) = metadata.modified()
            && let Some(text) = format_utc(modified)
        {
            facts.push(("Modified".to_string(), text));
        }
    }

    facts
}
