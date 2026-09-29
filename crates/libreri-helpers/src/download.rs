//! Reading large downloads without giving up on slow connections.
//!
//! ureq's body timeout is a budget for the whole body, so a large file on a
//! slow connection fails part-way even though bytes keep arriving. Instead,
//! downloads set only a very generous overall limit ([`BODY_LIMIT`]) and
//! read through a [`StallReader`], which gives up when no bytes arrive for a
//! while ([`STALL`]) and stops promptly when cancelled.

use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// How long a download may go without receiving any bytes.
pub const STALL: Duration = Duration::from_secs(60);

/// A last-resort limit for a whole body, so a connection that hangs for
/// ever is eventually closed. Far longer than any real download takes.
pub const BODY_LIMIT: Duration = Duration::from_secs(12 * 60 * 60);

/// How often a waiting read looks at the cancel flag.
const POLL: Duration = Duration::from_millis(250);

/// Size of the pieces read on the helper thread.
const CHUNK: usize = 256 * 1024;

/// Reads another reader on a helper thread, so that a read can give up when
/// nothing arrives for `stall`, or when `cancel` is set. Giving up returns
/// an error: [`io::ErrorKind::TimedOut`] for a stall, and
/// an error with the message "cancelled" for cancelling (see
/// [`is_cancelled`]; not `Interrupted`, which readers retry).
///
/// The helper thread ends when the reader is dropped and its next read
/// returns (the connection's overall limit makes sure it does).
pub struct StallReader<'a> {
    rx: Receiver<io::Result<Vec<u8>>>,
    piece: Vec<u8>,
    at: usize,
    stall: Duration,
    cancel: Option<&'a AtomicBool>,
    ended: bool,
}

impl<'a> StallReader<'a> {
    pub fn new<R: Read + Send + 'static>(
        mut inner: R,
        stall: Duration,
        cancel: Option<&'a AtomicBool>,
    ) -> Self {
        // A few pieces in flight at most, so memory stays small.
        let (tx, rx) = mpsc::sync_channel(4);
        let spawned = std::thread::Builder::new()
            .name("libreri-download".into())
            .spawn(move || loop {
                let mut buf = vec![0u8; CHUNK];
                match inner.read(&mut buf) {
                    Ok(0) => {
                        let _ = tx.send(Ok(Vec::new()));
                        return;
                    }
                    Ok(n) => {
                        buf.truncate(n);
                        if tx.send(Ok(buf)).is_err() {
                            return; // the reader was dropped
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        return;
                    }
                }
            });
        let (rx, ended) = match spawned {
            Ok(_) => (rx, false),
            Err(e) => {
                // No thread: report the failure on the first read.
                let (tx, rx) = mpsc::sync_channel(1);
                let _ = tx.send(Err(e));
                (rx, false)
            }
        };
        Self {
            rx,
            piece: Vec::new(),
            at: 0,
            stall,
            cancel,
            ended,
        }
    }

    fn cancelled(&self) -> bool {
        self.cancel.is_some_and(|c| c.load(Ordering::SeqCst))
    }
}

/// Whether a read error means the download was cancelled.
pub fn is_cancelled(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::Other && e.to_string() == "cancelled"
}

impl Read for StallReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let since = Instant::now();
        while self.at >= self.piece.len() {
            if self.ended {
                return Ok(0);
            }
            if self.cancelled() {
                return Err(io::Error::other("cancelled"));
            }
            let left = self.stall.saturating_sub(since.elapsed());
            if left.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!(
                        "nothing was received for {} seconds",
                        self.stall.as_secs().max(1)
                    ),
                ));
            }
            match self.rx.recv_timeout(left.min(POLL)) {
                Ok(Ok(piece)) if piece.is_empty() => self.ended = true,
                Ok(Ok(piece)) => {
                    self.piece = piece;
                    self.at = 0;
                }
                Ok(Err(e)) => {
                    self.ended = true;
                    return Err(e);
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    self.ended = true;
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "the download ended unexpectedly",
                    ));
                }
            }
        }
        let n = buf.len().min(self.piece.len() - self.at);
        buf[..n].copy_from_slice(&self.piece[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gives some bytes, then blocks until told to go on.
    struct Slow {
        first: Option<Vec<u8>>,
        gate: Receiver<()>,
    }

    impl Read for Slow {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if let Some(first) = self.first.take() {
                buf[..first.len()].copy_from_slice(&first);
                return Ok(first.len());
            }
            let _ = self.gate.recv_timeout(Duration::from_secs(10));
            Ok(0)
        }
    }

    #[test]
    fn reads_everything_from_a_steady_source() {
        let data: Vec<u8> = (0..1_000_000u32).map(|i| i as u8).collect();
        let mut r = StallReader::new(io::Cursor::new(data.clone()), STALL, None);
        let mut out = Vec::new();
        r.read_to_end(&mut out).unwrap();
        assert_eq!(out, data);
    }

    #[test]
    fn gives_up_when_nothing_arrives() {
        let (_keep, gate) = mpsc::channel();
        let slow = Slow {
            first: Some(b"abc".to_vec()),
            gate,
        };
        let mut r = StallReader::new(slow, Duration::from_millis(300), None);
        let mut buf = [0u8; 16];
        assert_eq!(r.read(&mut buf).unwrap(), 3);
        let started = Instant::now();
        let e = r.read(&mut buf).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn stops_promptly_when_cancelled() {
        let (_keep, gate) = mpsc::channel();
        let slow = Slow { first: None, gate };
        let cancel = AtomicBool::new(false);
        std::thread::scope(|s| {
            s.spawn(|| {
                std::thread::sleep(Duration::from_millis(100));
                cancel.store(true, Ordering::SeqCst);
            });
            let mut r = StallReader::new(slow, STALL, Some(&cancel));
            let started = Instant::now();
            let e = r.read(&mut [0u8; 16]).unwrap_err();
            assert!(is_cancelled(&e));
            assert!(started.elapsed() < Duration::from_secs(5));
        });
    }
}
