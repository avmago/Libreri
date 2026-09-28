//! Shared app state held by Tauri.

use crate::backup_store::{BackupSettings, BackupStore};
use crate::error::{AppError, AppErrorKind, AppResult};
use crate::events::{
    ArchiveImported, AutoSyncFinished, BackupFinished, CompareFinished, DetailsFilled,
    ExportFinished, ForeignImported, ImportFinished, JobEventPayload, LibraryChanged, OcrFinished,
    SearchIndexProgress,
};
use crate::online_store::OnlineStore;
use crate::settings_store::SettingsStore;
use libreri_core::AppSettings;
use libreri_core::BookId;
use libreri_jobs::{JobContext, JobError, JobId, JobQueue};
use libreri_library::{
    ArchiveImport, ExportRequest, ImportRequest, Library, LibraryWatcher, OcrOptions, Progress,
    SearchIndex,
};
use libreri_metadata::Http;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

/// Largest page cache kept between runs.
pub const PAGE_CACHE_LIMIT: u64 = 2 * 1024 * 1024 * 1024;

pub struct AppState {
    pub settings: Mutex<AppSettings>,
    pub settings_store: SettingsStore,
    /// Online sources and API keys (per computer, never exported).
    pub online: Mutex<libreri_metadata::Settings>,
    pub online_store: OnlineStore,
    /// Backups and files kept up to date, per library (per computer).
    pub backups: BackupStore,
    /// Page images of comics and DjVu books (this computer only).
    pub page_cache: std::path::PathBuf,
    /// Search indexes, one per library (this computer only).
    search_dir: std::path::PathBuf,
    /// The open library's search index.
    search: Mutex<Option<Arc<SearchIndex>>>,
    /// Indexing in the background: asked for, and running.
    index_wanted: Arc<AtomicBool>,
    index_running: Arc<AtomicBool>,
    /// OCR language files downloaded by Libreri (this computer only).
    pub tessdata: std::path::PathBuf,
    /// Photos of paper notes being captured (id → file in the library's
    /// scratch folder).
    pub capture_photos: Mutex<std::collections::HashMap<String, std::path::PathBuf>>,
    /// Extra canvas fonts downloaded by Libreri (this computer only).
    pub extras_dir: std::path::PathBuf,
    /// Extra font downloads running, to cancel them.
    pub font_downloads: Mutex<std::collections::HashMap<String, Arc<AtomicBool>>>,
    /// The maths model (app data `maths/`), loaded once when used, and
    /// its download, to cancel it.
    pub maths_dir: std::path::PathBuf,
    pub maths_reader: Mutex<Option<Arc<libreri_maths::Reader>>>,
    pub maths_download: Mutex<Option<Arc<AtomicBool>>>,
    /// Speech models downloaded by Libreri (this computer only).
    pub whisper_dir: std::path::PathBuf,
    /// Spell check: dictionaries (downloaded into app data `dictionaries/`)
    /// and words learned from books and notes.
    pub spell: Arc<crate::spell_cache::SpellCache>,
    /// The speech model in use, loaded once.
    transcriber: Mutex<Option<Arc<libreri_speech::Transcriber>>>,
    /// Model downloads running, to cancel them.
    pub model_downloads: Mutex<std::collections::HashMap<String, Arc<AtomicBool>>>,
    /// A backup is running; the schedule does not start another.
    backup_running: Arc<AtomicBool>,
    /// Shared by all lookups, so connections are reused.
    pub http: Arc<dyn Http + Send>,
    /// The phone scanning page, while it is open.
    phone: Mutex<Option<libreri_scan::PhoneScanner>>,
    /// Speaking with eSpeak NG (read aloud where the system has no voices).
    pub speaker: Arc<libreri_helpers::speech::Speaker>,
    /// Comparisons open in the interface (newest last, a few kept).
    compares: Mutex<Vec<(String, Arc<CompareSession>)>>,
    library: Mutex<Option<Arc<Library>>>,
    watcher: Mutex<Option<LibraryWatcher>>,
    pub jobs: JobQueue,
    /// A scan is queued and has not started yet; further requests are merged.
    scan_queued: Arc<AtomicBool>,
    app: AppHandle,
}

