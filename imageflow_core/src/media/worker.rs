//! Process-shared bounded Tokio worker runtime (`§4.2`).
//!
//! Lazily initialized ONLY when `async-ffi` / `streaming-async` starts an async job.
//! Enforces explicit 2 MiB worker stack size (isolating 256 KiB IIS caller threads from
//! codec stack usage), bounded synchronous admission queue (`admission_permits`), separate
//! resident-job and compute-burst semaphores (so jobs awaiting host I/O release CPU permits),
//! and graceful shutdown tracking.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use tokio::runtime::{Builder, Runtime};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Explicit stack size for async media worker threads (2 MiB, safe for all codecs + CMS).
pub const WORKER_STACK_SIZE_BYTES: usize = 2 * 1024 * 1024;

/// Bounded process-shared async runtime for streaming media jobs.
pub struct AsyncMediaRuntime {
    runtime: Runtime,
    /// Synchronous admission ceiling bounding total queued + resident plans/notifiers.
    admission_permits: Arc<Semaphore>,
    resident_jobs: Arc<Semaphore>,
    compute_permits: Arc<Semaphore>,
    active_jobs: Arc<AtomicUsize>,
    draining: AtomicBool,
}

impl AsyncMediaRuntime {
    /// Obtain or lazily initialize the global [`AsyncMediaRuntime`].
    pub fn global() -> std::result::Result<&'static Arc<Self>, String> {
        static RUNTIME: OnceLock<std::result::Result<Arc<AsyncMediaRuntime>, String>> =
            OnceLock::new();
        let res = RUNTIME.get_or_init(|| {
            let max_compute = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .clamp(2, 16);
            let max_resident = max_compute.saturating_mul(8).clamp(16, 128);
            let max_admitted = max_resident.saturating_mul(2);
            let runtime = Builder::new_multi_thread()
                .worker_threads(max_compute.min(4))
                .max_blocking_threads(max_resident)
                .thread_stack_size(WORKER_STACK_SIZE_BYTES)
                .thread_name("imageflow-media")
                .enable_time()
                .build()
                .map_err(|e| format!("failed to start imageflow async media runtime: {e}"))?;
            Ok(Arc::new(Self {
                runtime,
                admission_permits: Arc::new(Semaphore::new(max_admitted)),
                resident_jobs: Arc::new(Semaphore::new(max_resident)),
                compute_permits: Arc::new(Semaphore::new(max_compute)),
                active_jobs: Arc::new(AtomicUsize::new(0)),
                draining: AtomicBool::new(false),
            }))
        });
        res.as_ref().map_err(Clone::clone)
    }

    /// Tokio runtime handle for spawning async coordination tasks.
    #[must_use]
    pub fn handle(&self) -> &tokio::runtime::Handle {
        self.runtime.handle()
    }

    /// Synchronously try to reserve one bounded admission slot before spawning any Tokio task.
    pub(crate) fn try_admit_job(&self) -> Option<OwnedSemaphorePermit> {
        Arc::clone(&self.admission_permits).try_acquire_owned().ok()
    }

    /// Process-wide resident-job admission semaphore (held while a blocking worker is live).
    #[must_use]
    pub(crate) fn resident_permits(&self) -> Arc<Semaphore> {
        Arc::clone(&self.resident_jobs)
    }

    /// Process-wide compute-burst semaphore (held ONLY during `session.next` CPU work,
    /// released immediately while awaiting host input or output ACK!).
    #[must_use]
    pub(crate) fn compute_permits(&self) -> Arc<Semaphore> {
        Arc::clone(&self.compute_permits)
    }

    /// Currently available compute permits (observable for diagnostics & backpressure tests).
    #[must_use]
    pub fn available_compute_permits(&self) -> usize {
        self.compute_permits.available_permits()
    }

    /// Active running/queued async job count.
    #[must_use]
    pub fn active_jobs(&self) -> usize {
        self.active_jobs.load(Ordering::Acquire)
    }

    /// Whether the runtime is draining and refusing new async starts.
    #[must_use]
    pub fn is_draining(&self) -> bool {
        self.draining.load(Ordering::Acquire)
    }

    pub(crate) fn track_job_start(&self) -> Arc<AtomicUsize> {
        self.active_jobs.fetch_add(1, Ordering::AcqRel);
        Arc::clone(&self.active_jobs)
    }
}
