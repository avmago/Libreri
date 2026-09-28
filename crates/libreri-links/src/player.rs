//! The player page for embedded videos. YouTube and other sites refuse
//! to play in a frame whose page has no web address (Libreri's own pages
//! are `tauri://` on macOS and Linux), so the side panel shows a tiny page
//! served on this computer (`http://127.0.0.1:<port>/<token>/play`) that
//! holds the site's player.
//!
//! The server listens on the loopback address only, answers only requests
//! carrying its random token, and only makes pages that frame an `https`
//! address.

use rand_core::{OsRng, RngCore};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use tiny_http::{Header, Response, Server};

pub struct PlayerServer {
    port: u16,
    token: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The page that holds a player (`src` must be an https address).
pub fn page(src: &str) -> Option<String> {
    let u = url::Url::parse(src).ok()?;
    if u.scheme() != "https" || u.host_str().is_none() {
        return None;
    }
    Some(format!(
        r#"<!doctype html>
<html><head><meta charset="utf-8"><meta name="referrer" content="strict-origin-when-cross-origin">
<title>Player</title>
<style>html,body{{margin:0;height:100%;background:#000;overflow:hidden}}iframe{{border:0;width:100%;height:100%;display:block}}</style>
</head><body><iframe src="{}" allow="autoplay; encrypted-media; fullscreen; picture-in-picture; clipboard-write" allowfullscreen referrerpolicy="strict-origin-when-cross-origin"></iframe></body></html>"#,
        escape(u.as_str())
    ))
}

impl PlayerServer {
    pub fn start() -> std::io::Result<Self> {
        let server = Server::http("127.0.0.1:0").map_err(std::io::Error::other)?;
        let port = server
            .server_addr()
            .to_ip()
            .map(|a| a.port())
            .ok_or_else(|| std::io::Error::other("no port"))?;
        let mut bytes = [0u8; 16];
        OsRng.fill_bytes(&mut bytes);
        let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let stop = Arc::new(AtomicBool::new(false));
        let (t, s) = (token.clone(), Arc::clone(&stop));
        let thread = std::thread::spawn(move || {
            while !s.load(Ordering::SeqCst) {
                let Ok(Some(req)) = server.recv_timeout(Duration::from_millis(300)) else {
                    continue;
                };
                let url = format!("http://127.0.0.1{}", req.url());
                let body = url::Url::parse(&url).ok().and_then(|u| {
                    (u.path() == format!("/{t}/play"))
                        .then(|| {
                            u.query_pairs()
                                .find(|(k, _)| k == "src")
                                .map(|(_, v)| v.into_owned())
                        })
                        .flatten()
                        .and_then(|src| page(&src))
                });
                let res = match body {
                    Some(html) => Response::from_string(html)
                        .with_header(header("Content-Type", "text/html; charset=utf-8"))
                        .with_header(header(
                            "Content-Security-Policy",
                            "default-src 'none'; frame-src https:; style-src 'unsafe-inline'",
                        ))
                        .with_header(header("Referrer-Policy", "strict-origin-when-cross-origin"))
                        .with_header(header("Cache-Control", "no-store")),
                    None => Response::from_string("Not found").with_status_code(404),
                };
                let _ = req.respond(res);
            }
        });
        Ok(Self {
            port,
            token,
            stop,
            thread: Some(thread),
        })
    }

    /// The player page's address for a site's player address.
    pub fn url_for(&self, src: &str) -> String {
        let enc: String = url::form_urlencoded::byte_serialize(src.as_bytes()).collect();
        format!(
            "http://127.0.0.1:{}/{}/play?src={enc}",
            self.port, self.token
        )
    }
}

fn header(k: &str, v: &str) -> Header {
    Header::from_bytes(k.as_bytes(), v.as_bytes()).expect("header")
}

impl Drop for PlayerServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    fn get(url: &str) -> String {
        let u = url::Url::parse(url).unwrap();
        let mut s = std::net::TcpStream::connect(("127.0.0.1", u.port().unwrap())).unwrap();
        let path = match u.query() {
            Some(q) => format!("{}?{q}", u.path()),
            None => u.path().to_owned(),
        };
        write!(s, "GET {path} HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn serves_player_pages_with_its_token_only() {
        let p = PlayerServer::start().unwrap();
        let ok = get(&p.url_for("https://www.youtube-nocookie.com/embed/abc?start=5&x=\"1\""));
        assert!(
            ok.starts_with("HTTP/1.1 200") || ok.starts_with("HTTP/1.0 200"),
            "{ok}"
        );
        assert!(
            ok.contains(
                r#"src="https://www.youtube-nocookie.com/embed/abc?start=5&amp;x=%221%22""#
            ),
            "{ok}"
        );
        let bad = get(&p.url_for("javascript:alert(1)"));
        assert!(bad.contains("404"));
        let wrong = get(&format!(
            "http://127.0.0.1:{}/nottoken/play?src=https%3A%2F%2Fa.b",
            p.port
        ));
        assert!(wrong.contains("404"));
    }
}
