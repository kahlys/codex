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
//! Writing is the same trip in reverse: the OPF's `<metadata>` block is rebuilt
//! from scratch — everything the book used to claim is dropped, and only the new
//! fields plus the few entries an EPUB cannot do without (identifier, language,
//! cover pointer, modification date) are written back — then the whole ZIP is
//! rebuilt next to the book, and that copy replaces the original file.
//!
//! `zip` reads the container, `roxmltree` turns the XML documents into a small
//! read-only tree we can walk (much easier, and safer, than matching tags by
//! hand).

use std::io::{Read, Seek, Write};
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

use super::format::{format_utc, human_size, now_timestamp};

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
/// The whole `<metadata>` block is replaced, not patched: the point of this
/// operation is that the file ends up describing the book the way the catalogue
/// does, with nothing left of what a previous tool claimed. Only the entries an
/// EPUB cannot work without are carried over — the identifier the package calls
/// unique, the language and the cover pointer — plus a fresh modification date.
///
/// Everything outside that block is copied through untouched: the manifest, the
/// spine and the reading order are not ours to judge. The new archive is built
/// next to the book and only then renamed over it, so a failure leaves the
/// original readable.
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
    let patched = rewrite_metadata(&package_xml, metadata)?;

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

/// The package document with its `<metadata>` block rebuilt from scratch.
///
/// Patching the old values in place leaves everything this app knows nothing
/// about — descriptions, subjects, publishers, accessibility statements, the
/// bookkeeping blobs of whatever tool made the file — so a reader would still
/// be shown the metadata of that tool. Rebuilding the block is the only way to
/// really replace it.
///
/// The entries the format itself requires are carried over as the file wrote
/// them rather than invented: the identifier `package/@unique-identifier`
/// points at, the language, and the EPUB 2 cover pointer.
fn rewrite_metadata(xml: &str, metadata: &EpubMetadata) -> Result<String, String> {
    // Dublin Core requires a title, and there is nothing sensible to fall back
    // on here: a file written without one would be invalid.
    let title = collapse_whitespace(&metadata.title);
    if title.is_empty() {
        return Err("the new metadata does not have a title".to_string());
    }

    let document = Document::parse(xml)
        .map_err(|error| format!("its package document is not valid XML ({error})"))?;

    let package = document.root_element();

    // EPUB 2 packages are written in the older OPF vocabulary, where
    // `<meta property="...">` does not exist at all. Their series has to be
    // expressed with the `name`/`content` attributes Calibre introduced, which
    // is also what every reader of those files understands.
    let epub2 = package
        .attribute("version")
        .is_some_and(|version| version.starts_with('2'));

    let block = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "metadata")
        .ok_or_else(|| "its package document has no <metadata> block".to_string())?;

    // The book's identity. Its element is copied verbatim, so the scheme, the
    // id and the value the file already uses survive untouched.
    let unique = package.attribute("unique-identifier").unwrap_or_default();
    let identifiers = dc_elements(block, "identifier");
    let identifier = identifiers
        .iter()
        .find(|node| node.attribute("id") == Some(unique))
        // A file that names no unique identifier is already broken, but its
        // first identifier is still a better identity than none at all.
        .or_else(|| identifiers.first())
        .map(|node| raw(xml, *node));

    // `<dc:language>` is the other element the format requires.
    let language = dc_elements(block, "language")
        .first()
        .map(|node| raw(xml, *node));

    // `<meta name="cover">` is how EPUB 2 tells a reader which manifest item
    // is the cover; EPUB 3 files usually keep it for the same reason. It points
    // at a resource, so it is structure rather than description.
    let cover = block
        .children()
        .filter(is_meta)
        .find(|node| node.attribute("name") == Some("cover"))
        .map(|node| raw(xml, node));

    let mut elements: Vec<String> = Vec::new();
    elements.extend(identifier);
    elements.push(new_dc_element("title", &title));
    if let Some(author) = &metadata.author {
        elements.push(new_dc_element("creator", author));
    }
    elements.extend(language);
    if let Some(published) = &metadata.published {
        elements.push(new_dc_element("date", published));
    }
    if let Some(saga) = &metadata.saga {
        elements.extend(collection_elements(saga, metadata.tome.as_deref(), epub2));
    }
    if !epub2 {
        // EPUB 3 requires exactly one of these, always in this exact form.
        elements.push(format!(
            "<meta xmlns=\"{OPF_NAMESPACE}\" property=\"dcterms:modified\">{}</meta>",
            now_timestamp()
        ));
    }
    elements.extend(cover);

    // Build the replacement around the block's own opening tag, so its
    // namespace declarations (and any attribute on it) stay in force.
    let start = block.range().start;
    let open_end = open_tag_end(xml, start)
        .ok_or_else(|| "its <metadata> block is written in a form this app cannot rewrite".to_string())?;
    let name = qname(xml, block);
    let indent = indentation(xml, start);
    let mut rebuilt = start_tag_with_dc(&xml[start..open_end]);
    if rebuilt.ends_with("/>") {
        // A self-closing `<metadata/>` has no content to replace: give it some.
        rebuilt.truncate(rebuilt.len() - 2);
        rebuilt.push('>');
    }

    let inner = format!("{indent}  ");
    for element in elements {
        rebuilt.push('\n');
        rebuilt.push_str(&inner);
        rebuilt.push_str(&element);
    }
    rebuilt.push('\n');
    rebuilt.push_str(&indent);
    rebuilt.push_str(&format!("</{name}>"));

    let mut patched = xml.to_string();
    patched.replace_range(block.range(), &rebuilt);

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

