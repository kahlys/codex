//! Looking a book up in a real catalogue: the Bibliothèque nationale de France.
//!
//! French EPUBs are often badly tagged, so the metadata panel has a button that
//! searches the BnF's online catalogue for the book and replaces the metadata
//! read from the file with what the national library says about it: title,
//! author, publication year, and the saga the volume belongs to.
//!
//! The catalogue is exposed as an **SRU** service, the standard HTTP + CQL way
//! of querying library catalogues:
//!
//! ```text
//! GET https://catalogue.bnf.fr/api/SRU?version=1.2&operation=searchRetrieve
//!     &recordSchema=unimarcxchange&maximumRecords=5&query=bib.title all "..."
//! ```
//!
//! It answers with XML: a `<srw:searchRetrieveResponse>` holding the total
//! number of matches plus a list of records. We ask for the `unimarcxchange`
//! schema rather than the simpler Dublin Core one because only MARC carries the
//! *collection* a volume belongs to, which is what the page calls the saga.
//!
//! A UNIMARC record is a list of numbered fields, each with lettered subfields:
//!
//! * `200 $a $e` — title and subtitle,
//! * `700 $a $b` — author (surname, forename); `710` for a corporate author,
//! * `210` / `214 $c $d` — publisher and date,
//! * `461 $t $v` — the collection, and the volume number inside it,
//! * `010 $a` — ISBN.
//!
//! No API key is needed, and `roxmltree` (already used for EPUB metadata) is
//! enough to read the answer. Everything here is blocking network IO and must
//! run off the UI thread.

use std::io::Read;
use std::time::Duration;

use roxmltree::Document;

/// BnF SRU endpoint. The schema is chosen per request (see the module comment).
const ENDPOINT: &str = "https://catalogue.bnf.fr/api/SRU";

/// One catalogue record, reduced to the fields the details page shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookRecord {
    /// Title, with its subtitle appended as `Title : Subtitle`.
    pub title: String,
    /// Author names, joined with ` ; ` when a book has several.
    pub author: Option<String>,
    /// Publication year, as the catalogue spells it (`DL 2018` -> `2018`).
    pub date: Option<String>,
    /// The collection/series the volume belongs to (`461 $t`).
    pub saga: Option<String>,
    /// The volume number inside that collection (`461 $v`).
    pub tome: Option<String>,
    /// Publisher name.
    pub publisher: Option<String>,
    /// ISBN, which identifies this exact edition.
    pub isbn: Option<String>,
}

impl BookRecord {
    /// The record as the rows the details page shows once it has replaced the
    /// metadata read from the EPUB.
    ///
    /// Title, author and year are always present; the saga and its tome only
    /// appear when the catalogue names a collection, and the tome only when it
    /// also gives a volume number.
    pub fn metadata_rows(&self) -> Vec<(String, String)> {
        let mut rows = vec![
            ("Title".to_string(), self.title.clone()),
            (
                "Author".to_string(),
                self.author.clone().unwrap_or_else(|| "Unknown".to_string()),
            ),
            (
                "Published".to_string(),
                self.date.clone().unwrap_or_else(|| "Unknown".to_string()),
            ),
        ];

        if let Some(saga) = &self.saga {
            rows.push(("Saga".to_string(), saga.clone()));
            if let Some(tome) = &self.tome {
                rows.push(("Tome".to_string(), tome.clone()));
            }
        }

        rows
    }
}

/// The answer to one search: how many records exist in total, and the first few
/// of them ready to display.
pub struct CatalogueResults {
    /// Matches in the whole catalogue, not just the ones we asked for.
    pub total: u64,
    /// At most the number of records that were asked for.
    pub books: Vec<BookRecord>,
}

/// Searches the catalogue for `title` (and, when we have one, `author`).
///
/// Errors are lowercase clauses meant to complete the sentence "Could not
/// search the BnF catalogue: ...".
pub fn search_books(
    title: &str,
    author: Option<&str>,
    limit: usize,
) -> Result<CatalogueResults, String> {
    let Some(query) = cql_query(title, author) else {
        return Err("this EPUB does not name anything to search for".to_string());
    };

    // Ask for more records than we show, because the relevance ranking mixes in
    // films, recordings and other non-book material (see `parse_response`).
    let wanted = (limit * 4).clamp(1, 40);
    let results = run_query(&query, wanted, limit)?;

    // The author as written inside an EPUB ("J. K. Rowling") rarely matches the
    // catalogue's "Rowling, J. K.", so an empty answer may just mean the author
    // filter was too strict. Ask again with the title alone.
    if results.books.is_empty()
        && author.is_some()
        && let Some(fallback) = cql_query(title, None)
    {
        return run_query(&fallback, wanted, limit);
    }

    Ok(results)
}

