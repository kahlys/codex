//! The list page: search bar, status line, results, footer counter.
//!
//! Every function here only *reads* the view; the actions themselves live in
//! [`crate::app`] and are reached through `cx.listener(...)`.

use std::path::Path;

use gpui::{
    AnyElement, ClickEvent, Context, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, Window, div, prelude::*, px, rgb,
};

use crate::app::EpubBrowser;
use crate::ui::components::stepper_button;
use crate::ui::theme;

/// Builds the whole page.
pub fn render(app: &EpubBrowser, cx: &mut Context<EpubBrowser>) -> AnyElement {
    div()
        .flex_1()
        .w_full()
        .flex()
        .flex_col()
        .gap_4()
        // The page must be allowed to be smaller than its content for the
        // inner list to scroll.
        .min_h_0()
        // ---- Search bar (opens the folder picker) ------------------
        .child(search_bar(app, cx))
        // ---- Status line ------------------------------------------
        .child(
            div()
                .text_size(px(12.0))
                .text_color(rgb(theme::TEXT_MUTED))
                .child(app.status.clone()),
        )
        // ---- The .epub files --------------------------------------
        .child(results(app, cx))
        // ---- The original counter, kept in the footer -------------
        .child(footer(app, cx))
        .into_any_element()
}

/// The bar at the top. It looks like a search field, but its job is only to
/// open the folder dialog: the whole bar is clickable.
fn search_bar(app: &EpubBrowser, cx: &mut Context<EpubBrowser>) -> impl IntoElement {
    // Before a folder is picked the bar shows a placeholder, so it reads as
    // an empty search field. Afterwards it shows the chosen path.
    let (label, muted) = match &app.folder {
        Some(folder) => (folder.display().to_string(), false),
        None => ("Search a folder for .epub files…".to_string(), true),
    };

    div()
        // `id` is not just for clicks: an interactive element needs a stable
        // id to keep its per-element state (hover, focus...) between frames.
        .id("folder-picker")
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(rgb(theme::FIELD))
        .border_1()
        .border_color(rgb(theme::CONTROL))
        .cursor_pointer()
        // `hover` receives a style refinement that GPUI applies only while
        // the pointer is over the element.
        .hover(|style| style.bg(rgb(theme::FIELD_HOVER)))
        .child(
            div()
                // `flex_1` lets the path take the remaining width, and
                // `truncate` adds an ellipsis instead of overflowing.
                .flex_1()
                .truncate()
                .text_size(px(14.0))
                .text_color(if muted {
                    rgb(theme::TEXT_FAINT)
                } else {
                    rgb(theme::TEXT)
                })
                .child(label),
        )
        .child(
            div()
                .px_3()
                .py_1()
                .rounded_md()
                .bg(rgb(theme::ACCENT))
                .text_color(rgb(theme::ON_ACCENT))
                .text_size(px(12.0))
                .child("Pick folder…"),
        )
        // The listener needs `&mut self`, which `on_click` alone cannot
        // provide, hence `cx.listener(...)`.
        .on_click(cx.listener(EpubBrowser::pick_folder))
}

/// The results area: fills the space between the search bar and the footer.
fn results(app: &EpubBrowser, cx: &mut Context<EpubBrowser>) -> AnyElement {
    if app.epubs.is_empty() {
        // Both branches return different element types, so each one is
        // boxed with `into_any_element()` to share the `AnyElement` type.
        return div()
            .flex_1()
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(13.0))
            .text_color(rgb(theme::TEXT_FAINT))
            .child(if app.scanning {
                "Scanning folders…"
            } else {
                "No .epub files to show yet."
            })
            .into_any_element();
    }

    div()
        // Scrollable containers must have an id (see `StatefulInteractiveElement`)
        // and a bounded size. `flex_1` gives it exactly the leftover height,
        // so `overflow_y_scroll` actually has something to scroll.
        .id("epub-results")
        .flex_1()
        // A flex item refuses to shrink below its content unless its
        // minimum size is zeroed (CSS `min-height: auto` behaves the same).
        // Without this the list grows past the window instead of scrolling.
        .min_h_0()
        .w_full()
        .flex()
        .flex_col()
        .gap_1()
        .overflow_y_scroll()
        // `track_scroll` ties the div to the handle stored on the view, so
        // the offset survives the rebuild that happens every frame.
        .track_scroll(&app.scroll_handle)
        // `children` takes an iterator, so the list needs no temporary Vec.
        // The index doubles as part of each row's element id. The rows are
        // built up front because each one borrows `cx` mutably while its
        // click listener is created.
        .children(
            app.epubs
                .iter()
                .enumerate()
                .map(|(index, path)| row(index, path, cx).into_any_element())
                .collect::<Vec<_>>(),
        )
        .into_any_element()
}

/// One row of the list: the file name with its folder underneath. Clicking it
/// opens the details page for that file.
fn row(index: usize, path: &Path, cx: &mut Context<EpubBrowser>) -> impl IntoElement {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let folder = path
        .parent()
        .map(|parent| parent.display().to_string())
        .unwrap_or_default();
    // The handler must be `'static`, so it owns its own copy of the path.
    let target = path.to_path_buf();

    div()
        // A clickable row needs an id. The list index makes it unique, and
        // stays stable while the rows are rebuilt every frame.
        .id(("epub-row", index))
        .w_full()
        .flex()
        .flex_col()
        .gap_1()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(rgb(theme::SURFACE))
        .cursor_pointer()
        .hover(|style| style.bg(rgb(theme::SURFACE_HOVER)))
        .child(div().truncate().text_size(px(14.0)).child(name))
        .child(
            div()
                .truncate()
                .text_size(px(11.0))
                .text_color(rgb(theme::TEXT_FAINT))
                .child(folder),
        )
        // A closure listener (instead of a plain fn) is what lets the click
        // carry the path of the row it belongs to.
        .on_click(cx.listener(
            move |this: &mut EpubBrowser,
                  _event: &ClickEvent,
                  _window: &mut Window,
                  cx: &mut Context<EpubBrowser>| {
                this.open_epub(target.clone(), cx);
            },
        ))
}

/// Brand on the left, the original counter on the right.
fn footer(app: &EpubBrowser, cx: &mut Context<EpubBrowser>) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_size(px(12.0))
                .text_color(rgb(theme::TEXT_DIM))
                .child("GPUI EPUB browser"),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .child(stepper_button(
                    "-",
                    "decrement-button",
                    EpubBrowser::decrement,
                    cx,
                ))
                // A fixed minimum width so the buttons do not shift when the
                // number goes from 9 to 10 or negative.
                .child(
                    div()
                        .min_w(px(30.0))
                        .flex()
                        .justify_center()
                        .text_size(px(16.0))
                        .child(app.count.to_string()),
                )
                .child(stepper_button(
                    "+",
                    "increment-button",
                    EpubBrowser::increment,
                    cx,
                )),
        )
}
