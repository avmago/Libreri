//! The `book://` protocol.
//!
//! The interface never reads files directly. It asks for
//! `book://localhost/<path relative to the library>` and this handler serves
//! the file from the open library, refusing anything outside it.
//!
//! Requests are answered on a worker thread, and HTTP range requests are
//! supported so PDF.js can load large PDFs page by page instead of reading
//! the whole file first.

use crate::state::AppState;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use tauri::http::{header, Method, Request, Response, StatusCode};
use tauri::{AppHandle, Manager, UriSchemeContext, UriSchemeResponder, Wry};

/// Called by Tauri; answers on a separate thread so big reads never block.
pub fn handle(
    ctx: UriSchemeContext<'_, Wry>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = ctx.app_handle().clone();
    std::thread::spawn(move || responder.respond(serve(&app, &request)));
}

fn base(code: StatusCode) -> tauri::http::response::Builder {
    Response::builder()
        .status(code)
        // The dev server runs on another origin; the app itself is the
        // only page this webview ever loads.
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(
            header::ACCESS_CONTROL_EXPOSE_HEADERS,
            "Content-Range, Accept-Ranges, Content-Length",
        )
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    base(code).body(Vec::new()).expect("static response")
}

fn serve(app: &AppHandle, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    if request.method() == Method::OPTIONS {
        return base(StatusCode::NO_CONTENT)
            .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, HEAD, OPTIONS")
            .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "Range")
            .body(Vec::new())
            .expect("static response");
    }
    let Some(state) = app.try_state::<AppState>() else {
        return status(StatusCode::SERVICE_UNAVAILABLE);
    };
    let raw = request.uri().path().trim_start_matches('/');
    let Ok(relative) = percent_decode(raw) else {
        return status(StatusCode::BAD_REQUEST);
    };
    // `.extras/fonts/<font>/<file>.woff2`: canvas fonts downloaded in Settings.
    if let Some(rest) = relative.strip_prefix(".extras/") {
        return match libreri_helpers::fonts::file(&state.extras_dir, rest)
            .and_then(|p| std::fs::read(p).ok())
        {
            Some(bytes) => base(StatusCode::OK)
                .header(header::CONTENT_TYPE, "font/woff2")
                .header(header::CACHE_CONTROL, "max-age=31536000")
                .body(bytes)
                .expect("font response"),
            None => status(StatusCode::NOT_FOUND),
        };
    }
    // `.media/<token>/<name>`: a video or audio file on this computer that
    // a link points to (registered when it is played).
    if let Some(rest) = relative.strip_prefix(".media/") {
        let token = rest.split('/').next().unwrap_or("");
        let path = state
            .media_files
            .lock()
            .expect("media lock")
            .get(token)
            .cloned();
        return match path {
            Some(p) => serve_file(request, &p, &p.to_string_lossy()),
            None => status(StatusCode::NOT_FOUND),
        };
    }
    let Some(library) = state.library_if_open() else {
        return status(StatusCode::NOT_FOUND);
    };
    // `.pages/<book id>/<page>?w=<width>`: a comic or DjVu page image.
    if let Some(rest) = relative.strip_prefix(".pages/") {
        return serve_page(&state, &library, rest, request.uri().query().unwrap_or(""));
    }
    // `.compare/<id>/<a|b>/<page>?w=<width>`: a page of a comparison.
    if let Some(rest) = relative.strip_prefix(".compare/") {
        return serve_compare(&state, rest, request.uri().query().unwrap_or(""));
    }
    // Only books the signed-in profile may see, and covers.
    if !library.may_open(&relative) {
        return status(StatusCode::FORBIDDEN);
    }
    let Some(path) = library.layout().resolve_relative(&relative) else {
        return status(StatusCode::FORBIDDEN);
    };
    serve_file(request, &path, &relative)
}

/// A file, with range requests (for seeking in audio and video).
fn serve_file(request: &Request<Vec<u8>>, path: &std::path::Path, name: &str) -> Response<Vec<u8>> {
    let relative = name;
    let Ok(mut file) = File::open(path) else {
        return status(StatusCode::NOT_FOUND);
    };
    let Ok(total) = file.metadata().map(|m| m.len()) else {
        return status(StatusCode::NOT_FOUND);
    };
    let range = request
        .headers()
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| parse_range(v, total));

    let kind = content_type(relative);
    let mut builder = base(StatusCode::OK)
        .header(header::CONTENT_TYPE, kind)
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::ACCEPT_RANGES, "bytes");
    if kind.starts_with("text/html") {
        // Saved web pages: nothing in them may run or reach out.
        builder = builder.header(
            header::CONTENT_SECURITY_POLICY,
            "sandbox; default-src 'none'; img-src data:; style-src 'unsafe-inline'",
        );
    }
    let head = request.method() == Method::HEAD;

    match range {
        Some(Err(())) => base(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(header::CONTENT_RANGE, format!("bytes */{total}"))
            .body(Vec::new())
            .expect("static response"),
        Some(Ok((start, end))) => {
            let len = end - start + 1;
            let mut buf = vec![0; if head { 0 } else { len as usize }];
            if !head
                && (file.seek(SeekFrom::Start(start)).is_err()
                    || file.read_exact(&mut buf).is_err())
            {
                return status(StatusCode::INTERNAL_SERVER_ERROR);
            }
            builder
                .status(StatusCode::PARTIAL_CONTENT)
                .header(
                    header::CONTENT_RANGE,
                    format!("bytes {start}-{end}/{total}"),
                )
                .header(header::CONTENT_LENGTH, len)
                .body(buf)
                .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR))
        }
        None => {
            let mut buf = Vec::with_capacity(if head { 0 } else { total as usize });
            if !head && file.read_to_end(&mut buf).is_err() {
                return status(StatusCode::INTERNAL_SERVER_ERROR);
            }
            builder
                .header(header::CONTENT_LENGTH, total)
                .body(buf)
                .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR))
        }
    }
}

