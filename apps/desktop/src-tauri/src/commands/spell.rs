//! Spell check and word suggestions (Phase 7c): dictionaries, checking,
//! corrections, completing words, and the person's own dictionary.

use crate::error::{AppError, AppErrorKind, AppResult};
use crate::events::DictionaryDownload;
use crate::state::AppState;
use libreri_spell::{Checker, DictionaryInfo, Miss, Sources, Vocab};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::new(AppErrorKind::Io, e.to_string()))?
}

/// Longest text checked at once (a very long notebook is checked in part).
const LONGEST: usize = 400_000;

/// Everything needed to check or complete: dictionaries, own words, the
/// open book's and the notes' words (when learned).
struct Context {
    dicts: Vec<Arc<libreri_spell::Loaded>>,
    own: Vec<String>,
    own_set: HashSet<String>,
    book: Option<Arc<Vocab>>,
    notes: Option<Arc<Vocab>>,
}

fn context(app: &AppHandle, languages: &[String], book: Option<&str>, notes: bool) -> Context {
    let state = app.state::<AppState>();
    let dicts = state.spell.dictionaries(languages);
    let library = state.library_if_open();
    let own = library
        .as_ref()
        .and_then(|l| l.own_words().ok())
        .unwrap_or_default();
    let own_set = own.iter().map(|w| w.to_lowercase()).collect();
    let book = library
        .as_ref()
        .zip(book)
        .and_then(|(l, id)| state.spell.book(l, id));
    let notes = if notes {
        library.as_ref().and_then(|l| state.spell.notes(l))
    } else {
        None
    };
    Context {
        dicts,
        own,
        own_set,
        book,
        notes,
    }
}

#[tauri::command]
#[specta::specta]
pub fn spell_dictionaries(state: State<'_, AppState>) -> Vec<DictionaryInfo> {
    libreri_spell::dictionaries(&state.spell.dir)
}

/// Downloads a dictionary; progress arrives as `DictionaryDownload`.
#[tauri::command]
#[specta::specta]
pub async fn download_dictionary(
    app: AppHandle,
    state: State<'_, AppState>,
    code: String,
) -> AppResult<Vec<DictionaryInfo>> {
    let dir = state.spell.dir.clone();
    let cancel: Arc<AtomicBool> = Arc::default();
    {
        let mut running = state.spell.downloads.lock().expect("downloads lock");
        if running.contains_key(&code) {
            return Err(AppError::invalid("that dictionary is already downloading"));
        }
        running.insert(code.clone(), Arc::clone(&cancel));
    }
    let c = code.clone();
    let result = blocking(move || {
        let mut last = std::time::Instant::now();
        let result = libreri_spell::catalog::download(&dir, &c, &cancel, |done, total| {
            if last.elapsed().as_millis() > 200 {
                last = std::time::Instant::now();
                let _ = DictionaryDownload {
                    code: c.clone(),
                    done: done as f64,
                    total: Some(total as f64),
                    finished: false,
                    error: None,
                }
                .emit(&app);
            }
        });
        let _ = DictionaryDownload {
            code: c.clone(),
            done: 0.0,
            total: None,
            finished: true,
            error: result.as_ref().err().cloned(),
        }
        .emit(&app);
        result.map_err(AppError::invalid)
    })
    .await;
    state
        .spell
        .downloads
        .lock()
        .expect("downloads lock")
        .remove(&code);
    result?;
    Ok(libreri_spell::dictionaries(&state.spell.dir))
}

#[tauri::command]
#[specta::specta]
pub fn cancel_dictionary_download(state: State<'_, AppState>, code: String) {
    if let Some(c) = state
        .spell
        .downloads
        .lock()
        .expect("downloads lock")
        .get(&code)
    {
        c.store(true, Ordering::SeqCst);
    }
}

#[tauri::command]
#[specta::specta]
pub fn remove_dictionary(
    state: State<'_, AppState>,
    code: String,
) -> AppResult<Vec<DictionaryInfo>> {
    state.spell.forget(&code);
    libreri_spell::catalog::remove(&state.spell.dir, &code).map_err(AppError::invalid)?;
    Ok(libreri_spell::dictionaries(&state.spell.dir))
}

/// The words of `text` that look misspelt, in the chosen languages. The
/// open book's names and terms (`book`) are accepted once they are learned.
#[tauri::command]
#[specta::specta]
pub async fn spell_check(
    app: AppHandle,
    text: String,
    languages: Vec<String>,
    book: Option<String>,
) -> AppResult<Vec<Miss>> {
    blocking(move || {
        let cx = context(&app, &languages, book.as_deref(), false);
        let mut end = text.len().min(LONGEST);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        let checker = Checker {
            dicts: &cx.dicts,
            own: &cx.own_set,
            book: cx.book.as_deref(),
        };
        Ok(checker.misses(&text[..end]))
    })
    .await
}

/// Corrections for a word.
#[tauri::command]
#[specta::specta]
pub async fn spell_suggest(
    app: AppHandle,
    word: String,
    languages: Vec<String>,
    book: Option<String>,
) -> AppResult<Vec<String>> {
    blocking(move || {
        let cx = context(&app, &languages, book.as_deref(), false);
        let checker = Checker {
            dicts: &cx.dicts,
            own: &cx.own_set,
            book: cx.book.as_deref(),
        };
        Ok(checker.suggest(&word, 6))
    })
    .await
}

/// Ways to finish a word being typed: the person's own words, then the
/// open book's, their notes' and the dictionaries'.
#[tauri::command]
#[specta::specta]
pub async fn spell_complete(
    app: AppHandle,
    prefix: String,
    languages: Vec<String>,
    book: Option<String>,
) -> AppResult<Vec<String>> {
    blocking(move || {
        let cx = context(&app, &languages, book.as_deref(), true);
        let stems: Vec<&Vocab> = cx.dicts.iter().map(|d| &d.stems).collect();
        let sources = Sources {
            own: &cx.own,
            book: cx.book.as_deref(),
            notes: cx.notes.as_deref(),
            dictionaries: &stems,
        };
        Ok(libreri_spell::complete(&prefix, &sources, 5))
    })
    .await
}

/// Starts learning a book's words (when it opens), so checking and
/// completing know its names and terms.
#[tauri::command]
#[specta::specta]
pub fn spell_learn_book(state: State<'_, AppState>, book: String) {
    if let Some(l) = state.library_if_open() {
        let _ = state.spell.book(&l, &book);
        let _ = state.spell.notes(&l);
    }
}

#[tauri::command]
#[specta::specta]
pub fn own_words(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    Ok(state.library()?.own_words()?)
}

#[tauri::command]
#[specta::specta]
pub fn add_own_word(state: State<'_, AppState>, word: String) -> AppResult<Vec<String>> {
    Ok(state.library()?.add_own_word(&word)?)
}

#[tauri::command]
#[specta::specta]
pub fn remove_own_word(state: State<'_, AppState>, word: String) -> AppResult<Vec<String>> {
    Ok(state.library()?.remove_own_word(&word)?)
}
