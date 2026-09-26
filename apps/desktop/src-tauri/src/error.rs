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
    /// No library is open (the window is on the Welcome screen).
    NoLibrary,
    NotFound,
    /// Something the user typed was refused; the message says why.
    InvalidInput,
    NameTaken,
    Trash,
    Cancelled,
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
            E::Db(_) | E::BadInfo(_) | E::Closed => AppErrorKind::Storage,
            E::Io(_) => AppErrorKind::Io,
            E::BookNotFound => AppErrorKind::NotFound,
            E::InvalidInput(_) => AppErrorKind::InvalidInput,
            E::NameTaken(_) => AppErrorKind::NameTaken,
            E::Trash(_) => AppErrorKind::Trash,
            E::Cancelled => AppErrorKind::Cancelled,
        };
        Self::new(kind, err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::new(AppErrorKind::Io, err.to_string())
    }
}

impl AppError {
    pub fn no_library() -> Self {
        Self::new(AppErrorKind::NoLibrary, "no library is open")
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::InvalidInput, message)
    }
}

pub type AppResult<T> = Result<T, AppError>;
