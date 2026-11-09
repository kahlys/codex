//! The application state and the actions that change it.
//!
//! This is the "controller" layer, and it has two jobs only:
//!
//! * hold [`EpubBrowser`] — every field the window needs to draw a frame, plus
//!   which page is current;
//! * react to the user — pick a folder, open a file, go back, step the counter.
//!
//! Notice what is *not* here: no element building (that is [`crate::ui`]) and no
//! file-system or ZIP code (that is [`crate::library`]). The handlers orchestrate
//! the two: they update state, ask the library to do blocking work on a
//! background task, and call `cx.notify()` so the UI redraws.
//!
//! Blocking work never runs on the UI thread. Both long operations follow the
//! same shape: show a loading state, `background_spawn` the work, then update
//! the state again on the main thread when the task resolves.

use std::path::PathBuf;

use gpui::{
    ClickEvent, Context, IntoElement, PathPromptOptions, Render, ScrollHandle, Window, div,
    prelude::*, rgb,
};

use crate::library::{BookDetails, collect_epubs, read_epub};
use crate::ui::{details, list, theme};

/// Which page the single window is showing.
///
/// There is no navigation stack: this app only ever has two pages, so an enum
/// with the current one is enough. Switching pages is a plain assignment plus
/// `cx.notify()`.
pub enum Screen {
    /// The search bar plus the list of `.epub` files.
    List,
    /// The metadata page for one file. `state` tracks the three phases of
    /// reading it: still reading, parsed, or failed.
    Details { path: PathBuf, state: DetailsState },
}

/// Phase of the metadata read shown on the details page.
pub enum DetailsState {
    /// The background task is still opening the archive / parsing the XML.
    Loading,
    /// Everything the file advertised, ready to display.
    Ready(BookDetails),
    /// Something went wrong; the message is shown verbatim.
    Failed(String),
}

/// The application state.
///
/// In GPUI, a "view" is just a struct with a `Render` impl. The framework stores
/// it in an `Entity<EpubBrowser>` and calls `render(&mut self, ...)` whenever the
/// window needs a frame. Keeping the whole UI state in one struct like this is
/// the normal pattern for small apps (Zed itself splits state across many
/// entities that observe each other).
///
/// The fields are `pub(crate)` so the page modules in [`crate::ui`] can read
/// them directly; that is cheaper to read than seven one-line getters, and this
/// crate is a single binary, so "crate" is already the smallest useful scope.
pub struct EpubBrowser {
    /// The number being displayed. `i32` (signed) so it can go negative.
    pub(crate) count: i32,
    /// The folder the user picked, if any. Shown in the search bar.
    pub(crate) folder: Option<PathBuf>,
    /// Every `.epub` file under `folder` (including subfolders), sorted.
    pub(crate) epubs: Vec<PathBuf>,
    /// One-line message under the search bar: progress, result count, errors.
    pub(crate) status: String,
    /// True while a scan is in flight, so the UI can say what it is doing.
    pub(crate) scanning: bool,
    /// Scroll offset of the results list. It must live on the view: a
    /// `ScrollHandle` built inside `render` would reset on every frame.
    pub(crate) scroll_handle: ScrollHandle,
    /// Scroll offset of the details page. Separate from `scroll_handle` so the
    /// list keeps its position while the user is on the details page.
    pub(crate) details_scroll: ScrollHandle,
    /// Which page is on screen. Read in `render` to pick the layout.
    screen: Screen,
}

impl EpubBrowser {
    /// Constructor, referenced as `EpubBrowser::new` inside `cx.new(...)`.
    /// Nothing GPUI-specific happens here: it just builds the initial state.
    pub fn new() -> Self {
        Self {
            count: 0,
            folder: None,
            epubs: Vec::new(),
            status: "Pick a folder to list the .epub files inside it.".to_string(),
            scanning: false,
            scroll_handle: ScrollHandle::new(),
            details_scroll: ScrollHandle::new(),
            screen: Screen::List,
        }
    }

