//! Finding EPUB files on disk.

use std::path::{Path, PathBuf};

/// Walks `root` and every subfolder, returning all `.epub` files, sorted by
/// path. This is plain blocking `std::fs` code; it is called from a background
/// task so the UI thread never waits on the disk.
///
/// An iterative stack (not recursion) keeps deep trees from risking a stack
/// overflow, and unreadable entries are skipped instead of aborting the scan.
pub fn collect_epubs(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];

    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };

            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() && is_epub(&entry.path()) {
                found.push(entry.path());
            }
            // Symlinks are neither `is_dir` nor `is_file` here, so they are
            // skipped: following them could loop forever.
        }
    }

    // Case-insensitive sort so the order matches what a user expects on
    // Windows, where paths compare case-insensitively.
    found.sort_by_cached_key(|path| path.to_string_lossy().to_lowercase());
    found
}

/// True when the path ends in `.epub`, ignoring case (`.EPUB` counts too).
fn is_epub(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("epub"))
}
