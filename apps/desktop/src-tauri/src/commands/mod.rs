//! Tauri commands, one module per feature.
//!
//! Each command validates input, calls one service and maps the error.
//! No business rules here.

pub mod app;
pub mod library;
pub mod settings;
