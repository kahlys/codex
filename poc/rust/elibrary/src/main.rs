//! A small **GPUI** app: a search bar that opens the native "pick a folder"
//! dialog, then lists every `.epub` file found in that folder and its
//! subfolders. Clicking one of those files opens a detail **page** (in the same
//! window) showing the metadata stored inside the EPUB itself, with a button to
//! go back to the list. The original counter lives in the footer of the list
//! page.
//!
//! Run it with:
//!
//! ```text
//! cargo run
//! ```
//!
//! ---------------------------------------------------------------------------
//! How the code is organised
//! ---------------------------------------------------------------------------
//!
//! ```text
//! src/main.rs            entry point: opens the window, nothing else
//! src/app.rs             state + event handlers ("controller")
//! src/ui/theme.rs        the colour palette
//! src/ui/components.rs   widgets shared by both pages
//! src/ui/list.rs         the list page
//! src/ui/details.rs      the metadata page
//! src/library/scan.rs    find .epub files under a folder
//! src/library/epub.rs    read metadata out of one EPUB
//! src/library/format.rs  sizes and timestamps as text
//! ```
//!
//! The three layers only point one way: `main` -> `app` -> `ui`, and
//! `app` -> `library`. A page module never touches the file system, and the
//! library never imports `gpui`, so each part can be read (or tested) alone.
//!
//! ---------------------------------------------------------------------------
//! The GPUI mental model, in five bullets
//! ---------------------------------------------------------------------------
//!
//! 1. **`Application`** owns the event loop. `Application::new()` builds one and
//!    `.run(callback)` starts it. The callback receives `&mut App`, which is the
//!    handle to everything (windows, entities, globals...).
//!
//! 2. **A window shows one root view.** `App::open_window(...)` creates an OS
//!    window and asks for the view that should fill it.
//!
//! 3. **A view is an `Entity<T>`** — a value of your own type with a `Render`
//!    implementation. GPUI owns the value; you get `&mut T` back inside
//!    callbacks. Think of it as an `Rc<RefCell<T>>` managed by the framework.
//!
//! 4. **Views are rebuilt from state on every frame.** `render` is called again
//!    whenever the entity is notified, and returns a *description* of the UI
//!    (an element tree) that GPUI lays out and paints. You never touch pixels
//!    or widgets directly.
//!
//! 5. **Mutating state is not enough — you must ask for a repaint** by calling
//!    `cx.notify()`. That is the single most important rule of GPUI: change the
//!    field, notify, and `render` runs again with the new value.
//!
//! ---------------------------------------------------------------------------
//! What happens at runtime
//! ---------------------------------------------------------------------------
//!
//! Clicking the bar at the top calls `App::prompt_for_paths` with
//! `directories: true`, which opens the platform's own folder chooser (on
//! Windows a standard "Select Folder" dialog). Its result arrives later on a
//! one-shot channel, so the handler spawns a task, awaits it, then walks the
//! directory tree on a background thread (never blocking the UI thread) and
//! stores the `.epub` files it found. Notifying the view makes `render` paint
//! the new list.
//!
//! Clicking a row switches the view's `screen` field from `Screen::List` to
//! `Screen::Details`. Both pages are drawn by the same `Render` impl: there is
//! one window and one root view, so a "page" is just which branch of state is
//! current. The EPUB file is a ZIP archive, so its metadata is read off the UI
//! thread with the `zip` crate and parsed with `roxmltree`. The "back" button
//! sets the screen back to `Screen::List`; the results list keeps its scroll
//! offset because that handle lives on the view.

mod app;
mod library;
mod ui;

// `prelude::*` brings in the traits whose methods are used below; `AppContext`
// (which provides `cx.new`) is one of them.
use gpui::{
    App, Application, Bounds, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, px, size,
};

use app::EpubBrowser;

/// Entry point. `Application::new()` prepares platforms (windowing, GPU,
/// clipboard, fonts...); `.run` blocks on the event loop for the app's lifetime.
fn main() {
    Application::new().run(|cx: &mut App| {
        // A window needs a rectangle before it is created.
        // `Bounds::centered(None, ...)` centres it on the primary display
        // (passing `Some(display_id)` would centre it on a given monitor).
        // Taller than the counter demo: a file list needs vertical room.
        let bounds = Bounds::centered(None, size(px(760.0), px(600.0)), cx);

        cx.open_window(
            WindowOptions {
                // Without this the window is sized by its content (often tiny).
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                // On platforms that support it, `titlebar.title` sets the title.
                // (The whole titlebar config is optional in this gpui version.)
                titlebar: Some(TitlebarOptions {
                    title: Some("GPUI EPUB Browser".into()),
                    ..Default::default()
                }),
                // `..Default::default()` keeps every other option at its
                // sensible default; add fields here as you need them.
                ..Default::default()
            },
            // Closure that builds the *root view* of the window. `cx.new` takes
            // ownership of our struct, wraps it in an `Entity<EpubBrowser>`, and
            // makes it the window's root. From now on GPUI calls
            // `Render::render` on it as needed.
            |_window, cx| cx.new(|_cx| EpubBrowser::new()),
        )
        // Opening a window can fail (e.g. no display available on CI), so the
        // result must be handled. Panicking with a message is fine for a demo.
        .expect("failed to open a window");

        // Bring the window to the front. Without it, a freshly launched app
        // can start behind other windows on some platforms.
        cx.activate(true);
    });
}
