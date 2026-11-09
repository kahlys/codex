//! The metadata panel on the right of the book list: a header and whatever we
//! could read from the selected EPUB, or a placeholder when nothing is
//! selected.

use std::path::Path;

use gpui::{
    AnyElement, Context, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div,
    prelude::*, px, rgb,
};

use crate::app::{DetailsState, EpubBrowser, LookupState};
use crate::library::{BookDetails, BookRecord, CatalogueResults};
use crate::ui::components::{detail_row, detail_section};
use crate::ui::theme;

/// Builds the right-hand panel for whichever book is selected.
pub fn panel(app: &EpubBrowser, cx: &mut Context<EpubBrowser>) -> AnyElement {
    let Some(selection) = &app.selected else {
        return placeholder();
    };

    let body: AnyElement = match &selection.state {
        DetailsState::Loading => loading(),
        DetailsState::Failed(error) => failed(error),
        DetailsState::Ready(details) => ready(app, details),
    };

    div()
        // `flex_1` takes the width the list did not use, and `min_w_0` keeps a
        // very long path from pushing the panel wider than the window.
        .flex_1()
        .min_w_0()
        .h_full()
        // Part of the scroll chain below.
        .min_h_0()
        .flex()
        .flex_col()
        .gap_3()
        // Header stays put, `body` scrolls.
        .child(header(&selection.path, &selection.state, cx))
        .child(body)
        .into_any_element()
}

/// The panel before a book has been clicked.
fn placeholder() -> AnyElement {
    div()
        .flex_1()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .px_4()
        .text_size(px(13.0))
        .text_color(rgb(theme::TEXT_FAINT))
        .child("Select a book on the left to see its metadata.")
        .into_any_element()
}

/// The path of the selected file, the catalogue button, and the button that
/// empties the panel.
fn header(path: &Path, state: &DetailsState, cx: &mut Context<EpubBrowser>) -> AnyElement {
    let mut row = div()
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .child(
            div()
                // The path completes the headline, so it may shrink and get an
                // ellipsis but never wrap.
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(12.0))
                .text_color(rgb(theme::TEXT_FAINT))
                .child(path.display().to_string()),
        );

    // The lookup button only makes sense once we know which title and author to
    // search for, so it appears with the "ready" phase.
    if matches!(state, DetailsState::Ready(_)) {
        row = row.child(search_button(cx));
    }

    row.child(close_button(cx)).into_any_element()
}

/// The button that clears the selection and brings the placeholder back.
fn close_button(cx: &mut Context<EpubBrowser>) -> AnyElement {
    div()
        .id("close-details")
        .px_2()
        .py_1()
        .rounded_md()
        .bg(rgb(theme::CONTROL))
        .cursor_pointer()
        .hover(|style| style.bg(rgb(theme::CONTROL_HOVER)))
        .child(
            div()
                .text_size(px(13.0))
                .text_color(rgb(theme::TEXT_MUTED))
                .child("×"),
        )
        .on_click(cx.listener(EpubBrowser::clear_selection))
        .into_any_element()
}

/// The panel while the archive is still being read.
fn loading() -> AnyElement {
    div()
        .flex_1()
        .w_full()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(13.0))
        .text_color(rgb(theme::TEXT_MUTED))
        .child("Reading metadata…")
        .into_any_element()
}

/// The panel when the file could not be read; the message is shown verbatim.
fn failed(error: &str) -> AnyElement {
    div()
        .flex_1()
        .w_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .px_4()
        .child(
            div()
                .text_size(px(13.0))
                .text_color(rgb(theme::DANGER))
                .child(error.to_string()),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(theme::TEXT_FAINT))
                .child("Pick another book from the list."),
        )
        .into_any_element()
}