/// Builds the CQL query, or `None` when the title normalises to nothing.
///
/// CQL quoted strings have only one escape character (`\`); dropping it and any
/// double quote keeps the query well-formed whatever the EPUB contained.
fn cql_query(title: &str, author: Option<&str>) -> Option<String> {
    let title = clean(title);
    if title.is_empty() {
        return None;
    }

    let mut query = format!("bib.title all \"{title}\"");
    if let Some(author) = author {
        let author = clean(author);
        if !author.is_empty() {
            query.push_str(&format!(" and bib.author all \"{author}\""));
        }
    }
    Some(query)
}

/// Trims a value and squeezes its runs of whitespace into single spaces.
fn clean(text: &str) -> String {
    text.chars()
        .filter(|character| *character != '"' && *character != '\\')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Sends one SRU request and parses the answer.
fn run_query(query: &str, wanted: usize, limit: usize) -> Result<CatalogueResults, String> {
    let xml = fetch(query, wanted)?;
    parse_response(&xml, limit)
}

/// Performs the HTTP GET and returns the response body as text.
fn fetch(query: &str, wanted: usize) -> Result<String, String> {
    let url = format!(
        "{ENDPOINT}?version=1.2&operation=searchRetrieve\
         &recordSchema=unimarcxchange&maximumRecords={wanted}&query={}",
        percent_encode(query)
    );

    let response = ureq::get(&url)
        // Some servers refuse requests without one; naming the app is also the
        // polite thing to do for a free public service.
        .set("User-Agent", "elibrary/0.1 (EPUB metadata lookup)")
        .set("Accept", "application/xml")
        .timeout(Duration::from_secs(20))
        .call()
        .map_err(|error| match error {
            ureq::Error::Status(code, _) => format!("the catalogue answered HTTP {code}"),
            other => format!("the catalogue could not be reached ({other})"),
        })?;

    // A generous ceiling: one record is a few kilobytes, so this only guards
    // against a pathological response, never against a normal one.
    let mut body = Vec::new();
    response
        .into_reader()
        .take(8 * 1024 * 1024)
        .read_to_end(&mut body)
        .map_err(|error| format!("its answer could not be read ({error})"))?;

    // The service always answers in UTF-8, accented French included. A lossy
    // conversion keeps a damaged answer readable instead of failing outright.
    Ok(String::from_utf8_lossy(&body).into_owned())
}

/// `%`-escapes everything that is not an unreserved URL character, so accented
/// titles survive the trip (`é` -> `%C3%A9`).
fn percent_encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(*byte as char);
            }
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    encoded
}

/// Reads a whole SRU answer: the total match count and every record it carries.
fn parse_response(xml: &str, limit: usize) -> Result<CatalogueResults, String> {
    let document =
        Document::parse(xml).map_err(|error| format!("its answer is not valid XML ({error})"))?;

    let total = document
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name() == "numberOfRecords")
        .and_then(|node| node.text())
        .and_then(|text| text.trim().parse::<u64>().ok())
        .unwrap_or(0);

    let mut found: Vec<(u8, BookRecord)> = Vec::new();
    for record in document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "recordData")
    {
        let fields = fields_of(record);
        let Some(book) = book_from(&fields) else {
            continue;
        };
        // Different editions of one book are worth showing (they carry another
        // date or collection); identical rows are not.
        if found.iter().all(|(_, existing)| *existing != book) {
            found.push((book_rank(&fields), book));
        }
    }

    // A title search also matches films and recordings. Put the printed books
    // first; `sort_by_key` is stable, so relevance order survives inside each
    // group.
    found.sort_by_key(|(rank, _)| std::cmp::Reverse(*rank));

    let books = found
        .into_iter()
        .take(limit)
        .map(|(_, book)| book)
        .collect();

    Ok(CatalogueResults { total, books })
}

/// One `datafield` of a record: its tag and its subfields in document order.
struct Field {
    tag: String,
    subfields: Vec<(String, String)>,
}

/// Collects the `datafield` elements of one record.
fn fields_of(record: roxmltree::Node) -> Vec<Field> {
    record
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "datafield")
        .map(|node| Field {
            tag: node.attribute("tag").unwrap_or_default().to_string(),
            subfields: node
                .children()
                .filter(|child| child.is_element() && child.tag_name().name() == "subfield")
                .filter_map(|child| {
                    Some((child.attribute("code")?.to_string(), text_of(child)))
                })
                .collect(),
        })
        .collect()
}

