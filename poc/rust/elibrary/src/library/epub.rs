//! Reading the metadata stored inside an EPUB file.
//!
//! An EPUB is a ZIP archive with a fixed layout:
//!
//! * `META-INF/container.xml` — points at the package document,
//! * `<something>.opf`       — the package document; its `<metadata>` block
//!   carries Dublin Core fields (title, creator, date, language, identifiers...)
//!   plus optional `<meta>` entries such as the modification date.
//!
//! So reading metadata is: open the ZIP, read container.xml, follow the
//! `rootfile@full-path` attribute to the OPF, then walk its metadata elements.
//! Everything here is blocking file IO and must run off the UI thread.
//!
//! Writing is the same trip in reverse: the OPF is patched as text (so the
//! prefixes, identifiers, language and cover pointers already in it survive),
//! the whole ZIP is rebuilt next to the book, and that copy then replaces the
//! original file.
//!
//! `zip` reads the container, `roxmltree` turns the XML documents into a small
//! read-only tree we can walk (much easier, and safer, than matching tags by
//! hand).

use std::io::{Read, Seek, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};

use roxmltree::{Document, Node};
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::{CompressionMethod, ZipArchive};

/// Namespace of the Dublin Core descriptive fields (`dc:title`, `dc:creator`,
/// `dc:date`...).
const DC_NAMESPACE: &str = "http://purl.org/dc/elements/1.1/";
/// Namespace of the package document's own elements, `<meta>` included.
const OPF_NAMESPACE: &str = "http://www.idpf.org/2007/opf";
/// Id given to the `belongs-to-collection` entry this app writes, so its two
/// refinements can point at it.
const SAGA_ID: &str = "elibrary-saga";

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

    let package = package_document(&mut archive)?;

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

