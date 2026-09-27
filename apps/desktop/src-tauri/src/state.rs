//! Shared app state held by Tauri.

use crate::error::{AppError, AppResult};
use crate::events::{DetailsFilled, ImportFinished, JobEventPayload, LibraryChanged};
use crate::online_store::OnlineStore;
use crate::settings_store::SettingsStore;
use libreri_core::AppSettings;
use libreri_core::BookId;
use libreri_jobs::{JobContext, JobError, JobId, JobQueue};
use libreri_library::{ImportRequest, Library, LibraryWatcher, Progress};
use libreri_metadata::Http;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

pub struct AppState {
    pub settings: Mutex<AppSettings>,
    pub settings_store: SettingsStore,
    /// Online sources and API keys (per computer, never exported).
    pub online: Mutex<libreri_metadata::Settings>,
    pub online_store: OnlineStore,
    /// Shared by all lookups, so connections are reused.
    pub http: Arc<dyn Http + Send>,
    /// The phone scanning page, while it is open.
    phone: Mutex<Option<libreri_scan::PhoneScanner>>,
    library: Mutex<Option<Arc<Library>>>,
    watcher: Mutex<Option<LibraryWatcher>>,
    pub jobs: JobQueue,
    /// A scan is queued and has not started yet; further requests are merged.
    scan_queued: Arc<AtomicBool>,
    app: AppHandle,
}

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
            http: make_http(),
            phone: Mutex::new(None),
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

    /// Rebuilds the database from files and sidecars.
    pub fn start_rebuild(&self) -> AppResult<JobId> {
        let library = self.library()?;
        let handle = self.app.clone();
        Ok(self
            .jobs
            .submit("Rebuilding the library index", move |ctx| {
                let result = library.rebuild_index(&JobProgress(ctx));
                let _ = LibraryChanged::default().emit(&handle);
                result.map(|_| ()).map_err(job_error)
            }))
    }

    /// Closes the open library, if any. Errors are logged, not raised,
    /// because this also runs while the window is closing. Running jobs keep
    /// their reference and the library closes fully when they finish.
    pub fn close_library(&self) {
        self.stop_phone_scan();
        lock(&self.watcher).take();
        if let Some(library) = lock(&self.library).take() {
            if let Err(err) = library.shutdown() {
                eprintln!("Libreri: failed to close library cleanly: {err}");
            }
        }
    }
}
