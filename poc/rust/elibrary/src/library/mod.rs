//! Everything that does not draw anything.
//!
//! No file in this module imports `gpui`: it is plain, blocking Rust. The
//! layers above it are free to call these functions from a background task and
//! to test them without a window, which is exactly why they are kept apart
//! from the UI code.
//!
//! * [`scan`] — find the `.epub` files under a folder.
//! * [`epub`] — read the metadata stored inside one EPUB.
//! * [`lookup`] — ask the BnF catalogue what a book really is.
//! * [`format`] — turn numbers and timestamps into short display strings.

mod epub;
mod format;
mod lookup;
mod scan;

pub use epub::{BookDetails, read_epub};
pub use lookup::{BookRecord, CatalogueResults, search_books};
pub use scan::collect_epubs;
