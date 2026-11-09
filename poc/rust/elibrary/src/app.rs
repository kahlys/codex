//! The application state and the actions that change it.
//!
//! This is the "controller" layer, and it has two jobs only:
//!
//! * hold [`EpubBrowser`] — every field the window needs to draw a frame, plus
//!   which book the right-hand panel is showing;
//! * react to the user — pick a folder, select a book, look it up in the BnF
//!   catalogue, close the panel, step the counter.
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

use crate::library::{
    BookDetails, CatalogueResults, EpubMetadata, collect_epubs, read_epub, search_books,
    write_metadata,
};
use crate::ui::{list, theme};

/// The book selected in the list, plus the progress of reading it.
///
/// The window is one screen with two columns, so "which book is selected" is
/// the only navigation state there is: `None` means the right-hand panel still
/// shows its placeholder.
pub struct Selection {
    /// The `.epub` file the right-hand panel is about.
    pub path: PathBuf,
    /// How far reading it has got: still reading, parsed, or failed.
    pub state: DetailsState,
}

/// Phase of the metadata read shown in the right-hand panel.
pub enum DetailsState {
    /// The background task is still opening the archive / parsing the XML.
    Loading,
    /// Everything the file advertised, ready to display.
    Ready(BookDetails),
    /// Something went wrong; the message is shown verbatim.
    Failed(String),
}

/// Phase of the optional BnF catalogue lookup, which the user starts by hand.
///
/// It is separate from [`DetailsState`] because the two are independent: the
/// file's own metadata is read automatically, while the catalogue search only
/// happens when the button is clicked, and its answer can fail on its own
/// (no network, unknown book...).
#[derive(Default)]
pub enum LookupState {
    /// The button has not been pressed for this file yet.
    #[default]
    Idle,
    /// The request is in flight.
    Loading,
    /// What the Bibliothèque nationale de France knows about this book; its
    /// first record is the one already applied to the panel.
    Ready(CatalogueResults),
    /// The request failed; the message is shown verbatim.
    Failed(String),
}

