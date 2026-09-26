//! A small background job queue.
//!
//! Long work (importing, thumbnails, metadata lookups, OCR) runs on worker
//! threads so the interface stays responsive. Each job reports progress
//! through a [`JobContext`] and can be cancelled. Events go to a single
//! [`EventSink`], which the Tauri shell forwards to the UI.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
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
                    let task = match receiver.lock().expect("job queue poisoned").recv() {
                        Ok(task) => task,
                        Err(_) => break, // queue dropped
                    };
                    let cancel = cancels
                        .lock()
                        .expect("job queue poisoned")
                        .get(&task.id)
                        .cloned()
                        .unwrap_or_default();
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
                        let event = match (task.work)(&ctx) {
                            Ok(()) => JobEvent::Finished { id: task.id },
                            Err(JobError::Cancelled(_)) => JobEvent::Cancelled { id: task.id },
                            Err(JobError::Failed(error)) => JobEvent::Failed { id: task.id, error },
                        };
                        sink.emit(event);
                    }
                    cancels.lock().expect("job queue poisoned").remove(&task.id);
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
        self.cancels
            .lock()
            .expect("job queue poisoned")
            .insert(id, Arc::default());
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
        match self.cancels.lock().expect("job queue poisoned").get(&id) {
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
