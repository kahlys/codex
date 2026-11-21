//! The body of a book row once it is expanded: a headline, the metadata the
//! EPUB stores, facts about the file, and the buttons that act on the book.
//!
//! It is drawn *inside* the row, under its name. The list is the only thing on
//! screen, so there is no second column to keep in sync: expanding a book grows
//! its own row and collapsing it takes the body away again.

use gpui::{
    AnyElement, Context, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div,
    prelude::*, px, rgb,
};

use crate::app::{DetailsState, EpubBrowser, LookupState, SaveState};
use crate::library::{BookDetails, CatalogueResults};
use crate::ui::components::detail_section;
use crate::ui::theme;

/// Builds the expanded part of a row: whatever is known about the book at this
/// point, plus the buttons that act on it.
pub fn expanded(
    app: &EpubBrowser,
    state: &DetailsState,
    cx: &mut Context<EpubBrowser>,
) -> AnyElement {
    let body: AnyElement = match state {
        DetailsState::Loading => loading(),
        DetailsState::Failed(error) => failed(error),
        DetailsState::Ready(details) => ready(app, details, cx),
    };

    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        // The content lines up under the file name rather than at the card's
        // edge, which is what makes the body read as part of the row.
        .pl(px(22.0))
        .pr_3()
        .pb_3()
        .child(body)
        .into_any_element()
}

/// The body while the archive is still being read.
fn loading() -> AnyElement {
    div()
        .w_full()
        .py_2()
        .flex()
        .items_center()
        .text_size(px(13.0))
        .text_color(rgb(theme::TEXT_MUTED))
        .child("Reading metadata…")
        .into_any_element()
}

/// The body when the file could not be read; the message is shown verbatim.
fn failed(error: &str) -> AnyElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_1()
        .py_2()
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
                .child("Click the row again to close it."),
        )
        .into_any_element()
}

/// The body once the metadata has been parsed.
fn ready(app: &EpubBrowser, details: &BookDetails, cx: &mut Context<EpubBrowser>) -> AnyElement {
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_3()
        // ---- Headline, and the buttons that act on this book -------
        .child(
            div()
                .w_full()
                .flex()
                .flex_row()
                .items_start()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().text_size(px(18.0)).child(details.title.clone()))
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
                .child(search_button(cx))
                .child(close_button(cx)),
        )
        // ---- Catalogue answer, above the metadata so it is seen at once ----
        // `children` takes an iterator; `Option` is one, so an "idle" lookup
        // contributes nothing to the row at all.
        .children(lookup_section(app))
        // ---- The full metadata table ----------------------
        // Always exactly what the file stores: the catalogue answer is only
        // ever shown through what was written into the EPUB, never as invented
        // rows. `detail_section` prints a placeholder when it is empty.
        .child(detail_section("Metadata", &details.entries))
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
                .child("Clean EPUB"),
        )
        .on_click(cx.listener(EpubBrowser::lookup_book))
        .into_any_element()
}

/// The button that collapses the row it sits in.
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

/// The catalogue block below the metadata: nothing while idle, a note while
/// searching, an error when the request failed, or confirmation that the
/// catalogue's values are now in the file.
fn lookup_section(app: &EpubBrowser) -> Option<AnyElement> {
    match &app.lookup {
        LookupState::Idle => None,
        LookupState::Loading => Some(note(
            "Cleaning the EPUB…",
            theme::TEXT_MUTED,
            theme::TEXT_FAINT,
        )),
        LookupState::Failed(error) => Some(note(error, theme::DANGER, theme::TEXT_FAINT)),
        LookupState::Ready(results) if results.books.is_empty() => Some(note(
            "No matching metadata was found; the file was left unchanged.",
            theme::TEXT_MUTED,
            theme::TEXT_FAINT,
        )),
        LookupState::Ready(results) => {
            let (message, color) = replaced_message(results, &app.save);
            Some(note(&message, color, theme::TEXT_FAINT))
        }
    }
}

/// The confirmation under the metadata: where the values came from, and what
/// happened when they were written back into the EPUB.
///
/// Success is only reported once the written file has been read back, so the
/// row's values are the file's own by the time this message appears.
fn replaced_message(results: &CatalogueResults, save: &SaveState) -> (String, u32) {
    // The first record is already applied; saying how many matched explains
    // why the values may not be the only ones the catalogue knows.
    let source = if results.total > 1 {
        format!(
            "EPUB cleaned ({} matches found, the first one was used).",
            results.total
        )
    } else {
        "EPUB cleaned.".to_string()
    };

    // `error` is a full clause ("It could not be written ..."), because the
    // failure may come from the write or from reading the file back.

    match save {
        SaveState::Idle | SaveState::Writing => {
            (format!("{source} Writing it into the file…"), theme::TEXT_MUTED)
        }
        SaveState::Saved => (format!("{source} It was saved in the file."), theme::TEXT_MUTED),
        SaveState::Failed(error) => (format!("{source} {error}."), theme::DANGER),
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
                .child("The title, author, year and series in the file were refreshed."),
        )
        .into_any_element()
}
