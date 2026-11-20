//! The single screen: search bar, the books, and the footer counter.
//!
//! There is no second column: the list owns the whole width, and clicking a row
//! expands that row in place (see [`crate::ui::details`]), so a book's metadata
//! appears under its name instead of in a panel beside the list.
//!
//! Every function here only *reads* the view; the actions themselves live in
//! [`crate::app`] and are reached through `cx.listener(...)`.

use std::path::Path;

use gpui::{
    AnyElement, ClickEvent, Context, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, Window, div, prelude::*, px, rgb,
};

use crate::app::{CleanMark, DetailsState, EpubBrowser};
use crate::ui::components::stepper_button;
use crate::ui::{details, theme};

/// Builds the whole window: search bar, the list, and the footer.
pub fn render(app: &EpubBrowser, cx: &mut Context<EpubBrowser>) -> AnyElement {
    div()
        .flex_1()
        .w_full()
        .flex()
        .flex_col()
        .gap_3()
        // The screen must be allowed to be smaller than its content for the
        // list to scroll.
        .min_h_0()
        // ---- Search bar (opens the folder picker) ------------------
        .child(search_bar(app, cx))
        // ---- Status line ------------------------------------------
        .child(status_line(app))
        // ---- The books, using the whole width ----------------------
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

/// One line saying what the scanner is doing or what it found.
fn status_line(app: &EpubBrowser) -> impl IntoElement {
    div()
        .text_size(px(12.0))
        .text_color(rgb(theme::TEXT_MUTED))
        .child(app.status.clone())
}

/// The list menu on top, the books themselves below, both full width.
fn results(app: &EpubBrowser, cx: &mut Context<EpubBrowser>) -> AnyElement {
    div()
        .w_full()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .gap_2()
        .child(list_menu(app, cx))
        .child(list_body(app, cx))
        .into_any_element()
}

/// The line above the list: how many books it holds, and the button that cleans
/// all of them in one go.
fn list_menu(app: &EpubBrowser, cx: &mut Context<EpubBrowser>) -> impl IntoElement {
    let summary = match app.epubs.len() {
        0 => "No books".to_string(),
        1 => "1 book".to_string(),
        n => format!("{n} books"),
    };

    let mut menu = div()
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_2()
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(theme::TEXT_FAINT))
                .child(summary),
        );

    // With nothing listed there is nothing to clean, so the button only appears
    // once the folder has been scanned.
    if !app.epubs.is_empty() {
        menu = menu.child(clean_all_button(app, cx));
    }

    menu
}

/// The "Clean all" button: the same catalogue lookup and rewrite as an expanded
/// row, once per file in the list.
fn clean_all_button(app: &EpubBrowser, cx: &mut Context<EpubBrowser>) -> impl IntoElement {
    // While a run is in progress the button turns into a quiet label: the handler
    // ignores clicks then, and the status line reports the progress.
    let (label, background, background_hover, color) = if app.cleaning {
        (
            "Cleaning…",
            theme::CONTROL,
            theme::CONTROL,
            theme::TEXT_MUTED,
        )
    } else {
        (
            "Clean all",
            theme::ACCENT,
            theme::ACCENT_HOVER,
            theme::ON_ACCENT,
        )
    };

    div()
        .id("clean-all")
        .px_3()
        .py_1()
        .rounded_md()
        .bg(rgb(background))
        .cursor_pointer()
        .hover(|style| style.bg(rgb(background_hover)))
        .child(div().text_size(px(12.0)).text_color(rgb(color)).child(label))
        .on_click(cx.listener(EpubBrowser::clean_all))
}

