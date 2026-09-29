//! Scanning with a phone: a small web server on the local network serves a
//! one-time page (its address holds a random token and is shown as a QR
//! code). The phone takes a photo with its own camera app and sends it:
//! of a barcode (the computer reads it), or of a paper page to add to a
//! PDF (Phase 6b, [`PhoneMode::Pages`]).
//!
//! The server listens only on the computer's local network address, only
//! answers requests carrying the token, accepts only pictures and short
//! codes, and stops after a few minutes or when scanning ends.

use crate::Scanned;
use rand_core::{OsRng, RngCore};
use std::io::Read;
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tiny_http::{Header, Method, Response, Server};

/// Largest photo accepted (the page scales photos down before sending).
const MAX_PHOTO: usize = 10 * 1024 * 1024;

/// Requests answered at the same time. Each is answered on its own thread,
/// so a phone that stops sending half-way through a photo holds up only
/// its own request; more than this at once are turned away.
const MAX_HANDLERS: usize = 4;

/// How long a request's body may take to arrive.
const BODY_TIME: Duration = Duration::from_secs(60);

/// How long dropping a scanner waits for the server to stop.
const STOP_WAIT: Duration = Duration::from_secs(1);

/// What the phone page did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhoneEvent {
    /// The phone opened the page.
    Opened,
    /// A barcode was read (from a photo, or typed on the phone).
    Scanned(Scanned),
    /// A photo of a page (JPEG or PNG bytes), in [`PhoneMode::Pages`].
    Page(Vec<u8>),
}

/// What the phone page is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PhoneMode {
    /// Barcodes of books.
    #[default]
    Barcode,
    /// Photos of paper pages.
    Pages,
}

/// What to show so the phone can connect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhonePairing {
    pub url: String,
    /// The address as a QR code, an SVG image.
    pub qr_svg: String,
    /// Seconds until the page stops working.
    pub expires_in: u64,
}

pub struct PhoneScanner {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pairing: PhonePairing,
}

fn token() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn qr_svg(url: &str) -> Result<String, String> {
    let code = qrcode::QrCode::new(url.as_bytes()).map_err(|e| e.to_string())?;
    Ok(code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(220, 220)
        .quiet_zone(true)
        .build())
}

impl PhoneScanner {
    /// Starts serving the page on the computer's local network address.
    pub fn start(
        lifetime: Duration,
        on_event: impl Fn(PhoneEvent) + Send + 'static,
    ) -> Result<Self, String> {
        Self::start_for(PhoneMode::Barcode, lifetime, on_event)
    }

    /// Starts serving the page for `mode`.
    pub fn start_for(
        mode: PhoneMode,
        lifetime: Duration,
        on_event: impl Fn(PhoneEvent) + Send + 'static,
    ) -> Result<Self, String> {
        let ip = local_ip_address::local_ip()
            .map_err(|_| "this computer does not seem to be on a network".to_owned())?;
        Self::start_on(ip, mode, lifetime, on_event)
    }

    pub(crate) fn start_on(
        ip: IpAddr,
        mode: PhoneMode,
        lifetime: Duration,
        on_event: impl Fn(PhoneEvent) + Send + 'static,
    ) -> Result<Self, String> {
        let server = Server::http((ip, 0)).map_err(|e| format!("could not start: {e}"))?;
        let port = server
            .server_addr()
            .to_ip()
            .map(|a| a.port())
            .ok_or("could not start")?;
        let host = match ip {
            IpAddr::V4(v4) => v4.to_string(),
            IpAddr::V6(v6) => format!("[{v6}]"),
        };
        let token = token();
        let url = format!("http://{host}:{port}/{token}");
        let pairing = PhonePairing {
            qr_svg: qr_svg(&url)?,
            url,
            expires_in: lifetime.as_secs(),
        };
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let deadline = Instant::now() + lifetime;
        let on_event: Arc<Mutex<dyn Fn(PhoneEvent) + Send>> = Arc::new(Mutex::new(on_event));
        let token: Arc<str> = token.into();
        let busy = Arc::new(AtomicUsize::new(0));
        let thread = std::thread::Builder::new()
            .name("libreri-phone-scan".into())
            .spawn(move || {
                while !stopping.load(Ordering::SeqCst) && Instant::now() < deadline {
                    let req = match server.recv_timeout(Duration::from_millis(250)) {
                        Ok(Some(req)) => req,
                        Ok(None) => continue,
                        Err(_) => break,
                    };
                    if busy.fetch_add(1, Ordering::SeqCst) >= MAX_HANDLERS {
                        busy.fetch_sub(1, Ordering::SeqCst);
                        reply(req, 503, "text/plain", "Busy".into());
                        continue;
                    }
                    let (token, on_event, done, stopping) = (
                        Arc::clone(&token),
                        Arc::clone(&on_event),
                        Arc::clone(&busy),
                        Arc::clone(&stopping),
                    );
                    let spawned = std::thread::Builder::new()
                        .name("libreri-phone-request".into())
                        .spawn(move || {
                            let emit = |e: PhoneEvent| {
                                // Nothing is reported once scanning has ended.
                                if !stopping.load(Ordering::SeqCst) {
                                    (on_event.lock().unwrap_or_else(|p| p.into_inner()))(e)
                                }
                            };
                            handle(req, &token, mode, &emit);
                            done.fetch_sub(1, Ordering::SeqCst);
                        });
                    if spawned.is_err() {
                        busy.fetch_sub(1, Ordering::SeqCst);
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            stop,
            thread: Some(thread),
            pairing,
        })
    }

    pub fn pairing(&self) -> &PhonePairing {
        &self.pairing
    }

    /// True while the page still works.
    pub fn is_running(&self) -> bool {
        self.thread.as_ref().is_some_and(|t| !t.is_finished())
    }
}

impl Drop for PhoneScanner {
    /// Stops the page. Waits a moment for the server to close, but never
    /// longer: requests still being answered finish on their own.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let until = Instant::now() + STOP_WAIT;
            while !t.is_finished() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(10));
            }
            if t.is_finished() {
                let _ = t.join();
            }
        }
    }
}