/// Finds the package document of an opened archive.
///
/// The container at `META-INF/container.xml` always lives at that exact path
/// and names the package; for files that omit it, any `.opf` will do.
fn package_document<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<String, String> {
    let container = read_entry(archive, "META-INF/container.xml")
        .ok_or_else(|| "it has no META-INF/container.xml entry".to_string())?;
    package_path(&container)
        .or_else(|| find_package_document(archive))
        .ok_or_else(|| "it does not name a package (.opf) document".to_string())
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

/// The metadata that replaces an EPUB's own: what the details page shows and
/// what [`write_metadata`] puts into the package document.
///
/// The saga and its tome are a pair: a volume number only makes sense when the
/// book belongs to a collection.
pub struct EpubMetadata {
    /// Book title.
    pub title: String,
    /// Author, when one is known.
    pub author: Option<String>,
    /// Publication year.
    pub published: Option<String>,
    /// The collection the volume belongs to.
    pub saga: Option<String>,
    /// The volume number inside that collection.
    pub tome: Option<String>,
}

/// Rewrites the descriptive metadata of the EPUB at `path`.
///
/// Everything else in the file is copied through untouched: the identifiers,
/// language, cover pointer and reading order are not ours to judge. The new
/// archive is built next to the book and only then renamed over it, so a
/// failure leaves the original readable.
///
/// Errors are lowercase clauses meant to complete the sentence "Could not
/// update this EPUB: ...".
pub fn write_metadata(path: &Path, metadata: &EpubMetadata) -> Result<(), String> {
    let mut archive = ZipArchive::new(
        std::fs::File::open(path)
            .map_err(|error| format!("the file could not be opened ({error})"))?,
    )
    .map_err(|error| format!("it is not a valid ZIP archive ({error})"))?;

    let package = package_document(&mut archive)?;
    let package_xml = read_entry(&mut archive, &package)
        .ok_or_else(|| format!("its package document ({package}) could not be read"))?;
    let patched = patch_metadata(&package_xml, metadata)?;

    let temp = temp_path(path);
    if let Err(error) = rebuild_archive(&mut archive, &package, patched.as_bytes(), &temp) {
        // Never leave a half-written archive behind.
        let _ = std::fs::remove_file(&temp);
        return Err(error);
    }

    // Close the original before swapping the files: Windows is happier when
    // nothing holds the file we are replacing.
    drop(archive);

    std::fs::rename(&temp, path).map_err(|error| {
        let _ = std::fs::remove_file(&temp);
        format!("the rewritten copy could not replace the original ({error})")
    })
}

/// The package document with its `<metadata>` block rewritten.
///
/// Editing the text instead of generating a new block keeps everything we were
/// not asked to change: element prefixes, identifiers, language, the cover
/// pointer, and even the Dublin Core attributes (`opf:role`, `xml:lang`...).
fn patch_metadata(xml: &str, metadata: &EpubMetadata) -> Result<String, String> {
    let document = Document::parse(xml)
        .map_err(|error| format!("its package document is not valid XML ({error})"))?;

    let block = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "metadata")
        .ok_or_else(|| "its package document has no <metadata> block".to_string())?;

    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    // Elements the file does not have yet; they are added at the end of the
    // block, so the original formatting stays where it is.
    let mut added: Vec<String> = Vec::new();

    // Title, author and year: the first element of each kind is rewritten in
    // place and any duplicate is dropped, so the file keeps exactly one.
    for (local, value) in [
        ("title", Some(metadata.title.as_str())),
        ("creator", metadata.author.as_deref()),
        ("date", metadata.published.as_deref()),
    ] {
        let found = dc_elements(block, local);
        match (found.first(), value) {
            (Some(first), Some(value)) => {
                edits.push(text_edit(xml, *first, value));
                edits.extend(found[1..].iter().map(|node| (node.range(), String::new())));
            }
            // Without a new value the old one has to go: the catalogue did not
            // confirm it, and the whole point is to erase what was there.
            (Some(_), None) => {
                edits.extend(found.iter().map(|node| (node.range(), String::new())));
            }
            (None, Some(value)) => added.push(new_dc_element(local, value)),
            (None, None) => {}
        }
    }

    // A saga is an EPUB 3 `belongs-to-collection` meta plus refinements saying
    // it is a series and which volume this is. Previous collection claims are
    // dropped, including the metas that refine them.
    let collection_ids: Vec<String> = block
        .children()
        .filter(|node| {
            is_meta(node) && node.attribute("property") == Some("belongs-to-collection")
        })
        .filter_map(|node| node.attribute("id").map(str::to_string))
        .collect();

    for node in block.children().filter(is_meta) {
        let property = node.attribute("property").unwrap_or_default();
        let refines = node.attribute("refines").unwrap_or_default();
        let is_collection = matches!(property, "belongs-to-collection" | "collection-type" | "group-position");
        let refines_collection = collection_ids
            .iter()
            .any(|id| refines.strip_prefix('#') == Some(id.as_str()));
        if is_collection || refines_collection {
            edits.push((node.range(), String::new()));
        }
    }

    if let Some(saga) = &metadata.saga {
        added.push(format!(
            "<meta xmlns=\"{OPF_NAMESPACE}\" property=\"belongs-to-collection\" id=\"{SAGA_ID}\">{}</meta>",
            escape_xml(saga)
        ));
        added.push(format!(
            "<meta xmlns=\"{OPF_NAMESPACE}\" refines=\"#{SAGA_ID}\" property=\"collection-type\">series</meta>"
        ));
        if let Some(tome) = &metadata.tome {
            added.push(format!(
                "<meta xmlns=\"{OPF_NAMESPACE}\" refines=\"#{SAGA_ID}\" property=\"group-position\">{}</meta>",
                escape_xml(tome)
            ));
        }
    }

    if !added.is_empty() {
        let span = content_span(xml, block)
            .ok_or_else(|| "its <metadata> block is written in a form this app cannot rewrite".to_string())?;
        edits.push((
            span.end..span.end,
            format!("\n    {}\n  ", added.join("\n    ")),
        ));
    }

    // Apply from the end so the offsets of earlier edits stay valid. Sorting is
    // stable and ties are impossible: no edit starts where another begins.
    edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut patched = xml.to_string();
    for (range, text) in edits {
        patched.replace_range(range, &text);
    }

    // A file whose XML no longer parses is worse than an unchanged one, so this
    // is checked before anything is written to disk.
    Document::parse(&patched)
        .map_err(|error| format!("its rewritten package document is not valid XML ({error})"))?;
    Ok(patched)
}

/// Elements of the `<metadata>` block in the Dublin Core namespace with the
/// given local name (`title`, `creator`, `date`...).
fn dc_elements<'a, 'input: 'a>(block: Node<'a, 'input>, local: &str) -> Vec<Node<'a, 'input>> {
    block
        .children()
        .filter(|node| {
            node.is_element()
                && node.tag_name().name() == local
                && node.tag_name().namespace() == Some(DC_NAMESPACE)
        })
        .collect()
}

/// Whether `node` is a `<meta>` element, whatever prefix the file gives it.
fn is_meta(node: &Node) -> bool {
    node.is_element() && node.tag_name().name() == "meta"
}