/// The scrollable list of books, or the placeholder shown while there is
/// nothing to list.
fn list_body(app: &EpubBrowser, cx: &mut Context<EpubBrowser>) -> AnyElement {
    if app.epubs.is_empty() {
        // Both branches return different element types, so each one is
        // boxed with `into_any_element()` to share the `AnyElement` type.
        return div()
            .flex_1()
            .w_full()
            .min_h_0()
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

    // The body of the one expanded row, if any. Only one book is open at a
    // time, so the selection is also the answer to "which row is taller".
    let selected = app.selected.as_ref();

    div()
        // Scrollable containers must have an id (see `StatefulInteractiveElement`)
        // and a bounded size. `flex_1` gives it the height left under the menu,
        // so `overflow_y_scroll` actually has something to scroll.
        .id("epub-results")
        .w_full()
        .flex_1()
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
                    let state = selected
                        .filter(|selection| selection.path == *path)
                        .map(|selection| &selection.state);
                    // Whether this app already rewrote the file, which the row
                    // shows as a green check, and whether the run now in
                    // progress is on it or has failed on it.
                    let cleaned = app.cleaned.contains(path);
                    let mark = app.clean_marks.get(path);
                    row(app, index, path, state, cleaned, mark, cx)
                })
                .collect::<Vec<_>>(),
        )
        .into_any_element()
}

/// One book of the list. Its name line is the click target: clicking it toggles
/// the row, which grows downwards to show the metadata in
/// [`crate::ui::details`]. The line also says where the book stands: a spinner
/// while a "Clean all" run is on it, a green check once this app has cleaned it,
/// a red cross when it failed.
fn row(
    app: &EpubBrowser,
    index: usize,
    path: &Path,
    state: Option<&DetailsState>,
    cleaned: bool,
    mark: Option<&CleanMark>,
    cx: &mut Context<EpubBrowser>,
) -> AnyElement {
    let expanded = state.is_some();
    // One glyph per state. The slot is drawn even when there is nothing to say
    // (a blank space), so every file name starts at the same x position.
    let (symbol, symbol_color) = match mark {
        Some(CleanMark::Working) => ("◌", theme::ACCENT),
        Some(CleanMark::Failed) => ("✗", theme::DANGER),
        None if cleaned => ("✓", theme::SUCCESS),
        None => (" ", theme::TEXT_FAINT),
    };
    // The chevron says whether the row is open, so the row looks clickable.
    let (chevron, chevron_color) = if expanded {
        ("▾", theme::ACCENT)
    } else {
        ("▸", theme::TEXT_FAINT)
    };
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

    let mut card = div()
        // A clickable element needs an id. The list index makes it unique, and
        // stays stable while the rows are rebuilt every frame.
        .id(("epub-row", index))
        .w_full()
        .flex()
        .flex_col()
        .rounded_md()
        // The selected row is outlined in the accent colour. The border is
        // drawn on every row (same colour as the background when collapsed)
        // so opening one does not make it a pixel taller.
        .border_1()
        .border_color(rgb(if expanded {
            theme::ACCENT
        } else {
            theme::SURFACE
        }))
        .bg(rgb(if expanded {
            theme::SURFACE_HOVER
        } else {
            theme::SURFACE
        }))
        .child(
            div()
                // The name line is its own click target, so the buttons that
                // live in the expanded body never toggle the row as well.
                .id(("epub-header", index))
                .w_full()
                .flex()
                .flex_col()
                .gap_1()
                .px_3()
                .py_2()
                .rounded_md()
                .cursor_pointer()
                .hover(|style| style.bg(rgb(theme::SURFACE_HOVER)))
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .flex_none()
                                .w(px(12.0))
                                .text_size(px(12.0))
                                .text_color(rgb(symbol_color))
                                .child(symbol),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(px(14.0))
                                .child(name),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_size(px(11.0))
                                .text_color(rgb(chevron_color))
                                .child(chevron),
                        ),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(px(11.0))
                        .text_color(rgb(theme::TEXT_FAINT))
                        .child(folder),
                )
                // A closure listener (instead of a plain fn) is what lets the
                // click carry the path of the row it belongs to.
                .on_click(cx.listener(
                    move |this: &mut EpubBrowser,
                          _event: &ClickEvent,
                          _window: &mut Window,
                          cx: &mut Context<EpubBrowser>| {
                        this.select_epub(target.clone(), cx);
                    },
                )),
        );

    // The open row grows downwards: the details belong to the row itself, so
    // the list scrolls as one piece and nothing sits beside it.
    if let Some(state) = state {
        card = card.child(details::expanded(app, state, cx));
    }

    card.into_any_element()
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