/// Reads a body, giving up when it takes longer than `limit` in all.
struct Deadline<R> {
    inner: R,
    until: Instant,
}

impl<R: Read> Read for Deadline<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if Instant::now() > self.until {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "the photo took too long to arrive",
            ));
        }
        self.inner.read(buf)
    }
}

fn header(k: &str, v: &str) -> Header {
    Header::from_bytes(k.as_bytes(), v.as_bytes()).expect("valid header")
}

fn reply(req: tiny_http::Request, status: u16, kind: &str, body: String) {
    let res = Response::from_string(body)
        .with_status_code(status)
        .with_header(header("Content-Type", kind))
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("Referrer-Policy", "no-referrer"))
        .with_header(header(
            "Content-Security-Policy",
            "default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; connect-src 'self'; img-src blob:",
        ));
    let _ = req.respond(res);
}

fn json_result(found: Option<&Scanned>) -> String {
    match found {
        Some(s) => format!(r#"{{"found":true,"code":"{}"}}"#, s.code),
        None => r#"{"found":false}"#.into(),
    }
}

fn read_body(req: &mut tiny_http::Request, max: usize) -> Option<Vec<u8>> {
    if req.body_length().is_some_and(|n| n > max) {
        return None;
    }
    let mut body = Vec::new();
    Deadline {
        inner: req.as_reader(),
        until: Instant::now() + BODY_TIME,
    }
    .take(max as u64 + 1)
    .read_to_end(&mut body)
    .ok()?;
    (body.len() <= max).then_some(body)
}

fn handle(
    mut req: tiny_http::Request,
    token: &str,
    mode: PhoneMode,
    on_event: &dyn Fn(PhoneEvent),
) {
    let path = req.url().split('?').next().unwrap_or("").to_owned();
    let Some(rest) = path.strip_prefix('/').and_then(|p| p.strip_prefix(token)) else {
        return reply(req, 404, "text/plain", "Not found".into());
    };
    match (req.method(), rest) {
        (Method::Get, "" | "/") => {
            on_event(PhoneEvent::Opened);
            let page = match mode {
                PhoneMode::Barcode => PAGE,
                PhoneMode::Pages => PAGES_PAGE,
            };
            reply(
                req,
                200,
                "text/html; charset=utf-8",
                page.replace("{TOKEN}", token),
            );
        }
        (Method::Post, "/page") if mode == PhoneMode::Pages => {
            let Some(body) = read_body(&mut req, MAX_PHOTO) else {
                return reply(req, 413, "text/plain", "Too large".into());
            };
            let kind = image::guess_format(&body).ok();
            let ok = matches!(
                kind,
                Some(image::ImageFormat::Jpeg | image::ImageFormat::Png)
            ) && image::load_from_memory(&body).is_ok();
            if !ok {
                return reply(req, 415, "text/plain", "Not a picture".into());
            }
            on_event(PhoneEvent::Page(body));
            reply(req, 200, "application/json", r#"{"ok":true}"#.into());
        }
        (Method::Post, "/photo") if mode == PhoneMode::Barcode => {
            let Some(body) = read_body(&mut req, MAX_PHOTO) else {
                return reply(req, 413, "text/plain", "Too large".into());
            };
            match crate::decode(&body) {
                Ok(found) => {
                    if let Some(s) = &found {
                        on_event(PhoneEvent::Scanned(s.clone()));
                    }
                    reply(req, 200, "application/json", json_result(found.as_ref()));
                }
                Err(_) => reply(req, 415, "text/plain", "Not a picture".into()),
            }
        }
        (Method::Post, "/code") if mode == PhoneMode::Barcode => {
            let text = read_body(&mut req, 64)
                .and_then(|b| String::from_utf8(b).ok())
                .unwrap_or_default();
            let s = Scanned::new(text.trim());
            let ok = s.isbn13.is_some()
                || (s.code.len() >= 8 && s.code.chars().all(|c| c.is_ascii_digit()));
            if ok {
                on_event(PhoneEvent::Scanned(s.clone()));
            }
            reply(req, 200, "application/json", json_result(ok.then_some(&s)));
        }
        _ => reply(req, 404, "text/plain", "Not found".into()),
    }
}

/// The phone page. Self-contained: no fonts, scripts or images from
/// elsewhere.
const PAGE: &str = r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Libreri – Scan a barcode</title>
<style>
  :root { color-scheme: light dark; --fg: #111; --bg: #fff; --muted: #666; --line: #ddd; --accent: #111; --on-accent: #fff; }
  @media (prefers-color-scheme: dark) { :root { --fg: #f2f2f2; --bg: #111; --muted: #aaa; --line: #333; --accent: #f2f2f2; --on-accent: #111; } }
  * { box-sizing: border-box; }
  body { margin: 0; font: 16px/1.45 -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; color: var(--fg); background: var(--bg); }
  main { max-width: 28rem; margin: 0 auto; padding: 24px 16px 40px; }
  h1 { font-size: 1.35rem; margin: 0 0 4px; }
  p { margin: 0 0 16px; color: var(--muted); }
  .take { display: flex; align-items: center; justify-content: center; width: 100%; min-height: 56px; border-radius: 12px; background: var(--accent); color: var(--on-accent); font-weight: 600; font-size: 1.05rem; cursor: pointer; }
  input[type=file] { position: absolute; opacity: 0; width: 1px; height: 1px; }
  #status { margin: 16px 0; min-height: 1.5em; font-weight: 500; color: var(--fg); }
  #status.bad { color: #c62828; }
  #status.good { color: #2e7d32; }
  img { display: none; max-width: 100%; border-radius: 8px; border: 1px solid var(--line); margin-bottom: 16px; }
  form { display: flex; gap: 8px; margin-top: 24px; padding-top: 16px; border-top: 1px solid var(--line); }
  form input { flex: 1; min-width: 0; font: inherit; padding: 10px 12px; border-radius: 10px; border: 1px solid var(--line); background: transparent; color: inherit; }
  form button { font: inherit; padding: 10px 14px; border-radius: 10px; border: 1px solid var(--line); background: transparent; color: inherit; }
</style>
</head>
<body>
<main>
  <h1>Scan a barcode</h1>
  <p>Take a photo of the barcode on the back of the book. Fill the frame with it, in good light.</p>
  <label class="take">Take photo<input id="photo" type="file" accept="image/*" capture="environment"></label>
  <div id="status" role="status" aria-live="polite"></div>
  <img id="preview" alt="The photo you took">
  <form id="manual">
    <input id="code" inputmode="numeric" autocomplete="off" placeholder="Or type the ISBN" aria-label="ISBN">
    <button>Send</button>
  </form>
</main>
<script>
const base = "/{TOKEN}";
const status = document.getElementById("status");
const say = (text, kind) => { status.textContent = text; status.className = kind || ""; };
const done = (r) => r.found
  ? say("Sent " + r.code + " to Libreri. You can take another.", "good")
  : say("No barcode found. Try again closer, straight on, with good light.", "bad");
const fail = () => say("Could not reach Libreri. Is scanning still open on the computer?", "bad");

async function shrink(file) {
  const url = URL.createObjectURL(file);
  const img = new Image();
  await new Promise((ok, bad) => { img.onload = ok; img.onerror = bad; img.src = url; });
  const preview = document.getElementById("preview");
  preview.src = url; preview.style.display = "block";
  const scale = Math.min(1, 1600 / Math.max(img.naturalWidth, img.naturalHeight));
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(img.naturalWidth * scale);
  canvas.height = Math.round(img.naturalHeight * scale);
  canvas.getContext("2d").drawImage(img, 0, 0, canvas.width, canvas.height);
  return new Promise((ok) => canvas.toBlob(ok, "image/jpeg", 0.9));
}

document.getElementById("photo").addEventListener("change", async (e) => {
  const file = e.target.files[0];
  if (!file) return;
  say("Reading the barcode…");
  try {
    const body = await shrink(file).catch(() => file);
    const res = await fetch(base + "/photo", { method: "POST", body });
    if (!res.ok) throw new Error();
    done(await res.json());
  } catch { fail(); }
  e.target.value = "";
});

document.getElementById("manual").addEventListener("submit", async (e) => {
  e.preventDefault();
  const input = document.getElementById("code");
  const text = input.value.trim();
  if (!text) return;
  try {
    const res = await fetch(base + "/code", { method: "POST", body: text });
    const r = await res.json();
    if (r.found) input.value = "";
    r.found ? done(r) : say("That is not an ISBN. Check the digits.", "bad");
  } catch { fail(); }
});
</script>
</body>
</html>
"##;

/// The phone page for photos of paper pages.
const PAGES_PAGE: &str = r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Libreri – Add pages</title>
<style>
  :root { color-scheme: light dark; --fg: #111; --bg: #fff; --muted: #666; --line: #ddd; --accent: #111; --on-accent: #fff; }
  @media (prefers-color-scheme: dark) { :root { --fg: #f2f2f2; --bg: #111; --muted: #aaa; --line: #333; --accent: #f2f2f2; --on-accent: #111; } }
  * { box-sizing: border-box; }
  body { margin: 0; font: 16px/1.45 -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; color: var(--fg); background: var(--bg); }
  main { max-width: 28rem; margin: 0 auto; padding: 24px 16px 40px; }
  h1 { font-size: 1.35rem; margin: 0 0 4px; }
  p { margin: 0 0 16px; color: var(--muted); }
  .take { display: flex; align-items: center; justify-content: center; width: 100%; min-height: 56px; border-radius: 12px; background: var(--accent); color: var(--on-accent); font-weight: 600; font-size: 1.05rem; cursor: pointer; }
  input[type=file] { position: absolute; opacity: 0; width: 1px; height: 1px; }
  #status { margin: 16px 0; min-height: 1.5em; font-weight: 500; }
  #status.bad { color: #c62828; }
  #status.good { color: #2e7d32; }
  #sent { display: grid; grid-template-columns: repeat(4, 1fr); gap: 8px; }
  #sent img { width: 100%; aspect-ratio: 3 / 4; object-fit: cover; border-radius: 6px; border: 1px solid var(--line); }
</style>
</head>
<body>
<main>
  <h1>Add pages</h1>
  <p>Photograph each page flat, from straight above, in good light. Every photo goes to Libreri as a new page.</p>
  <label class="take">Take photo<input id="photo" type="file" accept="image/*" capture="environment" multiple></label>
  <div id="status" role="status" aria-live="polite"></div>
  <div id="sent"></div>
</main>
<script>
const base = "/{TOKEN}";
const status = document.getElementById("status");
const say = (text, kind) => { status.textContent = text; status.className = kind || ""; };
let count = 0;

async function shrink(file) {
  const url = URL.createObjectURL(file);
  const img = new Image();
  await new Promise((ok, bad) => { img.onload = ok; img.onerror = bad; img.src = url; });
  const scale = Math.min(1, 2400 / Math.max(img.naturalWidth, img.naturalHeight));
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(img.naturalWidth * scale);
  canvas.height = Math.round(img.naturalHeight * scale);
  canvas.getContext("2d").drawImage(img, 0, 0, canvas.width, canvas.height);
  const blob = await new Promise((ok) => canvas.toBlob(ok, "image/jpeg", 0.88));
  return { blob, url };
}

document.getElementById("photo").addEventListener("change", async (e) => {
  for (const file of e.target.files) {
    say("Sending…");
    try {
      const { blob, url } = await shrink(file);
      const res = await fetch(base + "/page", { method: "POST", body: blob });
      if (!res.ok) throw new Error();
      count += 1;
      const thumb = document.createElement("img");
      thumb.src = url; thumb.alt = "Page " + count;
      document.getElementById("sent").appendChild(thumb);
      say(count + (count === 1 ? " page" : " pages") + " sent. Take the next one.", "good");
    } catch { say("Could not reach Libreri. Is the window still open on the computer?", "bad"); }
  }
  e.target.value = "";
});
</script>
</body>
</html>
"##;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpStream;
    use std::sync::Mutex;

    fn request(url: &str, method: &str, body: &[u8]) -> (u16, String) {
        let rest = url.strip_prefix("http://").unwrap();
        let (host, path) = rest.split_once('/').unwrap();
        let mut s = TcpStream::connect(host).unwrap();
        write!(
            s,
            "{method} /{path} HTTP/1.1\r\nHost: {host}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        s.write_all(body).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        let status = out[9..12].parse().unwrap();
        let body = out
            .split_once("\r\n\r\n")
            .map(|(_, b)| b.to_owned())
            .unwrap_or_default();
        (status, body)
    }

    #[test]
    fn serves_the_page_and_reads_photos() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        let scanner = PhoneScanner::start_on(
            "127.0.0.1".parse().unwrap(),
            PhoneMode::Barcode,
            Duration::from_secs(30),
            move |e| log.lock().unwrap().push(e),
        )
        .unwrap();
        let url = scanner.pairing().url.clone();
        assert!(scanner.pairing().qr_svg.starts_with("<?xml"));
        assert!(scanner.is_running());

        let (status, page) = request(&url, "GET", b"");
        assert_eq!(status, 200);
        assert!(page.contains("Scan a barcode"));
        let token = url.rsplit('/').next().unwrap();
        assert!(page.contains(&format!("\"/{token}\"")));

        // Without the token: nothing.
        let wrong = url.replace(token, "0000");
        assert_eq!(request(&wrong, "GET", b"").0, 404);

        let (status, body) = request(
            &format!("{url}/photo"),
            "POST",
            &crate::testing::barcode_png("9780306406157", false),
        );
        assert_eq!(
            (status, body.as_str()),
            (200, r#"{"found":true,"code":"9780306406157"}"#)
        );
        assert_eq!(request(&format!("{url}/photo"), "POST", b"junk").0, 415);
        let (_, body) = request(&format!("{url}/code"), "POST", b"0-306-40615-2");
        assert!(body.contains("\"found\":true"));
        let (_, body) = request(&format!("{url}/code"), "POST", b"hello");
        assert!(body.contains("\"found\":false"));

        drop(scanner);
        let events = seen.lock().unwrap().clone();
        assert_eq!(events[0], PhoneEvent::Opened);
        assert_eq!(events.len(), 3, "{events:?}");
        std::thread::sleep(Duration::from_millis(300));
        assert!(
            TcpStream::connect(url[7..].split('/').next().unwrap()).is_err(),
            "the server stops with the scanner"
        );
    }

    #[test]
    fn takes_photos_of_pages() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        let scanner = PhoneScanner::start_on(
            "127.0.0.1".parse().unwrap(),
            PhoneMode::Pages,
            Duration::from_secs(30),
            move |e| log.lock().unwrap().push(e),
        )
        .unwrap();
        let url = scanner.pairing().url.clone();
        let (status, page) = request(&url, "GET", b"");
        assert_eq!(status, 200);
        assert!(page.contains("Add pages"));
        let png = crate::testing::barcode_png("9780306406157", false);
        assert_eq!(request(&format!("{url}/page"), "POST", &png).0, 200);
        assert_eq!(request(&format!("{url}/page"), "POST", b"junk").0, 415);
        // Barcodes are not read on this page.
        assert_eq!(request(&format!("{url}/photo"), "POST", &png).0, 404);
        drop(scanner);
        let events = seen.lock().unwrap().clone();
        assert_eq!(events.len(), 2);
        assert_eq!(events[1], PhoneEvent::Page(png));
    }

    #[test]
    fn a_stalled_upload_holds_up_nothing() {
        let scanner = PhoneScanner::start_on(
            "127.0.0.1".parse().unwrap(),
            PhoneMode::Barcode,
            Duration::from_secs(30),
            |_| {},
        )
        .unwrap();
        let url = scanner.pairing().url.clone();
        let rest = url.strip_prefix("http://").unwrap();
        let (host, path) = rest.split_once('/').unwrap();
        // A photo that promises 1000 bytes and sends 10, then stops.
        let mut stalled = TcpStream::connect(host).unwrap();
        write!(
            stalled,
            "POST /{path}/photo HTTP/1.1\r\nHost: {host}\r\nContent-Length: 1000\r\n\r\n0123456789"
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(300));
        // Others are still answered.
        assert_eq!(request(&url, "GET", b"").0, 200);
        // And stopping does not wait for the stalled upload.
        let started = Instant::now();
        drop(scanner);
        assert!(started.elapsed() < Duration::from_secs(3));
        drop(stalled);
    }
}