/// Checks every five minutes (and a minute after starting) whether a
/// backup or an auto-export is due.
pub fn start_scheduler(app: AppHandle) {
    std::thread::Builder::new()
        .name("libreri-scheduler".into())
        .spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(60));
            loop {
                match app.try_state::<AppState>() {
                    Some(state) => state.run_due_tasks(),
                    None => return,
                }
                std::thread::sleep(std::time::Duration::from_secs(300));
            }
        })
        .expect("the scheduler thread starts");
}

/// Page pictures of a comparison: (side, page, width) → JPEG.
type PictureCache = std::collections::HashMap<(u8, u32, u32), Arc<Vec<u8>>>;

/// Two documents being compared, and the result once it is ready.
pub struct CompareSession {
    pub a: libreri_library::CompareDoc,
    pub b: libreri_library::CompareDoc,
    pub result: Mutex<Option<Result<libreri_library::Comparison, String>>>,
    /// Page pictures already drawn: (side, page, width) → JPEG.
    pub images: Mutex<PictureCache>,
}

impl CompareSession {
    /// A page of one side as a JPEG (kept for the next time).
    pub fn picture(&self, side: u8, page: u32, width: u32) -> Result<Arc<Vec<u8>>, String> {
        let key = (side, page, width);
        if let Some(hit) = lock(&self.images).get(&key) {
            return Ok(Arc::clone(hit));
        }
        let doc = if side == 0 { &self.a } else { &self.b };
        if page == 0 || page > doc.pages() {
            return Err("no such page".into());
        }
        let (jpeg, _, _) = doc.jpeg(page, width).map_err(|e| e.to_string())?;
        let jpeg = Arc::new(jpeg);
        let mut images = lock(&self.images);
        // About 200 pictures at most.
        if images.len() > 200 {
            images.clear();
        }
        images.insert(key, Arc::clone(&jpeg));
        Ok(jpeg)
    }
}

/// Comparisons kept for the interface (each holds two open documents).
const COMPARES_KEPT: usize = 4;

/// Adapts a job's context to the library's progress interface.
struct JobProgress<'a>(&'a JobContext);

impl Progress for JobProgress<'_> {
    fn report(&self, done: u64, total: u64, message: &str) {
        self.0.progress(
            done,
            total,
            (!message.is_empty()).then(|| message.to_owned()),
        );
    }
    fn cancelled(&self) -> bool {
        self.0.check_cancelled().is_err()
    }
}

/// Reports indexing to the interface a few times a second, and stops when
/// the library is closed.
struct IndexProgress {
    handle: AppHandle,
    library: std::sync::Weak<Library>,
    last: Mutex<std::time::Instant>,
}

impl Progress for IndexProgress {
    fn report(&self, done: u64, total: u64, _message: &str) {
        let mut last = lock(&self.last);
        if done == 0 || done >= total || last.elapsed().as_millis() > 400 {
            *last = std::time::Instant::now();
            let _ = SearchIndexProgress {
                done: done as u32,
                total: total as u32,
                running: true,
            }
            .emit(&self.handle);
        }
    }
    fn cancelled(&self) -> bool {
        let open = self
            .handle
            .try_state::<AppState>()
            .and_then(|s| s.library_if_open());
        match (open, self.library.upgrade()) {
            (Some(a), Some(b)) => !Arc::ptr_eq(&a, &b),
            _ => true,
        }
    }
}

/// Job progress with the book's title in front, for OCR of many books.
struct OcrProgress<'a> {
    ctx: &'a JobContext,
    prefix: String,
}

impl Progress for OcrProgress<'_> {
    fn report(&self, done: u64, total: u64, message: &str) {
        self.ctx
            .progress(done, total, Some(format!("{}{message}", self.prefix)));
    }
    fn cancelled(&self) -> bool {
        self.ctx.check_cancelled().is_err()
    }
}