/// The edit that puts `value` inside `node` as its only text.
///
/// A normal element only has its content replaced, so attributes stay. A
/// self-closing one (`<dc:title/>`) has no content to replace, so the whole
/// tag is rebuilt, including the prefix the file chose for it.
fn text_edit(xml: &str, node: Node, value: &str) -> (Range<usize>, String) {
    let escaped = escape_xml(value);
    match content_span(xml, node) {
        Some(span) => (span, escaped),
        None => {
            let name = qname(xml, node);
            (node.range(), format!("<{name}>{escaped}</{name}>"))
        }
    }
}

/// The byte range between an element's opening and closing tags.
///
/// `None` when the element closes itself: it has no "inside", so the caller
/// has to rebuild the tag instead of filling it.
fn content_span(xml: &str, node: Node) -> Option<Range<usize>> {
    let range = node.range();
    let open_end = open_tag_end(xml, range.start)?;
    // `<dc:title/>` or `<dc:title />`: the tag ends with a slash.
    if xml[range.start..open_end]
        .trim_end_matches('>')
        .trim_end()
        .ends_with('/')
    {
        return None;
    }
    let close_start = xml[open_end..range.end].rfind('<')? + open_end;
    Some(open_end..close_start)
}

/// Index just past the `>` that closes the tag starting at `start`, skipping a
/// `>` that appears inside a quoted attribute value.
fn open_tag_end(xml: &str, start: usize) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (offset, byte) in xml.as_bytes()[start..].iter().copied().enumerate() {
        match byte {
            b'"' | b'\'' if quote.is_none() => quote = Some(byte),
            b'"' | b'\'' if quote == Some(byte) => quote = None,
            b'>' if quote.is_none() => return Some(start + offset + 1),
            _ => {}
        }
    }
    None
}

/// The qualified name of an element as the file writes it (`dc:title`).
fn qname(xml: &str, node: Node) -> String {
    let start = node.range().start + 1;
    let rest = &xml[start..node.range().end];
    let length = rest
        .find(|character: char| character.is_whitespace() || character == '/' || character == '>')
        .unwrap_or(rest.len());
    rest[..length].to_string()
}

/// A Dublin Core element for a file that has none yet. It declares the `dc`
/// prefix on itself, so it does not matter what prefixes the package uses.
fn new_dc_element(local: &str, value: &str) -> String {
    format!(
        "<dc:{local} xmlns:dc=\"{DC_NAMESPACE}\">{}</dc:{local}>",
        escape_xml(value)
    )
}

/// Escapes the characters that may not appear as plain text in XML.
fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// A sibling path for the rewritten archive: the final rename stays on the same
/// filesystem, which is what makes it atomic.
fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".elibrary-tmp");
    path.with_file_name(name)
}

/// Copies every entry of `archive` into a new ZIP at `target`, replacing the
/// package document with `package_xml`.
///
/// Entries keep their order and compression, so the `mimetype` entry stays
/// first and uncompressed as the EPUB format requires.
fn rebuild_archive<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    package: &str,
    package_xml: &[u8],
    target: &Path,
) -> Result<(), String> {
    let file = std::fs::File::create(target)
        .map_err(|error| format!("the rewritten copy could not be created ({error})"))?;
    let mut writer = ZipWriter::new(file);

    let count = archive.len();
    for index in 0..count {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("one of its entries could not be read ({error})"))?;
        let name = entry
            .name()
            .map_err(|error| format!("one of its entries could not be read ({error})"))?
            .to_string();
        let compression = match entry.compression() {
            CompressionMethod::Stored => CompressionMethod::Stored,
            _ => CompressionMethod::Deflated,
        };
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|error| format!("one of its entries could not be read ({error})"))?;
        drop(entry);

        let options = SimpleFileOptions::default().compression_method(compression);
        if name.ends_with('/') {
            writer
                .add_directory(name, options)
                .map_err(|error| format!("the rewritten copy could not be written ({error})"))?;
            continue;
        }

        let data = if name == package {
            package_xml
        } else {
            bytes.as_slice()
        };
        writer
            .start_file(name, options)
            .map_err(|error| format!("the rewritten copy could not be written ({error})"))?;
        writer
            .write_all(data)
            .map_err(|error| format!("the rewritten copy could not be written ({error})"))?;
    }

    writer
        .finish()
        .map_err(|error| format!("the rewritten copy could not be completed ({error})"))?;
    Ok(())
}