fn serve_page(
    state: &AppState,
    library: &libreri_library::Library,
    rest: &str,
    query: &str,
) -> Response<Vec<u8>> {
    let Some((id, page)) = rest.split_once('/') else {
        return status(StatusCode::BAD_REQUEST);
    };
    let (Ok(id), Ok(page)) = (libreri_core::BookId::from_hex(id), page.parse::<u32>()) else {
        return status(StatusCode::BAD_REQUEST);
    };
    let width = query
        .split('&')
        .find_map(|kv| kv.strip_prefix("w="))
        .and_then(|w| w.parse().ok())
        .unwrap_or(1600);
    match library.page_image(&id, page, width, &state.page_cache) {
        Ok((bytes, mime)) => base(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime)
            // Pages of a book never change (its id is its content).
            .header(header::CACHE_CONTROL, "private, max-age=86400")
            .header(header::CONTENT_LENGTH, bytes.len())
            .body(bytes)
            .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR)),
        Err(libreri_library::Error::BookNotFound) => status(StatusCode::NOT_FOUND),
        Err(e) => base(StatusCode::UNPROCESSABLE_ENTITY)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(e.to_string().into_bytes())
            .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR)),
    }
}

fn serve_compare(state: &AppState, rest: &str, query: &str) -> Response<Vec<u8>> {
    let parts: Vec<&str> = rest.split('/').collect();
    let [id, side, page] = parts[..] else {
        return status(StatusCode::BAD_REQUEST);
    };
    let side = match side {
        "a" => 0,
        "b" => 1,
        _ => return status(StatusCode::BAD_REQUEST),
    };
    let Ok(page) = page.parse::<u32>() else {
        return status(StatusCode::BAD_REQUEST);
    };
    let width = query
        .split('&')
        .find_map(|kv| kv.strip_prefix("w="))
        .and_then(|w| w.parse::<u32>().ok())
        .unwrap_or(900)
        .clamp(100, 2400);
    let Some(session) = state.compare_session(id) else {
        return status(StatusCode::NOT_FOUND);
    };
    match session.picture(side, page, width) {
        Ok(jpeg) => base(StatusCode::OK)
            .header(header::CONTENT_TYPE, "image/jpeg")
            .header(header::CACHE_CONTROL, "private, max-age=3600")
            .header(header::CONTENT_LENGTH, jpeg.len())
            .body(jpeg.to_vec())
            .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR)),
        Err(e) => base(StatusCode::UNPROCESSABLE_ENTITY)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(e.into_bytes())
            .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR)),
    }
}

/// Parses a single `bytes=` range. `Some(Err)` means unsatisfiable; `None`
/// means no usable range (serve the whole file).
fn parse_range(value: &str, total: u64) -> Option<Result<(u64, u64), ()>> {
    let spec = value.trim().strip_prefix("bytes=")?;
    if spec.contains(',') || total == 0 {
        return None;
    }
    let (a, b) = spec.split_once('-')?;
    let (start, end) = match (a.trim(), b.trim()) {
        ("", suffix) => {
            let n: u64 = suffix.parse().ok()?;
            (total.saturating_sub(n), total - 1)
        }
        (s, "") => (s.parse().ok()?, total - 1),
        (s, e) => (s.parse().ok()?, e.parse::<u64>().ok()?.min(total - 1)),
    };
    Some(if start > end || start >= total {
        Err(())
    } else {
        Ok((start, end))
    })
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
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "mp3" => "audio/mpeg",
        "m4a" | "m4b" => "audio/mp4",
        "ogg" | "opus" => "audio/ogg",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        "wav" => "audio/wav",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "ogv" => "video/ogg",
        // Offline copies of web pages (shown in a sandboxed frame).
        "html" | "htm" => "text/html; charset=utf-8",
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

    #[test]
    fn parses_ranges() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some(Ok((0, 99))));
        assert_eq!(parse_range("bytes=900-", 1000), Some(Ok((900, 999))));
        assert_eq!(parse_range("bytes=-100", 1000), Some(Ok((900, 999))));
        assert_eq!(parse_range("bytes=990-2000", 1000), Some(Ok((990, 999))));
        assert_eq!(parse_range("bytes=1000-", 1000), Some(Err(())));
        assert_eq!(parse_range("bytes=0-1,5-6", 1000), None);
        assert_eq!(parse_range("items=1-2", 1000), None);
    }
}
