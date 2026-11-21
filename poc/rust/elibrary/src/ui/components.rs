//! Small, reusable pieces of UI shared by both columns.

use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgb};

use crate::ui::theme;

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
///
/// `pub(crate)` because the catalogue results on the details page reuse it for
/// their own cards.
pub(crate) fn detail_row(label: &str, value: &str) -> impl IntoElement {
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