/// The source text of one element, exactly as the file wrote it.
fn raw(xml: &str, node: Node) -> String {
    xml[node.range()].trim().to_string()
}

/// The whitespace that indents the line the element starting at `offset` sits
/// on, so the rebuilt block lines up with the rest of the file. Empty when the
/// tag does not start a line.
fn indentation(xml: &str, offset: usize) -> String {
    let line_start = xml[..offset].rfind('\n').map_or(0, |index| index + 1);
    xml[line_start..offset]
        .chars()
        .take_while(|character| character.is_whitespace())
        .collect()
}

/// The `<metadata ...>` opening tag with `xmlns:dc` added when the file only
/// declares that prefix further up the document.
///
/// The Dublin Core elements written here use the `dc` prefix, and the spec
/// requires a prefix to be declared before it can be used.
fn start_tag_with_dc(opening: &str) -> String {
    if opening.contains("xmlns:dc=") {
        return opening.to_string();
    }

    let Some(close) = opening.rfind('>') else {
        return opening.to_string();
    };
    let body = opening[..close].trim_end();
    let (body, slash) = match body.strip_suffix('/') {
        Some(body) => (body.trim_end(), "/"),
        None => (body, ""),
    };
    format!("{body} xmlns:dc=\"{DC_NAMESPACE}\"{slash}>")
}

