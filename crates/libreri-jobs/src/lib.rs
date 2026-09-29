//! A small background job queue.
//!
//! Long work (importing, thumbnails, metadata lookups, OCR) runs on worker
//! threads so the interface stays responsive. Each job reports progress
//! through a [`JobContext`] and can be cancelled. Events go to a single
//! [`EventSink`], which the Tauri shell forwards to the UI.

use serde::Serialize;
use std::collections::HashMap;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use uuid::Uuid;

/// Identifier of a submitted job.
pub type JobId = Uuid;

/// What happened to a job; sent to the [`EventSink`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum JobEvent {
    Started {
        id: JobId,
        label: String,
    },
    Progress {
        id: JobId,
        done: u64,
        total: u64,
        message: Option<String>,
    },
    Finished {
        id: JobId,
    },
    Failed {
        id: JobId,
        error: String,
    },
    Cancelled {
        id: JobId,
    },
}

/// Receives job events. The Tauri shell implements this to emit UI events.
pub trait EventSink: Send + Sync + 'static {
    fn emit(&self, event: JobEvent);
}

impl<F: Fn(JobEvent) + Send + Sync + 'static> EventSink for F {
    fn emit(&self, event: JobEvent) {
        self(event)
    }
}

/// Returned by a job to stop early because it was cancelled.
#[derive(Debug, thiserror::Error)]
#[error("cancelled")]
pub struct Cancelled;

/// Handed to a running job for reporting progress and checking for
/// cancellation.
pub struct JobContext {
    id: JobId,
    cancel: Arc<AtomicBool>,
    sink: Arc<dyn EventSink>,
}

impl JobContext {
    pub fn id(&self) -> JobId {
        self.id
    }

    pub fn progress(&self, done: u64, total: u64, message: Option<String>) {
        self.sink.emit(JobEvent::Progress {
            id: self.id,
            done,
            total,
            message,
        });
    }

    /// Call between steps; returns `Err(Cancelled)` once cancellation was
    /// requested.
    pub fn check_cancelled(&self) -> Result<(), Cancelled> {
        if self.cancel.load(Ordering::Relaxed) {
            Err(Cancelled)
        } else {
            Ok(())
        }
    }
}

/// The error type a job returns: a message for the user, or cancellation.
#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error(transparent)]
    Cancelled(#[from] Cancelled),
    #[error("{0}")]
    Failed(String),
}

type Work = Box<dyn FnOnce(&JobContext) -> Result<(), JobError> + Send + 'static>;

struct Task {
    id: JobId,
    label: String,
    work: Work,
}

/// Locks a mutex even if a thread panicked while holding it (the data
/// here stays usable), so one failure does not stop the whole queue.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// The message of a panic, for the user.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    let detail = payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned());
    match detail {
        Some(d) => format!("something went wrong: {d}"),
        None => "something went wrong".to_owned(),
    }
}

/// A fixed-size pool of worker threads running submitted jobs in order.
pub struct JobQueue {
    sender: Option<Sender<Task>>,
    workers: Vec<JoinHandle<()>>,
    cancels: Arc<Mutex<HashMap<JobId, Arc<AtomicBool>>>>,
}

impl JobQueue {
    pub fn new(threads: usize, sink: impl EventSink) -> Self {
        let sink: Arc<dyn EventSink> = Arc::new(sink);
        let (sender, receiver) = mpsc::channel::<Task>();
        let receiver = Arc::new(Mutex::new(receiver));
        let cancels: Arc<Mutex<HashMap<JobId, Arc<AtomicBool>>>> = Arc::default();

        let workers = (0..threads.max(1))
            .map(|_| {
                let receiver = Arc::clone(&receiver);
                let sink = Arc::clone(&sink);
                let cancels = Arc::clone(&cancels);
                std::thread::spawn(move || loop {
                    let task = match lock(&receiver).recv() {
                        Ok(task) => task,
                        Err(_) => break, // queue dropped
                    };
                    let cancel = lock(&cancels).get(&task.id).cloned().unwrap_or_default();
                    let ctx = JobContext {
                        id: task.id,
                        cancel,
                        sink: Arc::clone(&sink),
                    };
                    if ctx.check_cancelled().is_err() {
                        sink.emit(JobEvent::Cancelled { id: task.id });
                    } else {
                        sink.emit(JobEvent::Started {
                            id: task.id,
                            label: task.label,
                        });
                        // A job that panics is reported as failed; the worker
                        // carries on with the next job.
                        let work = task.work;
                        let result = panic::catch_unwind(AssertUnwindSafe(|| work(&ctx)));
                        let event = match result {
                            Ok(Ok(())) => JobEvent::Finished { id: task.id },
                            Ok(Err(JobError::Cancelled(_))) => JobEvent::Cancelled { id: task.id },
                            Ok(Err(JobError::Failed(error))) => {
                                JobEvent::Failed { id: task.id, error }
                            }
                            Err(payload) => JobEvent::Failed {
                                id: task.id,
                                error: panic_message(payload.as_ref()),
                            },
                        };
                        sink.emit(event);
                    }
                    lock(&cancels).remove(&task.id);
                })
            })
            .collect();

        Self {
            sender: Some(sender),
            workers,
            cancels,
        }
    }

