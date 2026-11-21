//! The application state and the actions that change it.
//!
//! This is the "controller" layer, and it has two jobs only:
//!
//! * hold [`EpubBrowser`] — every field the window needs to draw a frame, plus
//!   which book's row is expanded;
//! * react to the user — pick a folder, expand a book, look it up in the BnF
//!   catalogue, collapse it again, clean the files.
//!
//! Notice what is *not* here: no element building (that is [`crate::ui`]) and no
//! file-system or ZIP code (that is [`crate::library`]). The handlers orchestrate
//! the two: they update state, ask the library to do blocking work on a
//! background task, and call `cx.notify()` so the UI redraws.
//!
//! Blocking work never runs on the UI thread. Both long operations follow the
//! same shape: show a loading state, `background_spawn` the work, then update
//! the state again on the main thread when the task resolves.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use gpui::{
    ClickEvent, Context, IntoElement, PathPromptOptions, Render, ScrollHandle, Window, div,
    prelude::*, rgb,
};

use crate::library::{
    BookDetails, CatalogueResults, EpubMetadata, clean_epub, collect_epubs, is_cleaned, read_epub,
    search_books, write_metadata,
};
use crate::ui::{list, theme};

/// The book whose row is expanded, plus the progress of reading it.
///
/// The window is one screen holding one list, so "which book is expanded" is
/// the only navigation state there is: `None` means every row is collapsed.
/// Clicking a row toggles it, which is why [`EpubBrowser::select_epub`] can
/// close as well as open.
pub struct Selection {
    /// The `.epub` file whose row is expanded.
    pub path: PathBuf,
    /// How far reading it has got: still reading, parsed, or failed.
    pub state: DetailsState,
}

/// Phase of the metadata read shown inside the expanded row.
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
    /// first record is the one already written into the file.
    Ready(CatalogueResults),
    /// The request failed; the message is shown verbatim.
    Failed(String),
}

/// Phase of writing the catalogue metadata back into the EPUB file.
///
/// It is separate from [`LookupState`] because the file can fail to be written
/// long after the catalogue answered, and the panel says which of the two
/// happened. Success means the file has been written *and* read back, so the
/// panel shows the rows the EPUB now stores.
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

/// What the list shows next to a book while a "Clean all" run is in progress.
///
/// A book that succeeded needs no entry: it goes into the `cleaned` set and the
/// row draws its green check from that, exactly like a book cleaned in an
/// earlier session.
pub enum CleanMark {
    /// The run is working on this book right now.
    Working,
    /// The lookup or the write failed; the row shows a red cross until the next
    /// run (or another folder) clears the marks.
    Failed,
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
    /// The folder the user picked, if any. Shown in the search bar.
    pub(crate) folder: Option<PathBuf>,
    /// Every `.epub` file under `folder` (including subfolders), sorted.
    pub(crate) epubs: Vec<PathBuf>,
    /// The books this app already rewrote, read from the marker entry inside
    /// each file. The list shows a green check next to these.
    pub(crate) cleaned: HashSet<PathBuf>,
    /// True while the list's "Clean all" button is working through the files.
    pub(crate) cleaning: bool,
    /// The books of the current run that are being worked on or failed. Books
    /// missing from this map are simply not part of a run in progress.
    pub(crate) clean_marks: HashMap<PathBuf, CleanMark>,
    /// One-line message under the search bar: progress, result count, errors.
    pub(crate) status: String,
    /// True while a scan is in flight, so the UI can say what it is doing.
    pub(crate) scanning: bool,
    /// Scroll offset of the results list. It must live on the view: a
    /// `ScrollHandle` built inside `render` would reset on every frame.
    pub(crate) scroll_handle: ScrollHandle,
    /// The book whose row is expanded; `None` while every row is collapsed.
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
            folder: None,
            epubs: Vec::new(),
            cleaned: HashSet::new(),
            cleaning: false,
            clean_marks: HashMap::new(),
            status: "Pick a folder to list the .epub files inside it.".to_string(),
            scanning: false,
            scroll_handle: ScrollHandle::new(),
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
            // new folder also folds away the expanded row: the selected file
            // may no longer be in the new list.
            this.update(cx, |this, cx| {
                this.folder = Some(folder.clone());
                this.epubs.clear();
                // The checks belong to the old list; the scan below fills them
                // in again for the new one.
                this.cleaned.clear();
                this.cleaning = false;
                this.clean_marks.clear();
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
                this.epubs = epubs.clone();
                cx.notify();
            })
            .ok();

