//! Speaking with eSpeak NG, for systems whose web view has no voices of
//! its own (often Linux). The interface reads one sentence at a time, so
//! it knows what to highlight.

use serde::Serialize;
use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

/// A voice eSpeak NG offers.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Voice {
    /// What to pass to `-v` (a language such as "en-gb").
    pub id: String,
    pub name: String,
    pub lang: String,
}

/// Parses `espeak-ng --voices`:
/// `Pty Language Age/Gender VoiceName File Other Languages`.
pub fn parse_voices(text: &str) -> Vec<Voice> {
    text.lines()
        .skip(1)
        .filter_map(|l| {
            let cols: Vec<&str> = l.split_whitespace().collect();
            let (lang, name) = (cols.get(1)?, cols.get(3)?);
            Some(Voice {
                id: (*lang).to_owned(),
                name: name.replace('_', " "),
                lang: (*lang).to_owned(),
            })
        })
        .collect()
}

/// The voices, or none when eSpeak NG is not installed.
pub fn voices() -> Vec<Voice> {
    let Some(mut cmd) = crate::command("espeak-ng") else {
        return Vec::new();
    };
    cmd.arg("--voices")
        .output()
        .map(|o| parse_voices(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

/// Speaks one piece of text at a time; `stop` cuts it short.
#[derive(Default)]
pub struct Speaker {
    /// The speaking program, with the number of the `speak` call that
    /// started it, so an earlier call never mistakes a later one's program
    /// for its own.
    child: Mutex<Option<(u64, Child)>>,
    calls: AtomicU64,
}

impl Speaker {
    /// Speaks `text` and returns when it is done: `Ok(true)` when it was
    /// spoken to the end, `Ok(false)` when stopped (or replaced by another
    /// `speak`). `rate` is 1 for normal speed.
    pub fn speak(&self, text: &str, voice: Option<&str>, rate: f64) -> Result<bool, String> {
        let mut cmd = crate::command("espeak-ng").ok_or("eSpeak NG is not installed")?;
        let wpm = (175.0 * rate).clamp(80.0, 500.0).round() as u32;
        cmd.args(["--stdin", "-s", &wpm.to_string()]);
        if let Some(v) = voice.filter(|v| !v.is_empty()) {
            cmd.args(["-v", v]);
        }
        self.run(cmd, text)
    }

    fn run(&self, mut cmd: Command, text: &str) -> Result<bool, String> {
        self.stop();
        let me = self.calls.fetch_add(1, Ordering::SeqCst);
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        if let Some(mut input) = child.stdin.take() {
            let _ = input.write_all(text.as_bytes());
        }
        {
            let mut slot = self.child.lock().unwrap_or_else(|p| p.into_inner());
            // Another call may have started meanwhile; this one gives way.
            if let Some((_, mut old)) = slot.take() {
                let _ = old.kill();
                let _ = old.wait();
            }
            *slot = Some((me, child));
        }
        loop {
            {
                let mut slot = self.child.lock().unwrap_or_else(|p| p.into_inner());
                match slot.as_mut() {
                    // Stopped, or another call's program took its place.
                    None => return Ok(false),
                    Some((call, _)) if *call != me => return Ok(false),
                    Some((_, c)) => {
                        if let Some(status) = c.try_wait().map_err(|e| e.to_string())? {
                            *slot = None;
                            return Ok(status.success());
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    pub fn stop(&self) {
        let taken = self.child.lock().unwrap_or_else(|p| p.into_inner()).take();
        if let Some((_, mut c)) = taken {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_voice_list() {
        let out = "Pty Language       Age/Gender VoiceName          File                 Other Languages\n \
                   5  af              --/M      Afrikaans          gmw/af\n \
                   2  en-gb           --/M      English_(Great_Britain) gmw/en            (en 2)\n";
        let v = parse_voices(out);
        assert_eq!(v.len(), 2);
        assert_eq!(
            (v[1].id.as_str(), v[1].name.as_str()),
            ("en-gb", "English (Great Britain)")
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_new_speech_stops_the_old_one_and_is_not_mistaken_for_it() {
        let speaker = std::sync::Arc::new(Speaker::default());
        let sh = |script: &str| {
            let mut c = Command::new("sh");
            c.args(["-c", script]);
            c
        };
        let first = {
            let speaker = std::sync::Arc::clone(&speaker);
            std::thread::spawn(move || speaker.run(sh("sleep 5"), ""))
        };
        std::thread::sleep(Duration::from_millis(200));
        let started = std::time::Instant::now();
        // The second speaks to the end; the first reports that it stopped.
        assert_eq!(speaker.run(sh("sleep 0.3"), ""), Ok(true));
        assert_eq!(first.join().unwrap(), Ok(false));
        assert!(started.elapsed() < Duration::from_secs(4));
        // Stopping ends a speech early.
        let third = {
            let speaker = std::sync::Arc::clone(&speaker);
            std::thread::spawn(move || speaker.run(sh("sleep 5"), ""))
        };
        std::thread::sleep(Duration::from_millis(200));
        speaker.stop();
        assert_eq!(third.join().unwrap(), Ok(false));
    }
}
