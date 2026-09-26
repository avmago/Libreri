//! One error type for every command, serialised for the UI.
//!
//! `kind` lets the interface react (for example, offer "Open anyway" for
//! `lockedElsewhere`); `message` is plain language the UI can show as is.

use serde::Serialize;
use specta::Type;

#[derive(Debug, Clone, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AppErrorKind {
    NotALibrary,
    AlreadyALibrary,
    FolderNotEmpty,
    FormatTooNew,
    LockedElsewhere,
    AlreadyOpenHere,
    Storage,
    Io,
}

#[derive(Debug, Clone, Serialize, Type, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{message}")]
pub struct AppError {
    pub kind: AppErrorKind,
    pub message: String,
}

impl AppError {
    pub fn new(kind: AppErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl From<libreri_library::Error> for AppError {
    fn from(err: libreri_library::Error) -> Self {
        use libreri_library::Error as E;
        let kind = match &err {
            E::NotALibrary(_) => AppErrorKind::NotALibrary,
            E::AlreadyALibrary(_) => AppErrorKind::AlreadyALibrary,
            E::FolderNotEmpty(_) => AppErrorKind::FolderNotEmpty,
            E::FormatTooNew { .. } => AppErrorKind::FormatTooNew,
            E::LockedElsewhere { .. } => AppErrorKind::LockedElsewhere,
            E::AlreadyOpenHere => AppErrorKind::AlreadyOpenHere,
            E::Db(_) | E::BadInfo(_) => AppErrorKind::Storage,
            E::Io(_) => AppErrorKind::Io,
        };
        Self::new(kind, err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::new(AppErrorKind::Io, err.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
