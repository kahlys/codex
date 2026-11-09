//! The single screen: search bar, the books on the left, the metadata panel on
//! the right, and the footer counter.
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
use crate::ui::{details, theme};

/// Width of the book column. Fixed, so the layout does not shift when the
/// selection changes; the metadata panel takes whatever width is left.
const LIST_WIDTH: f32 = 300.0;

/// Builds the whole window: search bar, two columns, footer.
pub fn render(app: &EpubBrowser, cx: &mut Context<EpubBrowser>) -> AnyElement {
    div()
        .flex_1()
        .w_full()
        .flex()
        .flex_col()
        .gap_3()
        // The screen must be allowed to be smaller than its content for the
        // inner columns to scroll.
        .min_h_0()
        // ---- Search bar (opens the folder picker) ------------------
        .child(search_bar(app, cx))
        // ---- Status line ------------------------------------------
        .child(status_line(app))
        // ---- Books on the left, selected book on the right ---------
        .child(
            div()
                .flex_1()
                .w_full()
                // This row is what gives both columns a bounded height;
                // without it neither side has anything to scroll inside.
                .min_h_0()
                .flex()
                .flex_row()
                .gap_3()
                .child(results(app, cx))
                .child(details::panel(app, cx)),
        )
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

/// One line saying what the scanner is doing or what it found.
fn status_line(app: &EpubBrowser) -> impl IntoElement {
    div()
        .text_size(px(12.0))
        .text_color(rgb(theme::TEXT_MUTED))
        .child(app.status.clone())
}

/// The left-hand column: every `.epub` file found under the chosen folder.
fn results(app: &EpubBrowser, cx: &mut Context<EpubBrowser>) -> AnyElement {
    if app.epubs.is_empty() {
        // Both branches return different element types, so each one is
        // boxed with `into_any_element()` to share the `AnyElement` type.
        return div()
            .w(px(LIST_WIDTH))
            .flex_none()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .px_2()
            .text_size(px(13.0))
            .text_color(rgb(theme::TEXT_FAINT))
            .child(if app.scanning {
                "Scanning folders…"
            } else {
                "No .epub files to show yet."
            })
            .into_any_element();
    }

    let selected_path = app.selected.as_ref().map(|selection| &selection.path);

    div()
        // Scrollable containers must have an id (see `StatefulInteractiveElement`)
        // and a bounded size. `h_full` gives it the height of the row, so
        // `overflow_y_scroll` actually has something to scroll.
        .id("epub-results")
        .w(px(LIST_WIDTH))
        .flex_none()
        .h_full()
        // A flex item refuses to shrink below its content unless its
        // minimum size is zeroed (CSS `min-height: auto` behaves the same).
        // Without this the list grows past the window instead of scrolling.
        .min_h_0()
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
                .map(|(index, path)| {
                    let selected =
                        selected_path.is_some_and(|chosen| chosen.as_path() == path.as_path());
                    row(index, path, selected, cx).into_any_element()
                })
                .collect::<Vec<_>>(),
        )
        .into_any_element()
}

/// One row of the left-hand list: the file name with its folder underneath.
/// Clicking it fills the metadata panel; it does not leave this screen.
fn row(
    index: usize,
    path: &Path,
    selected: bool,
    cx: &mut Context<EpubBrowser>,
) -> impl IntoElement {
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
        // The selected row is outlined in the accent colour. The border is
        // drawn on every row (same colour as the background when unselected)
        // so selecting one does not make it a pixel taller.
        .border_1()
        .border_color(rgb(if selected {
            theme::ACCENT
        } else {
            theme::SURFACE
        }))
        .bg(rgb(if selected {
            theme::SURFACE_HOVER
        } else {
            theme::SURFACE
        }))
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
                this.select_epub(target.clone(), cx);
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