/// The panel once the metadata has been parsed.
fn ready(app: &EpubBrowser, details: &BookDetails) -> AnyElement {
    div()
        .id("epub-details")
        .flex_1()
        // Metadata can be arbitrarily long, so this box has to be
        // allowed to shrink below its content, otherwise the panel
        // overflows the window and never scrolls.
        .min_h_0()
        .w_full()
        .flex()
        .flex_col()
        .gap_3()
        .overflow_y_scroll()
        .track_scroll(&app.details_scroll)
        // ---- Headline: title and author -------------------
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_size(px(22.0)).child(details.title.clone()))
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(theme::ACCENT))
                        .child(
                            details
                                .creator
                                .clone()
                                .unwrap_or_else(|| "Unknown author".to_string()),
                        ),
                ),
        )
        // ---- The full metadata table ----------------------
        // `detail_section` prints a placeholder when it is empty.
        .child(detail_section("Metadata", &details.entries))
        // ---- What the catalogue says, once the button was pressed ----
        // `children` takes an iterator; `Option` is one, so an "idle" lookup
        // contributes nothing to the panel at all.
        .children(lookup_section(app))
        // ---- Facts about the file on disk -----------------
        .child(detail_section("File", &details.file))
        .into_any_element()
}

/// The accent-coloured button that starts the catalogue search.
fn search_button(cx: &mut Context<EpubBrowser>) -> AnyElement {
    div()
        .id("lookup-bnf")
        .px_3()
        .py_1()
        .rounded_md()
        .bg(rgb(theme::ACCENT))
        .cursor_pointer()
        .hover(|style| style.bg(rgb(theme::ACCENT_HOVER)))
        .child(
            div()
                .text_size(px(13.0))
                .text_color(rgb(theme::ON_ACCENT))
                .child("Search the BnF"),
        )
        .on_click(cx.listener(EpubBrowser::lookup_book))
        .into_any_element()
}

/// The catalogue block below the EPUB metadata: nothing while idle, a note
/// while searching, an error when the request failed, or the matching books.
fn lookup_section(app: &EpubBrowser) -> Option<AnyElement> {
    match &app.lookup {
        LookupState::Idle => None,
        LookupState::Loading => Some(note(
            "Searching the BnF catalogue…",
            theme::TEXT_MUTED,
            theme::TEXT_FAINT,
        )),
        LookupState::Failed(error) => Some(note(error, theme::DANGER, theme::TEXT_FAINT)),
        LookupState::Ready(results) if results.books.is_empty() => Some(note(
            "No match in the BnF catalogue.",
            theme::TEXT_MUTED,
            theme::TEXT_FAINT,
        )),
        LookupState::Ready(results) => Some(results_section(results)),
    }
}

/// A card with one line of text and, under it, a quieter hint.
fn note(message: &str, color: u32, hint_color: u32) -> AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_1()
        .p_3()
        .rounded_md()
        .bg(rgb(theme::SURFACE))
        .child(div().text_size(px(13.0)).text_color(rgb(color)).child(message.to_string()))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(hint_color))
                .child("Bibliothèque nationale de France · catalogue.bnf.fr"),
        )
        .into_any_element()
}

/// The heading that says how many records matched, then one card per book.
fn results_section(results: &CatalogueResults) -> AnyElement {
    let shown = results.books.len();
    let heading = if results.total as usize > shown {
        format!("BnF catalogue · {} matches, first {shown} shown", results.total)
    } else {
        format!("BnF catalogue · {shown} match(es)")
    };

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(theme::ACCENT))
                .child(heading.to_uppercase()),
        )
        .children(results.books.iter().map(book_card))
        .into_any_element()
}

/// One catalogue record: its title as the headline, then the fields that tell
/// the user whether this is the right book.
fn book_card(book: &BookRecord) -> AnyElement {
    let mut rows: Vec<(String, String)> = Vec::with_capacity(5);
    rows.push((
        "Author".to_string(),
        book.author.clone().unwrap_or_else(|| "Unknown".to_string()),
    ));
    rows.push((
        "Published".to_string(),
        book.date.clone().unwrap_or_else(|| "Unknown".to_string()),
    ));
    // Always shown, so an empty saga is visible as the catalogue having none
    // rather than as the app forgetting to ask for it.
    rows.push((
        "Saga".to_string(),
        book.series.clone().unwrap_or_else(|| "—".to_string()),
    ));
    if let Some(publisher) = &book.publisher {
        rows.push(("Publisher".to_string(), publisher.clone()));
    }
    if let Some(isbn) = &book.isbn {
        rows.push(("ISBN".to_string(), isbn.clone()));
    }

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .rounded_md()
        .bg(rgb(theme::SURFACE_HOVER))
        .child(div().text_size(px(15.0)).child(book.title.clone()))
        .children(rows.iter().map(|(label, value)| detail_row(label, value)))
        .into_any_element()
}
