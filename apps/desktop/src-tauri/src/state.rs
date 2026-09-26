//! Shared app state held by Tauri.

use crate::events::JobEventPayload;
use crate::settings_store::SettingsStore;
use libreri_core::AppSettings;
use libreri_jobs::JobQueue;
use libreri_library::Library;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

pub struct AppState {
    pub settings: Mutex<AppSettings>,
    pub settings_store: SettingsStore,
    pub library: Mutex<Option<Library>>,
    /// Used from Phase 1 (imports, thumbnails); started now so events flow.
    #[allow(dead_code)]
    pub jobs: JobQueue,
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
            jobs,
        })
    }

    /// Updates settings in memory and on disk.
    pub fn update_settings(
        &self,
        change: impl FnOnce(&mut AppSettings),
    ) -> std::io::Result<AppSettings> {
        let mut settings = self.settings.lock().expect("settings lock poisoned");
        change(&mut settings);
        self.settings_store.save(&settings)?;
        Ok(settings.clone())
    }

    /// Closes the open library, if any. Errors are logged, not raised,
    /// because this also runs while the window is closing.
    pub fn close_library(&self) {
        if let Some(library) = self.library.lock().expect("library lock poisoned").take() {
            if let Err(err) = library.close() {
                eprintln!("Libreri: failed to close library cleanly: {err}");
            }
        }
    }
}
