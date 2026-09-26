//! Shared app state held by Tauri.

use crate::error::{AppError, AppResult};
use crate::events::{ImportFinished, JobEventPayload, LibraryChanged};
use crate::settings_store::SettingsStore;
use libreri_core::AppSettings;
use libreri_jobs::{JobContext, JobError, JobId, JobQueue};
use libreri_library::{ImportRequest, Library, LibraryWatcher, Progress};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

pub struct AppState {
    pub settings: Mutex<AppSettings>,
    pub settings_store: SettingsStore,
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

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl AppState {
    pub fn initialise(app: &AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let config_dir = app.path().app_config_dir()?;
        let settings_store = SettingsStore::new(&config_dir);
        let settings = settings_store.load();

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
        Ok(self.jobs.submit(label, move |ctx| {
            let result = library.import(&request, &JobProgress(ctx));
            let _ = LibraryChanged::default().emit(&handle);
            let report = result.map_err(job_error)?;
            let _ = ImportFinished::new(ctx.id(), &report).emit(&handle);
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
        lock(&self.watcher).take();
        if let Some(library) = lock(&self.library).take() {
            if let Err(err) = library.shutdown() {
                eprintln!("Libreri: failed to close library cleanly: {err}");
            }
        }
    }
}
