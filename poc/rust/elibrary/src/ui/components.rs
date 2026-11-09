//! Small, reusable pieces of UI shared by both pages.

use gpui::{
    AnyElement, ClickEvent, Context, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, Window, div, prelude::*, px, rgb,
};

use crate::app::EpubBrowser;
use crate::ui::theme;

/// The signature GPUI wants from a listener once it has been adapted by
/// `Context::listener`: exclusive access to the view state (`&mut self` is the
/// very reason `listener` exists — plain `on_click` callbacks cannot reach it),
/// the event, the window, and the view context.
pub type Handler = fn(&mut EpubBrowser, &ClickEvent, &mut Window, &mut Context<EpubBrowser>);

/// One `-` / `+` button for the counter in the footer.
///
/// `handler` is a plain function pointer with the shape of [`Handler`];
/// `cx.listener(handler)` adapts it into the closure `on_click` wants,
/// re-attaching `&mut self` for us. Passing a function instead of writing a
/// closure inline keeps the call sites in `list.rs` short.
///
/// The return type `impl IntoElement` means "some element type"; the caller
/// only uses `.child(...)` on it, which is all `IntoElement` requires.
pub fn stepper_button(
    label: &'static str,
    id: &'static str,
    handler: Handler,
    cx: &mut Context<EpubBrowser>,
) -> impl IntoElement {
    div()
        // Interactive elements that keep per-element state need an id. Using
        // one here also disambiguates the two buttons in GPUI's element cache,
        // so the click handler is attached to the right node.
        .id(id)
        // `px_*` / `py_*` are padding; numbers map to a spacing scale.
        .px_3()
        .py_1()
        // `rounded_md` rounds the corners; `bg` takes a colour from `rgb`.
        .rounded_md()
        .bg(rgb(theme::CONTROL))
        // Show the hand cursor so it is obvious the div is clickable:
        // GPUI divs are inert unless told otherwise.
        .cursor_pointer()
        .hover(|style| style.bg(rgb(theme::CONTROL_HOVER)))
        // A single string child; anything that is `IntoElement` works here,
        // including `format!()`ed strings.
        .child(div().text_size(px(16.0)).child(label))
        // Fire on a full click (press + release on the element).
        .on_click(cx.listener(handler))
}

/// One labelled card of `label: value` rows on the details page.
pub fn detail_section(heading: &str, rows: &[(String, String)]) -> AnyElement {
    let mut section = div()
        .w_full()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .rounded_md()
        .bg(rgb(theme::SURFACE))
        .child(
            div()
                .text_size(px(11.0))
                .text_color(rgb(theme::ACCENT))
                .child(heading.to_uppercase()),
        )
        .children(rows.iter().map(|(label, value)| detail_row(label, value)));

    if rows.is_empty() {
        section = section.child(
            div()
                .text_size(px(12.0))
                .text_color(rgb(theme::TEXT_FAINT))
                .child("Nothing recorded."),
        );
    }

    section.into_any_element()
}

/// One row inside a [`detail_section`]: a fixed-width label and a value that
/// takes the rest of the line (and wraps when it is long).
fn detail_row(label: &str, value: &str) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_row()
        .items_start()
        .gap_4()
        .child(
            div()
                .w(px(130.0))
                // `flex_none` keeps the label from being squeezed by a long value.
                .flex_none()
                .text_size(px(12.0))
                .text_color(rgb(theme::TEXT_FAINT))
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_1()
                // `min_w_0` lets the value shrink to the available width so a
                // long unbroken token wraps inside the card instead of pushing
                // the row (and the page) wider than the window.
                .min_w_0()
                .text_size(px(13.0))
                .child(value.to_string()),
        )
}
