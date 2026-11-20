//! The whole visual layer: the single screen, and the pieces its rows share.
//!
//! Nothing here owns state or does IO. Every part is a plain function that
//! takes a borrow of [`crate::app::EpubBrowser`] and returns an element tree, so
//! the "what to show" decision stays in [`crate::app`].

pub mod components;
pub mod details;
pub mod list;
pub mod theme;
