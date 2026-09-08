use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
};

use tokio::sync::{Notify, Semaphore, oneshot};

type Job = Box<dyn FnOnce() + Send + 'static>;

#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    inner: Arc<CancellationState>,
}

#[derive(Debug, Default)]
struct CancellationState {
    cancelled: AtomicBool,
    notify: Notify,
}

impl CancellationToken {
    pub fn cancel(&self) {
        if !self.inner.cancelled.swap(true, Ordering::AcqRel) {
            self.inner.notify.notify_waiters();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }

    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }

        let notified = self.inner.notify.notified();
        if self.is_cancelled() {
            return;
        }
        notified.await;
    }
}

#[derive(Clone)]
pub struct AnalysisPool {
    inner: Arc<AnalysisPoolInner>,
}

struct AnalysisPoolInner {
    sender: Mutex<Option<Sender<Job>>>,
    workers: Mutex<Vec<thread::JoinHandle<()>>>,
    permits: Arc<Semaphore>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalysisPoolError {
    Cancelled,
    Closed,
    WorkerPanicked,
}

impl AnalysisPool {
    pub fn new(worker_count: usize, queue_capacity: usize) -> Self {
        assert!(worker_count > 0, "analysis pool needs at least one worker");
        let capacity = worker_count
            .checked_add(queue_capacity)
            .expect("analysis pool capacity overflowed usize");
        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(worker_count);

        for index in 0..worker_count {
            let receiver = Arc::clone(&receiver);
            workers.push(
                thread::Builder::new()
                    .name(format!("aivi-lsp-analysis-{index}"))
                    .spawn(move || worker_loop(&receiver))
                    .expect("failed to start an AIVI LSP analysis worker"),
            );
        }

        Self {
            inner: Arc::new(AnalysisPoolInner {
                sender: Mutex::new(Some(sender)),
                workers: Mutex::new(workers),
                permits: Arc::new(Semaphore::new(capacity)),
            }),
        }
    }

    pub async fn execute<T, F>(
        &self,
        cancellation: CancellationToken,
        work: F,
    ) -> Result<T, AnalysisPoolError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let permit = tokio::select! {
            permit = Arc::clone(&self.inner.permits).acquire_owned() => {
                permit.map_err(|_| AnalysisPoolError::Closed)?
            }
            () = cancellation.cancelled() => return Err(AnalysisPoolError::Cancelled),
        };

        if cancellation.is_cancelled() {
            return Err(AnalysisPoolError::Cancelled);
        }

        let (result_sender, result_receiver) = oneshot::channel();
        let job_cancellation = cancellation.clone();
        let job = Box::new(move || {
            let result = if job_cancellation.is_cancelled() {
                Err(AnalysisPoolError::Cancelled)
            } else {
                match catch_unwind(AssertUnwindSafe(work)) {
                    Ok(_) if job_cancellation.is_cancelled() => Err(AnalysisPoolError::Cancelled),
                    Ok(value) => Ok(value),
                    Err(_) => Err(AnalysisPoolError::WorkerPanicked),
                }
            };
            let _ = result_sender.send(result);
            drop(permit);
        });

        let sender = self
            .inner
            .sender
            .lock()
            .expect("analysis pool sender lock should not be poisoned")
            .as_ref()
            .cloned()
            .ok_or(AnalysisPoolError::Closed)?;
        sender.send(job).map_err(|_| AnalysisPoolError::Closed)?;

        tokio::select! {
            result = result_receiver => result.unwrap_or(Err(AnalysisPoolError::Closed)),
            () = cancellation.cancelled() => Err(AnalysisPoolError::Cancelled),
        }
    }
}

impl Default for AnalysisPool {
    fn default() -> Self {
        let worker_count = thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(2)
            .clamp(1, 4);
        Self::new(worker_count, worker_count * 2)
    }
}

impl Drop for AnalysisPoolInner {
    fn drop(&mut self) {
        self.sender
            .get_mut()
            .expect("analysis pool sender lock should not be poisoned")
            .take();
        for worker in self
            .workers
            .get_mut()
            .expect("analysis pool worker lock should not be poisoned")
            .drain(..)
        {
            let _ = worker.join();
        }
    }
}

fn worker_loop(receiver: &Mutex<Receiver<Job>>) {
    loop {
        let job = receiver
            .lock()
            .expect("analysis pool receiver lock should not be poisoned")
            .recv();
        match job {
            Ok(job) => job(),
            Err(_) => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        time::Duration,
    };

    use super::{AnalysisPool, AnalysisPoolError, CancellationToken};

    #[tokio::test]
    async fn work_runs_on_named_dedicated_thread() {
        let pool = AnalysisPool::new(1, 0);
        let thread_name = pool
            .execute(CancellationToken::default(), || {
                std::thread::current().name().map(str::to_owned)
            })
            .await
            .expect("analysis should complete")
            .expect("worker should have a name");

        assert_eq!(thread_name, "aivi-lsp-analysis-0");
    }

    #[tokio::test]
    async fn cancellation_skips_queued_work() {
        let pool = AnalysisPool::new(1, 1);
        let (started_sender, started_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        let first_pool = pool.clone();
        let first = tokio::spawn(async move {
            first_pool
                .execute(CancellationToken::default(), move || {
                    started_sender.send(()).expect("test receiver should exist");
                    release_receiver
                        .recv()
                        .expect("test should release blocked analysis");
                })
                .await
        });
        tokio::task::yield_now().await;
        started_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("first analysis should start");

        let cancellation = CancellationToken::default();
        let queued_cancellation = cancellation.clone();
        let queued_pool = pool.clone();
        let queued_work_ran = Arc::new(AtomicBool::new(false));
        let queued_work_flag = Arc::clone(&queued_work_ran);
        let queued = tokio::spawn(async move {
            queued_pool
                .execute(queued_cancellation, move || {
                    queued_work_flag.store(true, Ordering::Release);
                })
                .await
        });
        tokio::task::yield_now().await;
        cancellation.cancel();
        assert_eq!(
            tokio::time::timeout(Duration::from_millis(100), queued)
                .await
                .expect("queued cancellation should complete promptly")
                .expect("queued task should join"),
            Err(AnalysisPoolError::Cancelled)
        );

        release_sender.send(()).expect("worker should still exist");
        assert_eq!(first.await.expect("task should join"), Ok(()));
        drop(pool);
        assert!(!queued_work_ran.load(Ordering::Acquire));
    }
}