/// Turns the fields of one record into a [`BookRecord`], `None` when the record
/// has no usable title (some catalogue entries describe only a collection).
fn book_from(fields: &[Field]) -> Option<BookRecord> {
    let (saga, tome) = saga_of(fields);
    Some(BookRecord {
        title: joined(fields, &["200"], &["a", "e"], " : ")?,
        author: author_of(fields),
        date: date_of(fields),
        saga,
        tome,
        publisher: joined(fields, &["214", "210"], &["c"], " "),
        isbn: joined(fields, &["010"], &["a"], " "),
    })
}

/// How likely a record is a book, used to sort the search results.
///
/// Two points for material the catalogue itself calls text (`200 $b`), one more
/// for an ISBN (`010`), which recordings and films get as an EAN instead. A
/// record without either hint keeps a middling score rather than being dropped.
fn book_rank(fields: &[Field]) -> u8 {
    let is_text = fields
        .iter()
        .find(|field| field.tag == "200")
        .and_then(|field| subfield(field, "b"))
        .map(|kind| kind.to_lowercase().contains("texte"))
        .unwrap_or(true);
    let has_isbn = fields.iter().any(|field| field.tag == "010");

    u8::from(is_text) * 2 + u8::from(has_isbn)
}

/// The first subfield `code` of the first field whose tag is in `tags`.
fn subfield<'a>(field: &'a Field, code: &str) -> Option<&'a str> {
    field
        .subfields
        .iter()
        .find(|(candidate, value)| candidate == code && !value.is_empty())
        .map(|(_, value)| value.as_str())
}

/// Reads the first of several possible fields, joining the listed subfields
/// with `separator` (`200 $a` + `$e` -> `Title : Subtitle`).
fn joined(fields: &[Field], tags: &[&str], codes: &[&str], separator: &str) -> Option<String> {
    for tag in tags {
        let Some(field) = fields.iter().find(|field| field.tag == *tag) else {
            continue;
        };
        let parts: Vec<&str> = codes
            .iter()
            .filter_map(|code| subfield(field, code))
            .collect();
        if !parts.is_empty() {
            return Some(parts.join(separator));
        }
    }
    None
}

/// Author names from the MARC responsibility fields, in document order.
///
/// `700`/`701` are personal names and `710`/`711` corporate ones; `$a` is the
/// surname and `$b` the forename, which the catalogue stores in that order.
/// Records that carry no such field fall back to the statement of
/// responsibility printed on the title page (`200 $f`).
fn author_of(fields: &[Field]) -> Option<String> {
    let mut names: Vec<String> = Vec::new();
    for field in fields
        .iter()
        .filter(|field| matches!(field.tag.as_str(), "700" | "701" | "710" | "711"))
    {
        let name = match (subfield(field, "a"), subfield(field, "b")) {
            (Some(surname), Some(forename)) => format!("{surname}, {forename}"),
            (Some(surname), None) => surname.to_string(),
            (None, _) => continue,
        };
        if !names.contains(&name) {
            names.push(name);
        }
    }

    if !names.is_empty() {
        return Some(names.join(" ; "));
    }

    joined(fields, &["200"], &["f"], " ")
}

/// Publication year, from the modern `214` or the older `210` field.
fn date_of(fields: &[Field]) -> Option<String> {
    let raw = joined(fields, &["214", "210"], &["d"], " ")?;
    // The catalogue writes `DL 2018`, `[DL 2003]` or `2003 [D.L.]`; the year is
    // the only part worth showing.
    first_year(&raw).or(Some(raw))
}

/// The first four-digit number in `text`, e.g. `DL 2018` -> `2018`.
fn first_year(text: &str) -> Option<String> {
    let characters: Vec<char> = text.chars().collect();
    characters
        .windows(4)
        .find(|window| window.iter().all(char::is_ascii_digit))
        .map(|window| window.iter().collect())
}

/// The collection a volume belongs to: `461` in current records, `410` in older
/// ones. Returns the saga name and, when the catalogue gives one, the volume
/// number inside it.
fn saga_of(fields: &[Field]) -> (Option<String>, Option<String>) {
    for tag in ["461", "410"] {
        let Some(field) = fields.iter().find(|field| field.tag == tag) else {
            continue;
        };
        let Some(name) = subfield(field, "t") else {
            continue;
        };
        return (
            Some(name.to_string()),
            subfield(field, "v").map(str::to_string),
        );
    }
    (None, None)
}

/// All text below `node`, joined, with whitespace collapsed: MARC XML puts
/// stray spaces and newlines inside subfield values.
fn text_of(node: roxmltree::Node) -> String {
    node.descendants()
        .filter(|child| child.is_text())
        .filter_map(|child| child.text())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