    /// Queues a job and returns its id.
    pub fn submit<F>(&self, label: impl Into<String>, work: F) -> JobId
    where
        F: FnOnce(&JobContext) -> Result<(), JobError> + Send + 'static,
    {
        let id = Uuid::new_v4();
        lock(&self.cancels).insert(id, Arc::default());
        let task = Task {
            id,
            label: label.into(),
            work: Box::new(work),
        };
        if let Some(sender) = &self.sender {
            // Only fails if every worker has exited, which only happens on drop.
            let _ = sender.send(task);
        }
        id
    }

    /// Requests cancellation; the job stops at its next `check_cancelled`.
    pub fn cancel(&self, id: JobId) -> bool {
        match lock(&self.cancels).get(&id) {
            Some(flag) => {
                flag.store(true, Ordering::Relaxed);
                true
            }
            None => false,
        }
    }
}

impl Drop for JobQueue {
    fn drop(&mut self) {
        // Closing the channel lets workers finish queued jobs and exit.
        self.sender.take();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::Receiver;
    use std::time::Duration;

    fn queue(threads: usize) -> (JobQueue, Receiver<JobEvent>) {
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let q = JobQueue::new(threads, move |e| {
            let _ = tx.lock().unwrap().send(e);
        });
        (q, rx)
    }

    fn until_done(rx: &Receiver<JobEvent>, id: JobId) -> Vec<JobEvent> {
        let mut seen = Vec::new();
        loop {
            let e = rx
                .recv_timeout(Duration::from_secs(5))
                .expect("job did not finish");
            let done = matches!(&e,
                JobEvent::Finished { id: i } | JobEvent::Failed { id: i, .. } | JobEvent::Cancelled { id: i } if *i == id);
            seen.push(e);
            if done {
                return seen;
            }
        }
    }

    #[test]
    fn a_job_reports_progress_and_finishes() {
        let (q, rx) = queue(2);
        let id = q.submit("count", |ctx| {
            for i in 1..=3 {
                ctx.progress(i, 3, None);
            }
            Ok(())
        });
        let events = until_done(&rx, id);
        assert!(matches!(events.first(), Some(JobEvent::Started { .. })));
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, JobEvent::Progress { .. }))
                .count(),
            3
        );
        assert_eq!(events.last(), Some(&JobEvent::Finished { id }));
    }

    #[test]
    fn a_failing_job_reports_its_message() {
        let (q, rx) = queue(1);
        let id = q.submit("fail", |_| Err(JobError::Failed("file is damaged".into())));
        let events = until_done(&rx, id);
        assert_eq!(
            events.last(),
            Some(&JobEvent::Failed {
                id,
                error: "file is damaged".into()
            })
        );
    }

    #[test]
    fn a_panicking_job_fails_and_the_queue_keeps_working() {
        let (q, rx) = queue(1);
        // More panics than workers: each must be reported, and later jobs
        // must still run.
        for _ in 0..3 {
            let id = q.submit("boom", |_| panic!("bad page"));
            match until_done(&rx, id).last() {
                Some(JobEvent::Failed { error, .. }) => assert!(error.contains("bad page")),
                other => panic!("expected a failure, got {other:?}"),
            }
        }
        let id = q.submit("fine", |_| Ok(()));
        assert_eq!(until_done(&rx, id).last(), Some(&JobEvent::Finished { id }));
    }

    #[test]
    fn a_job_cancelled_before_it_starts_is_dropped_unrun() {
        let (q, rx) = queue(1);
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let blocker = q.submit("block", move |_| {
            let _ = release_rx.recv_timeout(Duration::from_secs(5));
            Ok(())
        });
        struct Flag(Arc<AtomicBool>);
        impl Drop for Flag {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let dropped = Arc::new(AtomicBool::new(false));
        let flag = Flag(Arc::clone(&dropped));
        let ran = Arc::new(AtomicBool::new(false));
        let ran2 = Arc::clone(&ran);
        let id = q.submit("waiting", move |_| {
            let _keep = flag;
            ran2.store(true, Ordering::SeqCst);
            Ok(())
        });
        assert!(q.cancel(id));
        release_tx.send(()).unwrap();
        until_done(&rx, blocker);
        assert_eq!(
            until_done(&rx, id).last(),
            Some(&JobEvent::Cancelled { id })
        );
        assert!(!ran.load(Ordering::SeqCst));
        assert!(dropped.load(Ordering::SeqCst), "the unrun job was dropped");
    }

    #[test]
    fn cancelling_stops_a_running_job() {
        let (q, rx) = queue(1);
        let (started_tx, started_rx) = mpsc::channel();
        let id = q.submit("long", move |ctx| {
            started_tx.send(()).unwrap();
            loop {
                ctx.check_cancelled()?;
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(q.cancel(id));
        assert_eq!(
            until_done(&rx, id).last(),
            Some(&JobEvent::Cancelled { id })
        );
    }
}
