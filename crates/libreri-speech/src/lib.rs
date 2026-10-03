//! Speech to text, with whisper.cpp built into the app
//! (whisper-rs, MIT): downloadable models, audio decoding (symphonia),
//! transcription, voice-note files (FLAC) and finding an audiobook's place
//! in its text.

pub mod audio;
pub mod models;
pub mod sync;
pub mod transcribe;

pub use models::{model_path, models, ModelInfo};
pub use transcribe::{join, Segment, Transcriber};
