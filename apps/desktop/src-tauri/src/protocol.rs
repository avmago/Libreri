//! The `book://` protocol.
//!
//! The interface never reads files directly. It asks for
//! `book://localhost/<path relative to the library>` and this handler serves
//! the file from the open library, refusing anything outside it.
//!
//! Phase 0 serves whole files. Phase 2 adds HTTP range requests so large
//! PDFs and audiobooks stream instead of loading at once.

use crate::state::AppState;
use tauri::http::{header, Request, Response, StatusCode};
use tauri::{Manager, UriSchemeContext, Wry};

pub fn handle(ctx: UriSchemeContext<'_, Wry>, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    let Some(state) = ctx.app_handle().try_state::<AppState>() else {
        return status(StatusCode::SERVICE_UNAVAILABLE);
    };
    let guard = state.library.lock().expect("library lock poisoned");
    let Some(library) = guard.as_ref() else {
        return status(StatusCode::NOT_FOUND);
    };

    let raw = request.uri().path().trim_start_matches('/');
    let Ok(relative) = percent_decode(raw) else {
        return status(StatusCode::BAD_REQUEST);
    };
    let Some(path) = library.layout().resolve_relative(&relative) else {
        return status(StatusCode::FORBIDDEN);
    };

    match std::fs::read(&path) {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, content_type(&relative))
            .header(header::CACHE_CONTROL, "no-cache")
            .body(bytes)
            .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR)),
        Err(_) => status(StatusCode::NOT_FOUND),
    }
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(code)
        .body(Vec::new())
        .expect("static response")
}

/// Decodes `%20`-style escapes; rejects malformed input.
fn percent_decode(input: &str) -> Result<String, ()> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = input.get(i + 1..i + 3).ok_or(())?;
            out.push(u8::from_str_radix(hex, 16).map_err(|_| ())?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| ())
}

fn content_type(path: &str) -> &'static str {
    let ext = path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "pdf" => "application/pdf",
        "epub" => "application/epub+zip",
        "md" | "markdown" | "txt" => "text/plain; charset=utf-8",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "mp3" => "audio/mpeg",
        "m4a" | "m4b" => "audio/mp4",
        "ogg" | "opus" => "audio/ogg",
        "flac" => "audio/flac",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_escapes_and_rejects_garbage() {
        assert_eq!(
            percent_decode("Books/My%20Book.pdf").unwrap(),
            "Books/My Book.pdf"
        );
        assert!(percent_decode("bad%zz").is_err());
        assert!(percent_decode("cut%2").is_err());
    }

    #[test]
    fn picks_content_types() {
        assert_eq!(content_type("Books/a.PDF"), "application/pdf");
        assert_eq!(content_type("x.unknown"), "application/octet-stream");
    }
}
