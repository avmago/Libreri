//! Watches `Books/` for changes made outside Libreri (file manager, sync
//! clients, other apps) and asks for a scan once things settle.

use notify_debouncer_full::notify::{EventKind, RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};
use std::path::Path;
use std::time::Duration;

/// Wait this long after the last change before scanning, so copying a big
/// folder triggers one scan, not hundreds.
pub const SETTLE: Duration = Duration::from_millis(1500);

/// Stops watching when dropped.
pub struct LibraryWatcher {
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
}

impl LibraryWatcher {
    /// Starts watching `books_dir` recursively. `on_change` runs on the
    /// watcher's thread after changes have settled.
    pub fn start(
        books_dir: &Path,
        on_change: impl Fn() + Send + 'static,
    ) -> notify_debouncer_full::notify::Result<Self> {
        let mut debouncer = new_debouncer(SETTLE, None, move |result: DebounceEventResult| {
            let relevant = match result {
                Ok(events) => events.iter().any(|e| {
                    !matches!(e.kind, EventKind::Access(_))
                        && e.paths.iter().all(|p| {
                            !p.file_name()
                                .is_some_and(|n| n.to_string_lossy().starts_with('.'))
                        })
                }),
                // The watcher lost track (e.g. too many changes): scan anyway.
                Err(_) => true,
            };
            if relevant {
                on_change();
            }
        })?;
        debouncer.watch(books_dir, RecursiveMode::Recursive)?;
        Ok(Self {
            _debouncer: debouncer,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn reports_new_files_once_settled() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = mpsc::channel();
        let tx = std::sync::Mutex::new(tx);
        let _w = LibraryWatcher::start(dir.path(), move || {
            let _ = tx.lock().unwrap().send(());
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        for i in 0..5 {
            std::fs::write(dir.path().join(format!("{i}.pdf")), "x").unwrap();
        }
        rx.recv_timeout(Duration::from_secs(10))
            .expect("a change notification");
    }
}
