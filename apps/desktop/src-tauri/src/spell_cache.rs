//! What spell check keeps in memory: loaded dictionaries, and the words
//! of recently opened books and of the person's notes. Words are learned
//! in the background, so checking never waits for a big book to be read.

use libreri_library::Library;
use libreri_spell::{Loaded, Vocab};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Books whose words are kept.
const BOOKS_KEPT: usize = 4;
/// How long the words of the notes are used before being learned again.
const NOTES_FRESH: Duration = Duration::from_secs(300);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

#[derive(Default)]
pub struct SpellCache {
    pub dir: PathBuf,
    dicts: Mutex<HashMap<String, Arc<Loaded>>>,
    books: Mutex<Vec<(String, Arc<Vocab>)>>,
    notes: Mutex<Option<(String, Instant, Arc<Vocab>)>>,
    /// Books and notes being learned right now.
    learning: Mutex<HashSet<String>>,
    pub downloads: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl SpellCache {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            ..Default::default()
        }
    }

    /// The chosen dictionaries that can be used (others are skipped).
    pub fn dictionaries(&self, codes: &[String]) -> Vec<Arc<Loaded>> {
        let mut out = Vec::new();
        for code in codes.iter().take(4) {
            if let Some(d) = lock(&self.dicts).get(code) {
                out.push(Arc::clone(d));
                continue;
            }
            if let Ok(d) = libreri_spell::load(&self.dir, code) {
                let d = Arc::new(d);
                lock(&self.dicts).insert(code.clone(), Arc::clone(&d));
                out.push(d);
            }
        }
        out
    }

    /// Forgets a dictionary (it was removed).
    pub fn forget(&self, code: &str) {
        lock(&self.dicts).remove(code);
    }

    fn start_learning(self: &Arc<Self>, key: String, work: impl FnOnce() + Send + 'static) {
        if !lock(&self.learning).insert(key.clone()) {
            return;
        }
        let me = Arc::clone(self);
        std::thread::spawn(move || {
            work();
            lock(&me.learning).remove(&key);
        });
    }

    /// The words of a book, if learned; otherwise starts learning them.
    pub fn book(self: &Arc<Self>, library: &Arc<Library>, id: &str) -> Option<Arc<Vocab>> {
        {
            let mut books = lock(&self.books);
            if let Some(i) = books.iter().position(|(k, _)| k == id) {
                let entry = books.remove(i);
                let v = Arc::clone(&entry.1);
                books.push(entry);
                return Some(v);
            }
        }
        let Ok(book_id) = libreri_core::BookId::from_hex(id) else {
            return None;
        };
        let (me, lib, key) = (Arc::clone(self), Arc::clone(library), id.to_owned());
        self.start_learning(format!("book:{id}"), move || {
            let text = lib.book_words(&book_id).unwrap_or_default();
            let v = Arc::new(Vocab::learn([text.as_str()]));
            let mut books = lock(&me.books);
            books.retain(|(k, _)| k != &key);
            books.push((key, v));
            let extra = books.len().saturating_sub(BOOKS_KEPT);
            books.drain(..extra);
        });
        None
    }

    /// The words of the profile's notes (learned again every few minutes).
    pub fn notes(self: &Arc<Self>, library: &Arc<Library>) -> Option<Arc<Vocab>> {
        let who = format!(
            "{}|{}",
            library.layout().root().display(),
            library.profile().map(|p| p.to_string()).unwrap_or_default()
        );
        let current = lock(&self.notes).clone();
        let fresh = current
            .as_ref()
            .is_some_and(|(k, at, _)| k == &who && at.elapsed() < NOTES_FRESH);
        if !fresh {
            let (me, lib, key) = (Arc::clone(self), Arc::clone(library), who.clone());
            self.start_learning(format!("notes:{who}"), move || {
                let texts = lib.note_texts().unwrap_or_default();
                let v = Arc::new(Vocab::learn(texts.iter().map(String::as_str)));
                *lock(&me.notes) = Some((key, Instant::now(), v));
            });
        }
        current.filter(|(k, _, _)| k == &who).map(|(_, _, v)| v)
    }
}