    /// Opens the native folder picker, then scans the chosen folder.
    ///
    /// `prompt_for_paths` does not show the dialog synchronously: it returns a
    /// one-shot receiver that resolves with the user's choice (or `None` when
    /// they cancel). So this handler starts a task with `cx.spawn` and awaits
    /// that receiver there, which keeps the UI thread free while the dialog is
    /// open and while the folder is being walked.
    pub(crate) fn pick_folder(
        &mut self,
        _event: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // `files: false` + `directories: true` = a "pick a folder" dialog
        // rather than a file dialog. `multiple: false` allows exactly one.
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Select folder".into()),
        });

        // `async move |this, cx| ...` is an async closure. GPUI hands it a weak
        // handle to this view (`this`) and an `AsyncApp` (`cx`) that can be
        // held across `.await` points. `.detach()` lets the task outlive this
        // call; without holding or detaching it, the future would be dropped.
        cx.spawn(async move |this, cx| {
            let folder = match receiver.await {
                // `Ok(Ok(None))` means the user closed the dialog.
                Ok(Ok(paths)) => paths.and_then(|paths| paths.into_iter().next()),
                Ok(Err(error)) => {
                    this.update(cx, |this, cx| {
                        this.scanning = false;
                        this.status = format!("Could not open the folder picker: {error}");
                        cx.notify();
                    })
                    .ok();
                    return;
                }
                // The sending end was dropped; nothing to do.
                Err(_) => None,
            };

            let Some(folder) = folder else {
                return;
            };

            // A tiny state change, then `notify` so the list clears and the
            // status line immediately shows that work is happening. Picking a
            // new folder also leaves the details page, if it was open: its file
            // may no longer be in the new list.
            this.update(cx, |this, cx| {
                this.folder = Some(folder.clone());
                this.epubs.clear();
                this.screen = Screen::List;
                // Drop the previous scroll offset: it belongs to the old list.
                this.scroll_handle = ScrollHandle::new();
                this.scanning = true;
                this.status = "Scanning for .epub files…".to_string();
                cx.notify();
            })
            .ok();

            // Walking a large tree can take seconds, so it runs on the
            // background executor (`collect_epubs` uses plain `std::fs`, which
            // is blocking). `background_spawn` requires `Send`.
            let scan = cx.background_spawn(async move { collect_epubs(&folder) });
            let epubs = scan.await;

            // Back on the main thread: store the results and repaint.
            this.update(cx, |this, cx| {
                this.scanning = false;
                this.status = match epubs.len() {
                    0 => "No .epub files in that folder.".to_string(),
                    1 => "1 .epub file found.".to_string(),
                    n => format!("{n} .epub files found."),
                };
                this.epubs = epubs;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Switches to the details page for `path` and starts reading its metadata.
    ///
    /// The archive is opened on the background executor because it touches the
    /// disk, and parsing XML is not free either. The page appears immediately in
    /// its `Loading` state, then the same weak-entity pattern as
    /// [`Self::pick_folder`] fills it in.
    pub(crate) fn open_epub(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        // A fresh handle per file, so every epub starts at the top.
        self.details_scroll = ScrollHandle::new();
        self.screen = Screen::Details {
            path: path.clone(),
            state: DetailsState::Loading,
        };
        // Show the loading page before the read finishes.
        cx.notify();

        let read_path = path.clone();
        let task = cx.background_spawn(async move { read_epub(&read_path) });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                // Only fill in the page if it still shows this file: the user
                // may have pressed "back" (or opened another file) meanwhile.
                let shows_path = matches!(
                    &this.screen,
                    Screen::Details { path: shown, .. } if *shown == path
                );
                if shows_path {
                    let state = match result {
                        Ok(details) => DetailsState::Ready(details),
                        Err(error) => {
                            DetailsState::Failed(format!("Could not read this EPUB: {error}."))
                        }
                    };
                    this.screen = Screen::Details { path, state };
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Event handler for the "← Back to list" button on the details page.
    ///
    /// The signature is what GPUI expects from a listener after being wrapped by
    /// `Context::listener`:
    /// * `&mut self`  — exclusive access to the view state (this is the reason
    ///   `listener` exists: plain `on_click` callbacks cannot reach it),
    /// * `&ClickEvent` — what happened (mouse button, position, modifiers...),
    ///   ignored here, hence the `_` name,
    /// * `&mut Window` — the window that was clicked,
    /// * `&mut Context<Self>` — the view context, used here only to notify.
    ///
    /// Being an associated *function* rather than a closure lets us pass it
    /// straight to `cx.listener(Self::go_back)` without allocating.
    pub(crate) fn go_back(
        &mut self,
        _event: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.screen = Screen::List;
        // THE key line: without `notify` the state changes but the screen keeps
        // showing the old frame.
        cx.notify();
    }

    /// Event handler for the "+" button.
    pub(crate) fn increment(
        &mut self,
        _event: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.count += 1;

        // Changes the footer number, so the window needs a new frame.
        cx.notify();
    }

    /// Event handler for the "-" button. Identical to `increment`, minus one.
    pub(crate) fn decrement(
        &mut self,
        _event: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.count -= 1;
        cx.notify();
    }
}

/// `Render` is the only trait a view must implement. GPUI calls it once per
/// frame, for the roots of windows that need painting.
///
/// * `&mut self` — read (and, if you want, write) the state.
/// * `&mut Window` — window-level services: focus, bounds, scroll, etc. Unused
///   in this small app.
/// * `&mut Context<Self>` — the view context, used to spawn tasks, observe other
///   entities, notify, open windows...
///
/// Note that `render` does **not** return widgets to be mutated later; it
/// returns a value describing the UI for *this* frame. GPUI diffs those trees
/// internally so only what changed is repainted.
///
/// The window keeps one root view, so "navigating" is just choosing which page
/// the current state describes. All this impl does is pick the page and give it
/// the window's background — the pages themselves live in [`crate::ui`].
impl Render for EpubBrowser {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = match &self.screen {
            Screen::List => list::render(self, cx),
            Screen::Details { path, state } => details::render(self, path, state, cx),
        };

        div()
            // Fill the whole window. `size_full()` == width:100% + height:100%.
            .size_full()
            // Flexbox layout, exactly like CSS: a column that fills the window.
            .flex()
            .flex_col()
            .p_4()
            .bg(rgb(theme::WINDOW))
            .text_color(rgb(theme::TEXT))
            .child(page)
    }
}