            // Which of them were already cleaned can only be told by opening
            // each archive, so that runs on the background executor as well and
            // puts the green checks in place when it is done.
            let check = cx.background_spawn(async move {
                epubs
                    .into_iter()
                    .filter(|path| is_cleaned(path))
                    .collect::<HashSet<PathBuf>>()
            });
            let cleaned = check.await;
            this.update(cx, |this, cx| {
                this.cleaned = cleaned;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Expands the row of `path` and starts reading its metadata, or collapses
    /// that row when it is already the expanded one.
    ///
    /// The list is the whole screen, so clicking a file name is a toggle: the
    /// row grows to show everything the EPUB stores, and clicking it again puts
    /// it back. One handler for both means `selected` stays the only navigation
    /// state there is.
    ///
    /// The archive is opened on the background executor because it touches the
    /// disk, and parsing XML is not free either. The row expands immediately in
    /// its `Loading` state, then the same weak-entity pattern as
    /// [`Self::pick_folder`] fills it in.
    pub(crate) fn select_epub(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        // Clicking the row that is already open closes it again.
        if self
            .selected
            .as_ref()
            .is_some_and(|selection| selection.path == path)
        {
            self.selected = None;
            // The catalogue answer belongs to the row being folded away.
            self.lookup = LookupState::Idle;
            self.save = SaveState::Idle;
            cx.notify();
            return;
        }

        // The previous book's catalogue answer does not apply to this one.
        self.lookup = LookupState::Idle;
        self.save = SaveState::Idle;
        self.selected = Some(Selection {
            path: path.clone(),
            state: DetailsState::Loading,
        });
        // Show the loading row before the read finishes.
        cx.notify();

        let read_path = path.clone();
        let task = cx.background_spawn(async move { read_epub(&read_path) });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                // Only fill in the row if it is still the expanded one: the
                // user may have selected another book (or folded this one
                // away) meanwhile.
                let still_selected = this
                    .selected
                    .as_ref()
                    .is_some_and(|selection| selection.path == path);
                if still_selected {
                    let state = match result {
                        Ok(details) => {
                            // The file itself says whether it was cleaned, which
                            // keeps the list's check honest even when the file
                            // was replaced by another tool meanwhile.
                            if details.cleaned.is_some() {
                                this.cleaned.insert(path.clone());
                                this.clean_marks.remove(&path);
                            } else {
                                this.cleaned.remove(&path);
                            }
                            DetailsState::Ready(details)
                        }
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

    /// Event handler for the "×" button of an expanded row: forgets the
    /// selected book so the row folds back to its name line.
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

    /// Event handler for the "Clean EPUB" button in an expanded row.
    ///
    /// The search itself is a blocking HTTP request, so it follows the same
    /// shape as [`Self::select_epub`]: show the loading state, run the request
    /// on the background executor, then fill the panel in on the main thread.
    /// The title and author come from the EPUB metadata that is already
    /// displayed. A title that is only an approximation of the book's name often
    /// finds nothing in the catalogue, so the search is retried with the other
    /// name the book goes by — the file name — when the first one is empty.
    ///
    /// When the catalogue answers, the best match is written into the EPUB
    /// itself on another background task: its title, author, year and saga
    /// replace the file's own, and the mandatory entries the format cannot do
    /// without (identifier, language, cover pointer, modification date) are
    /// kept. The row is then filled from the file that was just written, not
    /// from the catalogue answer, so it shows exactly what the book stores now.
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
            }) => {
                // Every name the book goes by, best first: the title its own
                // metadata advertises, the title the panel shows (that same
                // title, or the file name when the file names none), and the
                // bare file name. Duplicates are dropped, so the usual case is
                // still a single query.
                let mut titles: Vec<String> = Vec::new();
                let file_stem = path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned());
                for candidate in [
                    details.metadata_title.as_deref(),
                    Some(details.title.as_str()),
                    file_stem.as_deref(),
                ] {
                    let Some(title) = candidate.map(str::trim).filter(|title| !title.is_empty())
                    else {
                        continue;
                    };
                    if !titles.iter().any(|known| known == title) {
                        titles.push(title.to_string());
                    }
                }
                Some((path.clone(), titles, details.creator.clone()))
            }
            _ => None,
        };
        let Some((path, titles, author)) = prepared else {
            return;
        };

        self.lookup = LookupState::Loading;
        self.save = SaveState::Idle;
        cx.notify();

        let task =
            cx.background_spawn(async move { search_books(&titles, author.as_deref(), 5) });

        cx.spawn(async move |this, cx| {
            let outcome = task.await;

            // First the row: apply the best match and remember what should go
            // into the file, if anything.
            let pending = this
                .update(cx, |this, cx| {
                    // Only fill in the row if it is still the expanded one: the
                    // user may have selected another book (or folded this one
                    // away) meanwhile.
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
                            // file's own. An empty answer leaves the file as
                            // the EPUB described it.
                            //
                            // The row is deliberately not rewritten here: it
                            // only ever shows what the file really contains,
                            // so it is refreshed by re-reading the file once
                            // the write below has landed (see the task below).
                            let best = results.books.first();
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
                                "Could not clean the EPUB: {error}."
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
            // goes to the background executor like the search did. The rewritten
            // file is read back straight away, because the panel must show the
            // metadata the file really holds — the same rows the book shows
            // when it is selected again — rather than the values we asked for.
            let save_path = path.clone();
            let still_wanted = save_path.clone();
            let write = cx.background_spawn(async move {
                match write_metadata(&save_path, &metadata) {
                    Ok(()) => read_epub(&save_path)
                        .map_err(|error| format!("It was written, but could not be read back ({error})")),
                    Err(error) => Err(format!("It could not be written into the file: {error}")),
                }
            });
            let result = write.await;

            this.update(cx, |this, cx| {
                let still_selected = this
                    .selected
                    .as_ref()
                    .is_some_and(|selection| selection.path == still_wanted);
                if still_selected {
                    match result {
                        Ok(details) => {
                            this.save = SaveState::Saved;
                            // The list marks every file this app has cleaned.
                            this.cleaned.insert(still_wanted.clone());
                            this.clean_marks.remove(&still_wanted);
                            // The file is the only source of the panel: the
                            // rows are now literally the ones it stores.
                            if let Some(selection) = this.selected.as_mut() {
                                selection.state = DetailsState::Ready(details);
                            }
                        }
                        Err(error) => this.save = SaveState::Failed(error),
                    }
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Event handler for the "Clean all" button above the list.
    ///
    /// It does an expanded row's work once per file: search the catalogue,
    /// write the best match into the EPUB, read it back. Each book is handled on
    /// the background executor and the list is repainted as soon as one is done,
    /// so the green checks appear one at a time. Requests are made one after the
    /// other, which is both what the public catalogue expects and what keeps a
    /// large folder from hammering it.
    ///
    /// Books that already show the green check are skipped: their file carries
    /// the marker this app wrote, so querying the catalogue again would only
    /// spend requests to write the same metadata a second time. The status line
    /// says how many were left alone, because a run that quietly does nothing
    /// would look broken.
    ///
    /// The status line reports progress; a book that fails is counted and the
    /// first failure is named, because a folder often holds a few files the
    /// catalogue simply does not know.
    pub(crate) fn clean_all(
        &mut self,
        _event: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // One run at a time: a second click while the first is in flight would
        // rewrite the same files twice.
        if self.cleaning || self.scanning || self.epubs.is_empty() {
            return;
        }

        // Marks belong to one run only: a cross from the previous run must not
        // sit next to a book this run has not reached yet.
        self.clean_marks.clear();

        // What is left once the already-cleaned books are taken out. The
        // set is the same one the green checks are drawn from, so the button
        // and the list can never disagree about what still needs a lookup.
        let skipped = self
            .epubs
            .iter()
            .filter(|path| self.cleaned.contains(*path))
            .count();
        let pending: Vec<PathBuf> = self
            .epubs
            .iter()
            .filter(|path| !self.cleaned.contains(*path))
            .cloned()
            .collect();

        if pending.is_empty() {
            self.status = match skipped {
                1 => "The only book in the list is already cleaned.".to_string(),
                n => format!("All {n} books are already cleaned; nothing to do."),
            };
            cx.notify();
            return;
        }

        let total = pending.len();
        // A short reminder in every progress line, so the gap between "12 books
        // in the list" and "0 of 9" is explained while the run is going on.
        let hint = if skipped > 0 {
            format!(" ({skipped} already cleaned, skipped)")
        } else {
            String::new()
        };
        self.cleaning = true;
        self.status = format!("Cleaning 0 of {total} books…{hint}");
        cx.notify();

        cx.spawn(async move |this, cx| {
            let mut cleaned = 0usize;
            let mut failures = 0usize;
            let mut first_failure: Option<String> = None;

            for (index, path) in pending.into_iter().enumerate() {
                let progress = format!("Cleaning {} of {total} books…{hint}", index + 1);

                // Flag the book as being worked on before it is read: a slow
                // catalogue answer is then visible in the list, not only in the
                // status line.
                let working = path.clone();
                let started = progress.clone();
                this.update(cx, |this, cx| {
                    this.clean_marks.insert(working, CleanMark::Working);
                    this.status = started;
                    cx.notify();
                })
                .ok();

                let target = path.clone();
                let result = cx
                    .background_spawn(async move { clean_epub(&target) })
                    .await;

                this.update(cx, |this, cx| {
                    match result {
                        Ok(details) => {
                            cleaned += 1;
                            this.clean_marks.remove(&path);
                            this.cleaned.insert(path.clone());
                            // A cleaned book that is on screen is refreshed
                            // from the copy that was just written: the expanded
                            // row only ever shows what the file holds.
                            if let Some(selection) = this.selected.as_mut()
                                && selection.path == path
                            {
                                selection.state = DetailsState::Ready(details);
                            }
                        }
                        Err(error) => {
                            failures += 1;
                            this.clean_marks.insert(path.clone(), CleanMark::Failed);
                            if first_failure.is_none() {
                                let name = path
                                    .file_name()
                                    .map(|name| name.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| path.display().to_string());
                                first_failure = Some(format!("{name}: {error}"));
                            }
                        }
                    }
                    this.status = progress;
                    cx.notify();
                })
                .ok();
            }

            this.update(cx, |this, cx| {
                this.cleaning = false;
                let mut status = match (cleaned, failures) {
                    (_, 0) => format!("Cleaned {cleaned} of {total} books."),
                    (0, _) => format!(
                        "No book could be cleaned ({}).",
                        first_failure.unwrap_or_default()
                    ),
                    (cleaned, failures) => format!(
                        "Cleaned {cleaned} of {total} books; {failures} failed ({}).",
                        first_failure.unwrap_or_default()
                    ),
                };
                match skipped {
                    0 => {}
                    1 => status.push_str(" 1 book was already cleaned, so it was skipped."),
                    skipped => status.push_str(&format!(
                        " {skipped} books were already cleaned, so they were skipped."
                    )),
                }
                this.status = status;
                cx.notify();
            })
            .ok();
        })
        .detach();
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
/// The window keeps one root view, so the whole screen — the search bar, the
/// list, the footer — is described in one tree. All this impl does is give it
/// the window's background; the screen itself lives in [`crate::ui`].
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
