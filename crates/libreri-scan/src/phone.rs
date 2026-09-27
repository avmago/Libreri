//! Scanning with a phone: a small web server on the local network serves a
//! one-time page (its address holds a random token and is shown as a QR
//! code). The phone takes a photo of the barcode with its own camera app
//! and sends it; the computer reads the barcode.
//!
//! The server listens only on the computer's local network address, only
//! answers requests carrying the token, accepts only pictures and short
//! codes, and stops after a few minutes or when scanning ends.

use crate::Scanned;
use rand_core::{OsRng, RngCore};
use std::io::Read;
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tiny_http::{Header, Method, Response, Server};

/// Largest photo accepted (the page scales photos down before sending).
const MAX_PHOTO: usize = 10 * 1024 * 1024;

/// What the phone page did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhoneEvent {
    /// The phone opened the page.
    Opened,
    /// A barcode was read (from a photo, or typed on the phone).
    Scanned(Scanned),
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
        let ip = local_ip_address::local_ip()
            .map_err(|_| "this computer does not seem to be on a network".to_owned())?;
        Self::start_on(ip, lifetime, on_event)
    }

    pub(crate) fn start_on(
        ip: IpAddr,
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
        let thread = std::thread::Builder::new()
            .name("libreri-phone-scan".into())
            .spawn(move || {
                while !stopping.load(Ordering::SeqCst) && Instant::now() < deadline {
                    match server.recv_timeout(Duration::from_millis(250)) {
                        Ok(Some(req)) => handle(req, &token, &on_event),
                        Ok(None) => {}
                        Err(_) => break,
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
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
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
    req.as_reader()
        .take(max as u64 + 1)
        .read_to_end(&mut body)
        .ok()?;
    (body.len() <= max).then_some(body)
}

fn handle(mut req: tiny_http::Request, token: &str, on_event: &dyn Fn(PhoneEvent)) {
    let path = req.url().split('?').next().unwrap_or("").to_owned();
    let Some(rest) = path.strip_prefix('/').and_then(|p| p.strip_prefix(token)) else {
        return reply(req, 404, "text/plain", "Not found".into());
    };
    match (req.method(), rest) {
        (Method::Get, "" | "/") => {
            on_event(PhoneEvent::Opened);
            reply(
                req,
                200,
                "text/html; charset=utf-8",
                PAGE.replace("{TOKEN}", token),
            );
        }
        (Method::Post, "/photo") => {
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
        (Method::Post, "/code") => {
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
}