fn job_error(e: libreri_library::Error) -> JobError {
    match e {
        libreri_library::Error::Cancelled => JobError::Cancelled(libreri_jobs::Cancelled),
        other => JobError::Failed(other.to_string()),
    }
}

/// The web client for online details. Debug builds can answer from files
/// instead (`LIBRERI_HTTP_FIXTURES`), to try the interface offline.
fn make_http() -> Arc<dyn Http + Send> {
    #[cfg(debug_assertions)]
    if let Some(dir) = std::env::var_os("LIBRERI_HTTP_FIXTURES") {
        match libreri_metadata::FixtureHttp::new(std::path::Path::new(&dir)) {
            Ok(h) => return Arc::new(h),
            Err(e) => eprintln!("Libreri: cannot use the HTTP fixtures: {e}"),
        }
    }
    Arc::new(libreri_metadata::UreqHttp::default())
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl AppState {
    pub fn initialise(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let config_dir = app.path().app_config_dir()?;
        let settings_store = SettingsStore::new(&config_dir);
        let settings = settings_store.load();
        let online_store = OnlineStore::new(&config_dir);
        let online = online_store.load();
        let backups = BackupStore::new(&config_dir);
        let page_cache = app.path().app_cache_dir()?.join("pages");
        let search_dir = app.path().app_cache_dir()?.join("search");
        let tessdata = app.path().app_data_dir()?.join("tessdata");
        let whisper_dir = app.path().app_data_dir()?.join("whisper");
        let dictionaries = app.path().app_data_dir()?.join("dictionaries");
        let extras_dir = app.path().app_data_dir()?.join("excalidraw");
        let maths_dir = app.path().app_data_dir()?;
        // Keep the page cache under 2 GB (least recently read books go first).
        let cache = page_cache.clone();
        std::thread::spawn(move || {
            libreri_library::prune_page_cache(&cache, PAGE_CACHE_LIMIT);
        });

        let handle = app.clone();
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2)
            .min(4);
        let jobs = JobQueue::new(threads, move |event: libreri_jobs::JobEvent| {
            let _ = JobEventPayload::from(event).emit(&handle);
        });

        Ok(Self {
            settings: Mutex::new(settings),
            settings_store,
            online: Mutex::new(online),
            online_store,
            backups,
            page_cache,
            search_dir,
            search: Mutex::new(None),
            index_wanted: Arc::default(),
            index_running: Arc::default(),
            tessdata,
            whisper_dir,
            extras_dir,
            maths_dir,
            maths_reader: Mutex::default(),
            maths_download: Mutex::default(),
            capture_photos: Mutex::default(),
            font_downloads: Mutex::default(),
            spell: Arc::new(crate::spell_cache::SpellCache::new(dictionaries)),
            transcriber: Mutex::new(None),
            model_downloads: Mutex::default(),
            backup_running: Arc::default(),
            http: make_http(),
            phone: Mutex::new(None),
            compares: Mutex::new(Vec::new()),
            speaker: Arc::default(),
            library: Mutex::new(None),
            watcher: Mutex::new(None),
            jobs,
            scan_queued: Arc::default(),
            app: app.clone(),
        })
    }

    /// The open library, or a `noLibrary` error.
    pub fn library(&self) -> AppResult<Arc<Library>> {
        lock(&self.library).clone().ok_or_else(AppError::no_library)
    }

    pub fn library_if_open(&self) -> Option<Arc<Library>> {
        lock(&self.library).clone()
    }

    /// Updates settings in memory and on disk.
    pub fn update_settings(
        &self,
        change: impl FnOnce(&mut AppSettings),
    ) -> std::io::Result<AppSettings> {
        let mut settings = lock(&self.settings);
        change(&mut settings);
        self.settings_store.save(&settings)?;
        Ok(settings.clone())
    }

    /// The speech model chosen in Settings (or the first downloaded one),
    /// loaded on first use and kept.
    pub fn transcriber(&self) -> AppResult<Arc<libreri_speech::Transcriber>> {
        let speech = lock(&self.settings).speech.clone();
        let models = libreri_speech::models(&self.whisper_dir);
        let id = speech
            .model
            .filter(|m| models.iter().any(|x| &x.id == m && x.downloaded))
            .or_else(|| models.iter().find(|m| m.downloaded).map(|m| m.id.clone()))
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::NotFound,
                    "download a speech model first (Settings › Speech)",
                )
            })?;
        let mut cached = lock(&self.transcriber);
        if let Some(t) = cached.as_ref().filter(|t| t.model == id) {
            return Ok(Arc::clone(t));
        }
        // Free the old model before loading the next (they are large).
        *cached = None;
        let path = libreri_speech::model_path(&self.whisper_dir, &id)
            .ok_or_else(|| AppError::new(AppErrorKind::NotFound, "the speech model is missing"))?;
        let t = Arc::new(libreri_speech::Transcriber::load(&path, &id).map_err(AppError::invalid)?);
        *cached = Some(Arc::clone(&t));
        Ok(t)
    }

    /// Forgets the loaded speech model (it was removed or changed).
    pub fn drop_transcriber(&self) {
        *lock(&self.transcriber) = None;
    }

    /// Listens to stretches of an audiobook and places them in its linked
    /// book; the places become sync points. Ends with `AutoSyncFinished`.
    pub fn start_auto_sync(&self, audio: BookId) -> AppResult<JobId> {
        let library = self.library()?;
        let link = library.audio_link(&audio)?;
        let text = link
            .text
            .ok_or_else(|| AppError::invalid("link the audiobook to its book first"))?;
        if !libreri_speech::models(&self.whisper_dir)
            .iter()
            .any(|m| m.downloaded)
        {
            return Err(AppError::new(
                AppErrorKind::NotFound,
                "download a speech model first (Settings › Speech)",
            ));
        }
        let language = lock(&self.settings).speech.language.clone();
        let handle = self.app.clone();
        let title = library
            .book(&audio)
            .map(|b| b.metadata.title)
            .unwrap_or_default();
        let label = format!("Finding places in “{title}”");
        Ok(self.jobs.submit(label, move |ctx| {
            let mut finished = AutoSyncFinished {
                audio_id: audio.to_string(),
                found: 0,
                tried: 0,
                error: None,
            };
            // Loading the model takes a moment, so it happens here.
            let result = handle
                .state::<AppState>()
                .transcriber()
                .map_err(|e| JobError::Failed(e.to_string()))
                .and_then(|transcriber| {
                    crate::commands::speech::auto_sync(
                        &library,
                        &transcriber,
                        language.as_deref(),
                        &audio,
                        &text,
                        ctx,
                        &mut finished,
                    )
                });
            let outcome = match result {
                Ok(()) => Ok(()),
                Err(JobError::Cancelled(c)) => {
                    finished.error = Some("cancelled".into());
                    Err(JobError::Cancelled(c))
                }
                Err(JobError::Failed(e)) => {
                    finished.error = Some(e.clone());
                    Err(JobError::Failed(e))
                }
            };
            let _ = finished.emit(&handle);
            outcome
        }))
    }

    pub fn set_phone_scanner(&self, scanner: libreri_scan::PhoneScanner) {
        *lock(&self.phone) = Some(scanner);
    }

    /// Stops the phone scanning page, if it is open.
    pub fn stop_phone_scan(&self) {
        let scanner = lock(&self.phone).take();
        drop(scanner);
    }

    pub fn online_settings(&self) -> libreri_metadata::Settings {
        lock(&self.online).clone()
    }

    /// Updates the online sources in memory and on disk.
    pub fn update_online(
        &self,
        change: impl FnOnce(&mut libreri_metadata::Settings),
    ) -> std::io::Result<libreri_metadata::Settings> {
        let mut online = lock(&self.online);
        change(&mut online);
        self.online_store.save(&online)?;
        Ok(online.clone())
    }

    /// Makes `library` the open one, starts watching its folder and checks it
    /// for changes made while Libreri was closed.
    pub fn adopt(&self, library: Library) {
        let books_dir = library.layout().books_dir();
        let index_path = self
            .search_dir
            .join(format!("{}.sqlite", library.info().id));
        match SearchIndex::open(&index_path) {
            Ok(index) => *lock(&self.search) = Some(Arc::new(index)),
            Err(e) => eprintln!("Libreri: the search index cannot be opened: {e}"),
        }
        *lock(&self.library) = Some(Arc::new(library));
        let handle = self.app.clone();
        let watcher = LibraryWatcher::start(&books_dir, move || {
            if let Some(state) = handle.try_state::<AppState>() {
                state.schedule_scan();
            }
        });
        match watcher {
            Ok(w) => *lock(&self.watcher) = Some(w),
            Err(e) => eprintln!("Libreri: cannot watch the library folder: {e}"),
        }
        self.schedule_scan();
    }

    /// Reopens the library used last time, like a book app should. Quietly
    /// does nothing if it moved, is open elsewhere, or fails to open.
    pub fn reopen_last_library(&self) {
        let last = lock(&self.settings).recent_libraries.first().cloned();
        if let Some(recent) = last {
            if let Ok(library) =
                Library::open(&recent.path, libreri_library::OpenOptions::default())
            {
                self.adopt(library);
            }
        }
    }

    /// Queues a scan of the library folder unless one is already waiting.
    pub fn schedule_scan(&self) -> Option<JobId> {
        let library = self.library_if_open()?;
        if self.scan_queued.swap(true, Ordering::SeqCst) {
            return None;
        }
        let queued = Arc::clone(&self.scan_queued);
        let handle = self.app.clone();
        Some(
            self.jobs
                .submit("Checking the library for changes", move |ctx| {
                    queued.store(false, Ordering::SeqCst);
                    let report = library.scan(&JobProgress(ctx)).map_err(job_error)?;
                    if report.changed_anything() {
                        let _ = LibraryChanged::default().emit(&handle);
                    }
                    // New or changed books get their words indexed.
                    if let Some(state) = handle.try_state::<AppState>() {
                        state.request_indexing();
                    }
                    Ok(())
                }),
        )
    }

    /// Queues an import; the result arrives as an `ImportFinished` event.
    pub fn start_import(&self, request: ImportRequest) -> AppResult<JobId> {
        let library = self.library()?;
        let handle = self.app.clone();
        let label = match request.sources.len() {
            1 => "Importing 1 item".to_owned(),
            n => format!("Importing {n} items"),
        };
        let online = self.online_settings();
        let http = Arc::clone(&self.http);
        Ok(self.jobs.submit(label, move |ctx| {
            let result = library.import(&request, &JobProgress(ctx));
            let _ = LibraryChanged::default().emit(&handle);
            let report = result.map_err(job_error)?;
            let _ = ImportFinished::new(ctx.id(), &report).emit(&handle);
            if online.fill_on_import && !report.added_ids.is_empty() {
                ctx.progress(
                    0,
                    report.added_ids.len() as u64,
                    Some("Looking up details".into()),
                );
                let filled = library
                    .fill_missing_details(&report.added_ids, &online, &*http, &JobProgress(ctx))
                    .map_err(job_error)?;
                let _ = LibraryChanged::default().emit(&handle);
                let _ = DetailsFilled::new(ctx.id(), &filled).emit(&handle);
            }
            Ok(())
        }))
    }

    /// Fills in missing details of many books; the result arrives as a
    /// `DetailsFilled` event.
    pub fn start_fill_details(&self, ids: Vec<BookId>) -> AppResult<JobId> {
        let library = self.library()?;
        let handle = self.app.clone();
        let online = self.online_settings();
        let http = Arc::clone(&self.http);
        let label = match ids.len() {
            1 => "Looking up details of 1 book".to_owned(),
            n => format!("Looking up details of {n} books"),
        };
        Ok(self.jobs.submit(label, move |ctx| {
            let result = library.fill_missing_details(&ids, &online, &*http, &JobProgress(ctx));
            let _ = LibraryChanged::default().emit(&handle);
            let report = result.map_err(job_error)?;
            let _ = DetailsFilled::new(ctx.id(), &report).emit(&handle);
            Ok(())
        }))
    }

    /// Writes an export; the result arrives as an `ExportFinished` event.
    pub fn start_export(&self, request: ExportRequest) -> AppResult<JobId> {
        let library = self.library()?;
        let handle = self.app.clone();
        Ok(self.jobs.submit("Exporting", move |ctx| {
            let report = library
                .export(&request, &JobProgress(ctx))
                .map_err(job_error)?;
            let _ = ExportFinished::new(ctx.id(), &report).emit(&handle);
            Ok(())
        }))
    }

    /// Imports a Libreri archive; the result arrives as `ArchiveImported`.
    pub fn start_archive_import(
        &self,
        path: std::path::PathBuf,
        choice: ArchiveImport,
    ) -> AppResult<JobId> {
        let library = self.library()?;
        let handle = self.app.clone();
        Ok(self.jobs.submit("Importing a Libreri archive", move |ctx| {
            let result = library.import_archive(&path, &choice, &JobProgress(ctx));
            let _ = LibraryChanged::default().emit(&handle);
            let report = result.map_err(job_error)?;
            let _ = ArchiveImported::new(ctx.id(), &report).emit(&handle);
            Ok(())
        }))
    }

    /// Imports from another app; the report arrives as `ForeignImported`.
    pub fn start_foreign_import(
        &self,
        path: std::path::PathBuf,
        options: libreri_library::ForeignImport,
        source: &'static str,
    ) -> AppResult<JobId> {
        let library = self.library()?;
        let handle = self.app.clone();
        Ok(self
            .jobs
            .submit(format!("Importing from {source}"), move |ctx| {
                let result = library.import_foreign(&path, &options, &JobProgress(ctx));
                let _ = LibraryChanged::default().emit(&handle);
                let report = result.map_err(job_error)?;
                let _ = ForeignImported::new(ctx.id(), source, &report).emit(&handle);
                Ok(())
            }))
    }

    /// Backup settings of the open library.
    pub fn backup_settings(&self) -> AppResult<(String, BackupSettings)> {
        let id = self.library()?.info().id.to_string();
        let settings = self.backups.get(&id);
        Ok((id, settings))
    }

    /// Backs up the open library into its backup folder; the result arrives
    /// as `BackupFinished`. `None` if a backup is already running.
    pub fn start_backup(&self, scheduled: bool) -> AppResult<Option<JobId>> {
        let library = self.library()?;
        let (id, settings) = self.backup_settings()?;
        let folder = settings
            .folder
            .clone()
            .filter(|f| !f.is_empty())
            .ok_or_else(|| AppError::invalid("choose a folder for backups first"))?;
        if self.backup_running.swap(true, Ordering::SeqCst) {
            return Ok(None);
        }
        let running = Arc::clone(&self.backup_running);
        let handle = self.app.clone();
        let started = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let _ = self.backups.update(&id, |s| s.last_attempt = Some(started));
        let label = if scheduled {
            "Backing up the library (scheduled)"
        } else {
            "Backing up the library"
        };
        Ok(Some(self.jobs.submit(label, move |ctx| {
            let result = library.back_up_into(
                std::path::Path::new(&folder),
                settings.keep as usize,
                settings.book_files,
                env!("CARGO_PKG_VERSION"),
                &JobProgress(ctx),
            );
            running.store(false, Ordering::SeqCst);
            let stamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            let (path, error) = match &result {
                Ok(r) => (Some(r.path.to_string_lossy().into_owned()), None),
                Err(libreri_library::Error::Cancelled) => (None, None),
                Err(e) => (None, Some(e.to_string())),
            };
            if let Some(state) = handle.try_state::<AppState>() {
                let _ = state.backups.update(&id, |s| {
                    if path.is_some() {
                        s.last_backup = Some(stamp.clone());
                        s.last_error = None;
                    } else if error.is_some() {
                        s.last_error = error.clone();
                    }
                });
            }
            let _ = BackupFinished {
                job_id: ctx.id().to_string(),
                path,
                error,
                scheduled,
            }
            .emit(&handle);
            result.map(|_| ()).map_err(job_error)
        })))
    }

    /// Runs what is due: a scheduled backup, and rewriting the file kept up
    /// to date. Called every few minutes by [`start_scheduler`].
    fn run_due_tasks(&self) {
        let Ok((id, settings)) = self.backup_settings() else {
            return;
        };
        let now = chrono::Utc::now();
        if settings.due(now) && !settings.failed_recently(now) {
            if let Err(e) = self.start_backup(true) {
                let stamp = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
                let _ = self.backups.update(&id, |s| {
                    s.last_error = Some(e.message);
                    s.last_attempt = Some(stamp);
                });
            }
        }
        if let (Some(auto), Some(library)) = (settings.auto_export, self.library_if_open()) {
            let result = library.write_catalogue(auto.format, std::path::Path::new(&auto.path));
            let stamp = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            let _ = self.backups.update(&id, |s| match result {
                Ok(true) => {
                    s.last_auto_export = Some(stamp);
                    s.auto_export_error = None;
                }
                Ok(false) => s.auto_export_error = None,
                Err(e) => s.auto_export_error = Some(e.to_string()),
            });
        }
    }

    /// Rebuilds the database from files and sidecars.
    pub fn start_rebuild(&self) -> AppResult<JobId> {
        let library = self.library()?;
        let handle = self.app.clone();
        Ok(self
            .jobs
            .submit("Rebuilding the library index", move |ctx| {
                let result = library.rebuild_index(&JobProgress(ctx));
                let _ = LibraryChanged::default().emit(&handle);
                if let Some(state) = handle.try_state::<AppState>() {
                    state.request_indexing();
                }
                result.map(|_| ()).map_err(job_error)
            }))
    }

    /// The open library's search index.
    pub fn search_index(&self) -> AppResult<Arc<SearchIndex>> {
        self.library()?;
        lock(&self.search)
            .clone()
            .ok_or_else(|| AppError::invalid("the search index could not be opened"))
    }

    /// Indexing progress, for the interface.
    pub fn indexing(&self) -> bool {
        self.index_running.load(Ordering::SeqCst)
    }

    /// Brings the search index up to date in the background. Calls while
    /// it runs make it go round once more when it finishes.
    pub fn request_indexing(&self) {
        self.index_wanted.store(true, Ordering::SeqCst);
        if self.index_running.swap(true, Ordering::SeqCst) {
            return;
        }
        let handle = self.app.clone();
        let wanted = Arc::clone(&self.index_wanted);
        let running = Arc::clone(&self.index_running);
        let spawned = std::thread::Builder::new()
            .name("libreri-indexer".into())
            .spawn(move || {
                while wanted.swap(false, Ordering::SeqCst) {
                    let Some(state) = handle.try_state::<AppState>() else {
                        break;
                    };
                    let (Some(library), Ok(index)) =
                        (state.library_if_open(), state.search_index())
                    else {
                        break;
                    };
                    let progress = IndexProgress {
                        handle: handle.clone(),
                        library: Arc::downgrade(&library),
                        last: Mutex::new(std::time::Instant::now()),
                    };
                    if let Err(e) = library.update_index(&index, &progress) {
                        eprintln!("Libreri: indexing stopped: {e}");
                    }
                }
                running.store(false, Ordering::SeqCst);
                let _ = SearchIndexProgress {
                    done: 0,
                    total: 0,
                    running: false,
                }
                .emit(&handle);
            });
        if spawned.is_err() {
            self.index_running.store(false, Ordering::SeqCst);
        }
    }

    /// Reads scanned books with OCR, one after another; the result arrives
    /// as `OcrFinished`.
    /// Opens both sides and compares them in the background; the result
    /// arrives as a `CompareFinished` event. Returns the comparison id and
    /// the job id.
    pub fn start_compare(
        &self,
        a: libreri_library::CompareSource,
        b: libreri_library::CompareSource,
    ) -> AppResult<(String, JobId)> {
        let library = self.library()?;
        let session = Arc::new(CompareSession {
            a: library.compare_doc(&a)?,
            b: library.compare_doc(&b)?,
            result: Mutex::new(None),
            images: Mutex::default(),
        });
        let id = uuid::Uuid::new_v4().to_string();
        {
            let mut list = lock(&self.compares);
            list.push((id.clone(), Arc::clone(&session)));
            let extra = list.len().saturating_sub(COMPARES_KEPT);
            list.drain(..extra);
        }
        let handle = self.app.clone();
        let compare_id = id.clone();
        let job = self.jobs.submit("Comparing".to_owned(), move |ctx| {
            let result = library.compare(&session.a, &session.b, &JobProgress(ctx));
            let (outcome, error) = match result {
                Ok(r) => {
                    *lock(&session.result) = Some(Ok(r));
                    (Ok(()), None)
                }
                Err(libreri_library::Error::Cancelled) => (
                    Err(JobError::Cancelled(libreri_jobs::Cancelled)),
                    Some("cancelled".to_owned()),
                ),
                Err(e) => {
                    *lock(&session.result) = Some(Err(e.to_string()));
                    (Err(JobError::Failed(e.to_string())), Some(e.to_string()))
                }
            };
            let _ = CompareFinished {
                id: compare_id,
                error,
            }
            .emit(&handle);
            outcome
        });
        Ok((id, job))
    }

    /// A comparison started earlier.
    pub fn compare_session(&self, id: &str) -> Option<Arc<CompareSession>> {
        lock(&self.compares)
            .iter()
            .find(|(k, _)| k == id)
            .map(|(_, s)| Arc::clone(s))
    }

    /// Forgets a comparison (its tab closed).
    pub fn close_compare(&self, id: &str) {
        lock(&self.compares).retain(|(k, _)| k != id);
    }

    pub fn start_ocr(&self, ids: Vec<BookId>, options: OcrOptions) -> AppResult<JobId> {
        let library = self.library()?;
        let index = self.search_index().ok();
        let scratch = self.page_cache.join(".ocr");
        let handle = self.app.clone();
        let label = match ids.len() {
            1 => "Making 1 book searchable".to_owned(),
            n => format!("Making {n} books searchable"),
        };
        Ok(self.jobs.submit(label, move |ctx| {
            let mut finished = OcrFinished {
                job_id: ctx.id().to_string(),
                books: 0,
                pages_read: 0,
                pages_failed: 0,
                errors: Vec::new(),
                book_ids: ids.iter().map(|i| i.to_string()).collect(),
            };
            let many = ids.len() > 1;
            let mut outcome = Ok(());
            for id in &ids {
                let title = library
                    .book(id)
                    .map(|b| b.metadata.title)
                    .unwrap_or_default();
                let progress = OcrProgress {
                    ctx,
                    prefix: if many {
                        format!("{title}: ")
                    } else {
                        String::new()
                    },
                };
                match library.make_searchable(id, &options, &scratch, index.as_deref(), &progress) {
                    Ok(r) => {
                        finished.books += 1;
                        finished.pages_read += r.pages_read;
                        finished.pages_failed += r.pages_failed;
                        for e in r.errors {
                            finished.errors.push(format!("{title}: {e}"));
                        }
                    }
                    Err(libreri_library::Error::Cancelled) => {
                        outcome = Err(JobError::Cancelled(libreri_jobs::Cancelled));
                        break;
                    }
                    Err(e) => finished.errors.push(format!("{title}: {e}")),
                }
            }
            let _ = finished.emit(&handle);
            outcome
        }))
    }

    /// Closes the open library, if any. Errors are logged, not raised,
    /// because this also runs while the window is closing. Running jobs keep
    /// their reference and the library closes fully when they finish.
    pub fn close_library(&self) {
        self.stop_phone_scan();
        lock(&self.watcher).take();
        lock(&self.search).take();
        if let Some(library) = lock(&self.library).take() {
            if let Err(err) = library.shutdown() {
                eprintln!("Libreri: failed to close library cleanly: {err}");
            }
        }
    }
}
