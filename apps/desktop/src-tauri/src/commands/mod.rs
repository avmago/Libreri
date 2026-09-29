//! Tauri commands, one module per feature.
//!
//! Each command validates input, calls one service and maps the error.
//! No business rules here.
//!
//! Commands that touch files, the database at length or the CPU are
//! `async` and do their work through [`blocking`], so the interface never
//! waits on them (plain commands run on the main thread).

pub mod app;
pub mod books;
pub mod canvas;
pub mod capture;
pub mod compare;
pub mod details;
pub mod edit;
pub mod feeds;
pub mod folders;
pub mod library;
pub mod links;
pub mod listening;
pub mod maths;
pub mod notes;
pub mod organize;
pub mod pages;
pub mod portability;
pub mod profiles;
pub mod reader;
pub mod scan;
pub mod search;
pub mod settings;
pub mod speech;
pub mod spell;

use crate::error::{AppError, AppErrorKind, AppResult};

/// Runs slow work (files, the database, hashing) on a worker thread.
pub(crate) async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::new(AppErrorKind::Io, e.to_string()))?
}