/// Phase of writing the catalogue metadata back into the EPUB file.
///
/// It is separate from [`LookupState`] because the file can fail to be written
/// long after the catalogue answered, and the panel says which of the two
/// happened.
#[derive(Default)]
pub enum SaveState {
    /// Nothing has been written for the selected book.
    #[default]
    Idle,
    /// The archive is being rebuilt and swapped in.
    Writing,
    /// The file now carries the catalogue metadata.
    Saved,
    /// The write failed; the message is shown verbatim.
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
    /// Scroll offset of the metadata panel. Separate from `scroll_handle`, so
    /// scrolling the metadata never moves the list.
    pub(crate) details_scroll: ScrollHandle,
    /// The book shown in the right-hand panel; `None` until a row is clicked.
    pub(crate) selected: Option<Selection>,
    /// Status of the BnF catalogue search for the selected book.
    pub(crate) lookup: LookupState,
    /// Status of writing that answer into the EPUB file itself.
    pub(crate) save: SaveState,
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
            selected: None,
            lookup: LookupState::Idle,
            save: SaveState::Idle,
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
            // new folder also empties the panel: the selected file may no
            // longer be in the new list.
            this.update(cx, |this, cx| {
                this.folder = Some(folder.clone());
                this.epubs.clear();
                this.selected = None;
                this.lookup = LookupState::Idle;
                this.save = SaveState::Idle;
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

    /// Selects `path` and starts reading its metadata into the right-hand panel.
    ///
    /// The archive is opened on the background executor because it touches the
    /// disk, and parsing XML is not free either. The panel appears immediately
    /// in its `Loading` state, then the same weak-entity pattern as
    /// [`Self::pick_folder`] fills it in.
    pub(crate) fn select_epub(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        // A fresh handle per book, so every one starts at the top.
        self.details_scroll = ScrollHandle::new();
        // The previous book's catalogue answer does not apply to this one.
        self.lookup = LookupState::Idle;
        self.save = SaveState::Idle;
        self.selected = Some(Selection {
            path: path.clone(),
            state: DetailsState::Loading,
        });
        // Show the loading panel before the read finishes.
        cx.notify();

        let read_path = path.clone();
        let task = cx.background_spawn(async move { read_epub(&read_path) });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                // Only fill in the panel if it still shows this file: the user
                // may have selected another book (or closed the panel) meanwhile.
                let still_selected = this
                    .selected
                    .as_ref()
                    .is_some_and(|selection| selection.path == path);
                if still_selected {
                    let state = match result {
                        Ok(details) => DetailsState::Ready(details),
                        Err(error) => {
                            DetailsState::Failed(format!("Could not read this EPUB: {error}."))
                        }
                    };
                    this.selected = Some(Selection { path, state });
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Event handler for the "×" button in the metadata panel: forgets the
    /// selected book so the panel shows its placeholder again.
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
    /// straight to `cx.listener(Self::clear_selection)` without allocating.
    pub(crate) fn clear_selection(
        &mut self,
        _event: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected = None;
        // The catalogue answer belongs to the panel being emptied.
        self.lookup = LookupState::Idle;
        self.save = SaveState::Idle;
        // THE key line: without `notify` the state changes but the screen keeps
        // showing the old frame.
        cx.notify();
    }

    /// Event handler for the "Search the BnF" button in the metadata panel.
    ///
    /// The search itself is a blocking HTTP request, so it follows the same
    /// shape as [`Self::select_epub`]: show the loading state, run the request
    /// on the background executor, then fill the panel in on the main thread.
    /// The title and author come from the EPUB metadata that is already
    /// displayed; the file name is used when the file advertises no title.
    ///
    /// When the catalogue answers, the best match replaces the file's metadata
    /// outright: its title, author, year and saga become the rows of the
    /// panel, and everything the EPUB claimed is dropped. The same values are
    /// then written into the EPUB itself on another background task, because
    /// the panel and the file must not disagree.
    pub(crate) fn lookup_book(
        &mut self,
        _event: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Copy what the search needs out of the selection first, so the borrow
        // of `self.selected` ends before the fields below are written.
        let prepared = match &self.selected {
            Some(Selection {
                path,
                state: DetailsState::Ready(details),
            }) => Some((path.clone(), details.title.clone(), details.creator.clone())),
            _ => None,
        };
        let Some((path, title, author)) = prepared else {
            return;
        };

        self.lookup = LookupState::Loading;
        self.save = SaveState::Idle;
        cx.notify();

        let task = cx.background_spawn(async move { search_books(&title, author.as_deref(), 5) });

        cx.spawn(async move |this, cx| {
            let outcome = task.await;

            // First the panel: apply the best match and remember what should go
            // into the file, if anything.
            let pending = this
                .update(cx, |this, cx| {
                    // Only fill in the panel if it still shows this book: the
                    // user may have selected another one (or closed the panel)
                    // meanwhile.
                    let still_selected = this
                        .selected
                        .as_ref()
                        .is_some_and(|selection| selection.path == path);
                    if !still_selected {
                        return None;
                    }

                    let to_write = match outcome {
                        Ok(results) => {
                            // `books` is sorted best-match first, so the first
                            // record is the one whose metadata replaces the
                            // file's own. An empty answer leaves the panel as
                            // the EPUB described it.
                            let best = results.books.first();
                            if let Some(record) = best
                                && let Some(Selection {
                                    state: DetailsState::Ready(details),
                                    ..
                                }) = this.selected.as_mut()
                            {
                                details.title = record.title.clone();
                                details.creator = record.author.clone();
                                details.entries = record.metadata_rows();
                            }
                            let to_write = best.map(|record| EpubMetadata {
                                title: record.title.clone(),
                                author: record.author.clone(),
                                published: record.date.clone(),
                                saga: record.saga.clone(),
                                tome: record.tome.clone(),
                            });
                            this.lookup = LookupState::Ready(results);
                            to_write
                        }
                        Err(error) => {
                            this.lookup = LookupState::Failed(format!(
                                "Could not search the BnF catalogue: {error}."
                            ));
                            None
                        }
                    };

                    this.save = if to_write.is_some() {
                        SaveState::Writing
                    } else {
                        SaveState::Idle
                    };
                    cx.notify();
                    to_write
                })
                .ok()
                .flatten();

            let Some(metadata) = pending else {
                return;
            };

            // Then the file: rewriting the whole archive is blocking IO, so it
            // goes to the background executor like the search did.
            let save_path = path.clone();
            let still_wanted = save_path.clone();
            let write = cx.background_spawn(async move { write_metadata(&save_path, &metadata) });
            let result = write.await;

            this.update(cx, |this, cx| {
                let still_selected = this
                    .selected
                    .as_ref()
                    .is_some_and(|selection| selection.path == still_wanted);
                if still_selected {
                    this.save = match result {
                        Ok(()) => SaveState::Saved,
                        Err(error) => SaveState::Failed(error),
                    };
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
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
/// The window keeps one root view, so the whole screen — both columns — is
/// described in one tree. All this impl does is give it the window's
/// background; the columns themselves live in [`crate::ui`].
impl Render for EpubBrowser {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            // Fill the whole window. `size_full()` == width:100% + height:100%.
            .size_full()
            // Flexbox layout, exactly like CSS: a column that fills the window.
            .flex()
            .flex_col()
            .p_4()
            .bg(rgb(theme::WINDOW))
            .text_color(rgb(theme::TEXT))
            .child(list::render(self, cx))
    }
}