/// The saga entries for one package version.
///
/// EPUB 3 has a vocabulary for this: `belongs-to-collection` and the
/// `collection-type` / `group-position` refinements that qualify it. EPUB 2 has
/// no such thing — a `<meta property="...">` there is not valid OPF at all —
/// so its files get the Calibre `name`/`content` spelling instead.
fn collection_elements(saga: &str, tome: Option<&str>, epub2: bool) -> Vec<String> {
    if epub2 {
        let mut elements = vec![format!(
            "<meta xmlns=\"{OPF_NAMESPACE}\" name=\"calibre:series\" content=\"{}\"/>",
            escape_xml(saga)
        )];
        if let Some(tome) = tome {
            elements.push(format!(
                "<meta xmlns=\"{OPF_NAMESPACE}\" name=\"calibre:series_index\" content=\"{}\"/>",
                escape_xml(tome)
            ));
        }
        return elements;
    }

    let mut elements = vec![format!(
        "<meta xmlns=\"{OPF_NAMESPACE}\" property=\"belongs-to-collection\" id=\"{SAGA_ID}\">{}</meta>",
        escape_xml(saga)
    )];
    elements.push(format!(
        "<meta xmlns=\"{OPF_NAMESPACE}\" refines=\"#{SAGA_ID}\" property=\"collection-type\">series</meta>"
    ));
    if let Some(tome) = tome {
        elements.push(format!(
            "<meta xmlns=\"{OPF_NAMESPACE}\" refines=\"#{SAGA_ID}\" property=\"group-position\">{}</meta>",
            escape_xml(tome)
        ));
    }
    elements
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

/// A Dublin Core element, using the `dc` prefix the rebuilt block declares.
fn new_dc_element(local: &str, value: &str) -> String {
    format!("<dc:{local}>{}</dc:{local}>", escape_xml(value))
}

/// Escapes the characters that may not appear as plain text in XML, quotes
/// included, because the same values are also written into attributes.
fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The books of the project's `epub/` folder, used as golden data.
    ///
    /// They are only ever read: every test works on a copy, so the folder keeps
    /// the original metadata that makes it worth testing against.
    fn golden_books() -> Vec<PathBuf> {
        let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("epub");
        let mut books: Vec<PathBuf> = std::fs::read_dir(folder)
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| {
                        path.extension()
                            .is_some_and(|extension| extension.eq_ignore_ascii_case("epub"))
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Non-ASCII titles make the default byte order of little use; path
        // order is all this needs, and it is stable between runs.
        books.sort_by_key(|path| path.to_string_lossy().to_lowercase());
        books
    }

    /// The package document of `path`, read back from the archive on disk.
    fn package_of(path: &Path) -> String {
        let mut archive = ZipArchive::new(std::fs::File::open(path).expect("the copy is missing"))
            .expect("the copy is not a ZIP archive");
        let package = package_document(&mut archive).expect("the copy has no package document");
        read_entry(&mut archive, &package).expect("the package document could not be read")
    }

    /// The `<metadata>` element of a package document, as text.
    fn metadata_of(xml: &str) -> String {
        let document = Document::parse(xml).expect("the rewritten package document does not parse");
        let block = document
            .descendants()
            .find(|node| node.is_element() && node.tag_name().name() == "metadata")
            .expect("the rewritten package document has no <metadata> block");
        xml[block.range()].to_string()
    }

    /// Rewrites every golden book and checks that the new metadata is all that
    /// is left afterwards.
    ///
    /// The second half is the point of the test: the old metadata is not left
    /// behind because the new values were written next to it, it is erased.
    #[test]
    fn rewriting_erases_the_metadata_of_every_golden_book() {
        let books = golden_books();
        assert!(
            !books.is_empty(),
            "no .epub file found in the epub/ folder of the project"
        );

        let metadata = EpubMetadata {
            title: "Un titre de test".to_string(),
            author: Some("Test, Auteur".to_string()),
            published: Some("2024".to_string()),
            saga: Some("Saga de test".to_string()),
            tome: Some("2".to_string()),
        };

        let work = std::env::temp_dir().join(format!("elibrary-epub-{}", std::process::id()));
        std::fs::create_dir_all(&work).expect("the temporary folder could not be created");

        for (index, original) in books.iter().enumerate() {
            let name = original
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();

            // Every run starts from the file as it is in the repository, so a
            // previous run cannot make this one pass.
            let before = std::fs::read(original).expect("a golden book could not be read");
            let copy = work.join(format!("book-{index}.epub"));
            std::fs::write(&copy, &before).expect("a golden book could not be copied");

            write_metadata(&copy, &metadata)
                .unwrap_or_else(|error| panic!("{name}: the copy could not be rewritten: {error}"));

            let after = std::fs::read(original).expect("a golden book could not be read again");
            assert_eq!(before, after, "{name}: the golden file itself was modified");

            let block = metadata_of(&package_of(&copy));

            // What was asked for is there...
            assert!(
                block.contains("<dc:title>Un titre de test</dc:title>"),
                "{name}: no new title"
            );
            assert!(block.contains("Test, Auteur"), "{name}: no new author");
            assert!(block.contains("<dc:date>2024</dc:date>"), "{name}: no new date");
            assert!(block.contains("Saga de test"), "{name}: no new saga");
            assert!(
                block.contains("dc:identifier"),
                "{name}: the identifier was dropped"
            );
            assert!(block.contains("language"), "{name}: the language was dropped");

            // ... and nothing the file used to claim.
            for leftover in [
                "<dc:description",
                "<dc:subject",
                "<dc:publisher",
                "<dc:source",
                "<dc:contributor",
                "calibre:user_metadata",
                "calibre:title_sort",
                "calibre:timestamp",
                "schema:access",
                "property=\"role\"",
                "title-type",
            ] {
                assert!(
                    !block.contains(leftover),
                    "{name}: {leftover} survived the rewrite"
                );
            }

            // The archive is still an EPUB: `mimetype` comes first and
            // uncompressed.
            let mut archive =
                ZipArchive::new(std::fs::File::open(&copy).expect("the copy is missing"))
                    .expect("the copy is not a ZIP archive");
            let first = archive.by_index(0).expect("the copy has no entries");
            assert_eq!(
                first.name().expect("an entry has no name"),
                "mimetype",
                "{name}: mimetype is no longer the first entry"
            );
            assert_eq!(
                first.compression(),
                CompressionMethod::Stored,
                "{name}: mimetype is compressed"
            );
            drop(first);
            drop(archive);

            // Reading it back is what the details page does, and it must show
            // the new values only.
            let details = read_epub(&copy).expect("the rewritten copy cannot be read back");
            assert_eq!(
                details.title, "Un titre de test",
                "{name}: wrong title read back"
            );
            assert_eq!(
                details.creator.as_deref(),
                Some("Test, Auteur"),
                "{name}: wrong author read back"
            );
            for (label, _) in &details.entries {
                assert!(
                    matches!(
                        label.as_str(),
                        "Identifier"
                            | "Title"
                            | "Author"
                            | "Language"
                            | "Date"
                            | "Belongs to collection"
                            | "Last modified"
                            | "Cover"
                            // EPUB 2 files get the Calibre series spelling.
                            | "Series"
                            | "Series index"
                    ),
                    "{name}: unexpected metadata row {label}"
                );
            }

            let _ = std::fs::remove_file(&copy);
        }

        let _ = std::fs::remove_dir_all(&work);
    }
}
