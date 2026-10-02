//! Web requests. Sources talk to an [`Http`] so tests can answer from
//! fixture files instead of the network.

use std::io::Read;
use std::time::Duration;

/// The User-Agent sent with every request: the app and where to find it,
/// never anything about the reader.
pub const USER_AGENT: &str = "Libreri/0.1 (+https://github.com/avmg0/Libreri)";

/// Biggest answer read (JSON, XML or a cover image).
pub const MAX_BODY: u64 = 12 * 1024 * 1024;

pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
    pub content_type: Option<String>,
}

impl Response {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

pub trait Http: Sync {
    /// A GET request. Errors are for requests that got no answer at all;
    /// an answer with an error status is returned as a [`Response`].
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, String>;
}

/// Requests over HTTPS with `ureq`. Uses the system's proxy settings
/// (HTTPS_PROXY and friends).
pub struct UreqHttp {
    agent: ureq::Agent,
}

impl Default for UreqHttp {
    fn default() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(12)))
            .user_agent(USER_AGENT)
            .http_status_as_error(false)
            .https_only(true)
            .max_redirects(4)
            .build();
        Self {
            agent: config.into(),
        }
    }
}

impl Http for UreqHttp {
    fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Response, String> {
        let mut req = self.agent.get(url);
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let mut res = req.call().map_err(describe)?;
        let status = res.status().as_u16();
        let content_type = res
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let mut body = Vec::new();
        res.body_mut()
            .as_reader()
            .take(MAX_BODY + 1)
            .read_to_end(&mut body)
            .map_err(|e| e.to_string())?;
        if body.len() as u64 > MAX_BODY {
            return Err("the answer was too large".into());
        }
        Ok(Response {
            status,
            body,
            content_type,
        })
    }
}

fn describe(e: ureq::Error) -> String {
    match e {
        ureq::Error::Timeout(_) => "it took too long to answer".into(),
        ureq::Error::HostNotFound => "it could not be found; check the internet connection".into(),
        ureq::Error::Io(e) => format!("it could not be reached ({e})"),
        other => other.to_string(),
    }
}

/// Reads a JSON answer, turning error statuses into messages.
pub(crate) fn json(res: Response) -> Result<serde_json::Value, String> {
    match res.status {
        s if (200..300).contains(&s) => serde_json::from_slice(&res.body)
            .map_err(|_| "it sent an answer that could not be read".into()),
        404 => Ok(serde_json::Value::Null),
        401 | 403 => Err("it refused the request; check the API key".into()),
        429 => Err("too many requests; try again in a minute".into()),
        s => Err(format!("it answered with an error ({s})")),
    }
}

/// Encodes a value for a URL query.
pub(crate) fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Answers from files instead of the network, for trying the interface
/// offline (debug builds, `LIBRERI_HTTP_FIXTURES=<folder>`). The folder
/// holds `routes.json`: `{"part of a URL": "file name", …}`; the longest
/// matching part wins.
pub struct FixtureHttp {
    dir: std::path::PathBuf,
    routes: Vec<(String, String)>,
}

impl FixtureHttp {
    pub fn new(dir: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(dir.join("routes.json")).map_err(|e| e.to_string())?;
        let map: std::collections::BTreeMap<String, String> =
            serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let mut routes: Vec<_> = map.into_iter().collect();
        routes.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));
        Ok(Self {
            dir: dir.to_path_buf(),
            routes,
        })
    }
}

impl Http for FixtureHttp {
    fn get(&self, url: &str, _headers: &[(&str, &str)]) -> Result<Response, String> {
        std::thread::sleep(Duration::from_millis(300));
        let Some((_, file)) = self.routes.iter().find(|(k, _)| url.contains(k.as_str())) else {
            return Err("it could not be reached (offline fixtures)".into());
        };
        if let Some(status) = file.strip_prefix("status:") {
            return Ok(Response {
                status: status.parse().unwrap_or(500),
                body: Vec::new(),
                content_type: None,
            });
        }
        let body = std::fs::read(self.dir.join(file)).map_err(|e| e.to_string())?;
        Ok(Response {
            status: 200,
            body,
            content_type: None,
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn encodes_queries() {
        assert_eq!(super::enc("Café & co/1"), "Caf%C3%A9%20%26%20co%2F1");
    }
}
