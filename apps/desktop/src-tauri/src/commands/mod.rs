//! Tauri commands, one module per feature.
//!
//! Each command validates input, calls one service and maps the error.
//! No business rules here.

pub mod app;
pub mod books;
pub mod details;
pub mod folders;
pub mod library;
pub mod notes;
pub mod organize;
pub mod portability;
pub mod profiles;
pub mod reader;
pub mod scan;
pub mod settings;
