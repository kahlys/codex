//! The metadata page for one EPUB: a back button and whatever we could read.

use std::path::Path;

use gpui::{
    AnyElement, Context, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div,
    prelude::*, px, rgb,
};

use crate::app::{DetailsState, EpubBrowser};
use crate::ui::components::detail_section;
use crate::ui::theme;

/// Builds the whole page for `path`, in whichever phase `state` describes.
pub fn render(
    app: &EpubBrowser,
    path: &Path,
    state: &DetailsState,
    cx: &mut Context<EpubBrowser>,
) -> AnyElement {
    let header = div()
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .child(
            div()
                .id("back-to-list")
                .px_3()
                .py_1()
                .rounded_md()
                .bg(rgb(theme::CONTROL))
                .cursor_pointer()
                .hover(|style| style.bg(rgb(theme::CONTROL_HOVER)))
                .child(div().text_size(px(13.0)).child("← Back to list"))
                // `go_back` only flips the screen back to the list.
                .on_click(cx.listener(EpubBrowser::go_back)),
        )
        .child(
            div()
                // The file name/path completes the headline, so it may
                // shrink and get an ellipsis but never wrap.
                .flex_1()
                .truncate()
                .text_size(px(12.0))
                .text_color(rgb(theme::TEXT_FAINT))
                .child(path.display().to_string()),
        );

    let body: AnyElement = match state {
        DetailsState::Loading => loading(),
        DetailsState::Failed(error) => failed(error),
        DetailsState::Ready(details) => ready(app, details),
    };

    div()
        .flex_1()
        // Part of the scroll chain: header stays put, `body` scrolls.
        .min_h_0()
        .w_full()
        .flex()
        .flex_col()
        .gap_4()
        .child(header)
        .child(body)
        .into_any_element()
}

/// The page while the archive is still being read.
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

/// The page when the file could not be read; the message is shown verbatim.
fn failed(error: &str) -> AnyElement {
    div()
        .flex_1()
        .w_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
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
                .child("Use \"← Back to list\" to pick another file."),
        )
        .into_any_element()
}

/// The page once the metadata has been parsed.
fn ready(app: &EpubBrowser, details: &crate::library::BookDetails) -> AnyElement {
    div()
        .id("epub-details")
        .flex_1()
        // Metadata can be arbitrarily long, so this box has to be
        // allowed to shrink below its content, otherwise the page
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
        // ---- Facts about the file on disk -----------------
        .child(detail_section("File", &details.file))
        .into_any_element()
}
