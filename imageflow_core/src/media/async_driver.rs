//! Tokio-backed async streaming job driver (`§4.2` & `§5.3`).
//!
//! Bridges a sans-I/O [`MediaSession`] onto an admitted bounded Tokio worker thread while
//! exposing a non-blocking mailbox to C / .NET / Rust hosts:
//! - Enforces a bounded synchronous admission queue BEFORE spawning any Tokio task,
//! - Starts deadline timer BEFORE resident/compute admission,
//! - Separates resident-job admission from compute-burst permits so a job awaiting host
//!   input or output ACK releases its CPU permit immediately,
//! - Dispatches host `notify(flags)` on a dedicated Tokio task with ZERO mutex held across
//!   host callback execution, keeping paused activation EXCLUSIVELY under caller authority
//!   (never prematurely activated by fast worker completion or `close()`!),
//! - Guards every in-flight `ReplyCompletion` with an unwind-safe `Drop` guard so even a
//!   worker panic mid-`session.reply(...)` wakes the caller immediately,
//! - Invokes `release_user()` EXACTLY ONCE strictly AFTER both the worker AND the notifier
//!   task have exited on BOTH normal and exceptional unwind paths.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use tokio::sync::{Notify, OwnedSemaphorePermit};

use crate::media::bindings::{IoEvent, IoInput, IoTicket, JobStop, Reply, StopCause, WorkQuota};
use crate::media::progress::{ProgressMode, ProgressObserver};
use crate::media::session::{CopyPendingError, MediaSession, SessionError};
use crate::media::worker::AsyncMediaRuntime;
use crate::ErrorCategory;

/// Notification flag: a new event or terminal state is visible via `poll_event()`.
pub const NOTIFY_EVENT: u32 = 1 << 0;
/// Notification flag: `STOP` has latched on this job.
pub const NOTIFY_STOP: u32 = 1 << 1;
/// Notification flag: paced/coalesced progress update is available via `progress_observer()`.
pub const NOTIFY_PROGRESS: u32 = 1 << 2;

/// Opt-in progress tracking mode for [`AsyncJobHandle`].
///
/// Default is [`AsyncProgressMode::Disabled`] (zero `PulseTree` allocation, zero progress
/// callbacks, zero wake overhead — pure `how_far::StopOnly`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AsyncProgressMode {
    /// No progress tracking (zero cost; stop token only).
    #[default]
    Disabled,
    /// Lock-free atomic `PulseTree` + `Observer` only (zero progress notifications/callbacks;
    /// caller polls `progress_observer()` at its own cadence).
    PullOnly,
    /// Atomic `FnPulse` + `Observer` emitting paced, coalesced [`NOTIFY_PROGRESS`] wakes at most
    /// once per `min_interval_ms` (plus on stage and terminal transitions).
    PacedNotify { min_interval_ms: u32 },
}

/// Owned host notification hooks transferred atomically on successful async start.
pub struct AsyncJobHooks {
    /// Invoked serially on a dedicated Tokio task (never under internal locks and never
    /// synchronously inside `start` or `cancel`) whenever an event becomes visible or STOP latches.
    pub notify: Box<dyn Fn(u32) + Send + Sync>,
    /// Invoked EXACTLY ONCE after the worker exits and all `notify` calls have returned.
    pub release: Box<dyn FnOnce() + Send + Sync>,
}

/// Errors returned synchronously when starting an async job.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AsyncStartError {
    /// Runtime failed to initialize or is draining.
    RuntimeUnavailable(String),
    /// Bounded pending+active async admission queue is full (`IFX_RESOURCE_LIMIT`).
    QueueFull,
    /// Session was already stepped manually and cannot transition to async mode.
    AlreadyStepped,
}

struct PendingOutputSnapshot {
    ticket: IoTicket,
    bytes: Vec<u8>,
}

struct ReplyCompletion {
    result: Mutex<Option<std::result::Result<(), SessionError>>>,
    cv: Condvar,
}

impl ReplyCompletion {
    fn new() -> Self {
        Self { result: Mutex::new(None), cv: Condvar::new() }
    }

    fn complete(&self, res: std::result::Result<(), SessionError>) {
        let mut g = match self.result.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        if g.is_none() {
            *g = Some(res);
            self.cv.notify_all();
        }
    }

    fn wait(&self) -> std::result::Result<(), SessionError> {
        let mut g = match self.result.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        loop {
            if let Some(res) = g.take() {
                return res;
            }
            g = match self.cv.wait(g) {
                Ok(guard) => guard,
                Err(p) => p.into_inner(),
            };
        }
    }
}

/// Unwind-safe guard around an in-flight `ReplyCompletion` taken from `MailboxState` so even
/// if `session.reply(...)` unwinds, `reply_in_flight` is cleared (strictly AFTER the reply's
/// `OwnedReply` buffer drops!) and the waiting caller is ALWAYS woken immediately!
struct InFlightReplyGuard<'a> {
    state: &'a Mutex<MailboxState>,
    completion: Option<Arc<ReplyCompletion>>,
}

impl<'a> InFlightReplyGuard<'a> {
    fn new(state: &'a Mutex<MailboxState>, completion: Arc<ReplyCompletion>) -> Self {
        Self { state, completion: Some(completion) }
    }

    fn complete(mut self, res: std::result::Result<(), SessionError>) {
        if let Some(c) = self.completion.take() {
            c.complete(res);
        }
    }
}

impl Drop for InFlightReplyGuard<'_> {
    fn drop(&mut self) {
        if let Some(c) = self.completion.take() {
            state_lock(self.state).reply_in_flight = false;
            c.complete(Err(SessionError::Internal(
                "async worker unwound while applying reply".to_string(),
            )));
        }
    }
}

struct ReplyEnvelope {
    ticket: IoTicket,
    kind: OwnedReply,
    completion: Arc<ReplyCompletion>,
}

enum OwnedReply {
    Bytes(Vec<u8>),
    End { total_len: Option<u64> },
    Length(u64),
    UnknownLength,
    Ack { bytes: u64 },
    Aborted,
    /// Inline fixed-size diagnostic buffer (zero heap allocation for IoError diagnostics).
    IoError { accepted_prefix: Option<u64>, diagnostic: [u8; 512], diagnostic_len: u16 },
}

struct MailboxState {
    visible_event: Option<IoEvent>,
    pending_output: Option<PendingOutputSnapshot>,
    terminal_json: Option<Vec<u8>>,
    incoming_reply: Option<ReplyEnvelope>,
    /// True from the instant the worker takes `incoming_reply` until the reply's owned buffer
    /// has been applied AND dropped, preventing concurrent Rust callers from allocating a second
    /// transport buffer against the single reserved transport slot!
    reply_in_flight: bool,
    worker_done: bool,
}

/// Lock-free notification coalescer owned by ONE dedicated Tokio notification task.
///
/// `activated` is controlled EXCLUSIVELY by `NotificationActivator` (never set by `close()`!)
/// so a fast-finishing or pre-cancelled job can NEVER invoke host `notify` before the caller's
/// `start_async` scope drops its state lock and busy guard.
struct NotifierDispatch {
    pending_flags: AtomicU32,
    closed: AtomicBool,
    activated: AtomicBool,
    wake: Notify,
}

impl NotifierDispatch {
    fn new(start_activated: bool) -> Self {
        Self {
            pending_flags: AtomicU32::new(0),
            closed: AtomicBool::new(false),
            activated: AtomicBool::new(start_activated),
            wake: Notify::new(),
        }
    }

    fn activate(&self) {
        self.activated.store(true, Ordering::Release);
        self.wake.notify_one();
    }

    fn signal(&self, flags: u32) {
        if flags == 0 || self.closed.load(Ordering::Acquire) {
            return;
        }
        self.pending_flags.fetch_or(flags, Ordering::AcqRel);
        self.wake.notify_one();
    }

    /// Signal the notifier task to drain queued flags and exit once `activated` becomes true.
    /// Does NOT set `activated = true` — activation remains strictly under caller authority!
    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.wake.notify_one();
    }
}

struct ReleaseOnce {
    release_fn: Mutex<Option<Box<dyn FnOnce() + Send + Sync>>>,
}

impl ReleaseOnce {
    fn new(f: Box<dyn FnOnce() + Send + Sync>) -> Self {
        Self { release_fn: Mutex::new(Some(f)) }
    }

    fn fire_once(&self) {
        let taken = match self.release_fn.lock() {
            Ok(mut g) => g.take(),
            Err(p) => p.into_inner().take(),
        };
        if let Some(f) = taken {
            let _ = catch_unwind(AssertUnwindSafe(f));
        }
    }
}

/// Unwind-safe coordinator finalizer guaranteeing that on BOTH normal AND exceptional exit:
/// - pending reply waiters wake immediately,
/// - `notifier_handle` exits BEFORE `release_once.fire_once()` runs (zero notify/release race!),
/// - `active_counter` and `admission_permit` are released strictly once.
struct JobFinalizer {
    rt_handle: tokio::runtime::Handle,
    state: Arc<Mutex<MailboxState>>,
    notifier: Arc<NotifierDispatch>,
    notifier_handle: Option<tokio::task::JoinHandle<()>>,
    release: Arc<ReleaseOnce>,
    active_counter: Arc<AtomicUsize>,
    admission_permit: Option<OwnedSemaphorePermit>,
    finished_normally: bool,
}

impl JobFinalizer {
    async fn finish_quiesced(mut self) {
        self.finished_normally = true;
        self.wake_pending_reply();
        self.notifier.close();
        if let Some(nh) = self.notifier_handle.take() {
            let _ = nh.await;
        }
        self.release.fire_once();
        self.active_counter.fetch_sub(1, Ordering::AcqRel);
        drop(self.admission_permit.take());
    }

    fn wake_pending_reply(&self) {
        let pending = {
            let mut g = state_lock(&self.state);
            g.worker_done = true;
            g.incoming_reply.take()
        };
        if let Some(env) = pending {
            env.completion.complete(Err(SessionError::Closed));
        }
    }
}

impl Drop for JobFinalizer {
    fn drop(&mut self) {
        if self.finished_normally {
            return;
        }
        // Exceptional coordinator unwind path: still guarantee `release_user` fires strictly
        // AFTER `notifier_handle` completes!
        self.wake_pending_reply();
        self.notifier.close();
        let nh = self.notifier_handle.take();
        let release = Arc::clone(&self.release);
        let active = Arc::clone(&self.active_counter);
        let permit = self.admission_permit.take();
        if let Some(nh) = nh {
            self.rt_handle.spawn(async move {
                let _ = nh.await;
                release.fire_once();
                active.fetch_sub(1, Ordering::AcqRel);
                drop(permit);
            });
        } else {
            release.fire_once();
            active.fetch_sub(1, Ordering::AcqRel);
            drop(permit);
        }
    }
}

/// One-shot activator returned by [`AsyncJobHandle::start_paused`] so FFI wrappers can
/// unlock their outer handle mutex & busy guard BEFORE any host `notify` callback can fire.
pub struct NotificationActivator {
    notifier: Arc<NotifierDispatch>,
}

impl NotificationActivator {
    /// Enable host notification delivery (idempotent; flushes any queued pre-activation flags).
    pub fn activate(self) {
        self.notifier.activate();
    }
}

impl Drop for NotificationActivator {
    fn drop(&mut self) {
        self.notifier.activate();
    }
}

/// Non-blocking host handle to a running Tokio-backed streaming media job.
///
/// Dropping [`AsyncJobHandle`] (or calling [`AsyncJobHandle::release_caller_handle`])
/// immediately requests abandonment (`JobStop::abandon`), waking any waiting worker
/// (even during an in-flight `Commit`) so the job always reaches quiescence and fires
/// `release` exactly once.
pub struct AsyncJobHandle {
    state: Arc<Mutex<MailboxState>>,
    stop: JobStop,
    reply_wake: Arc<Notify>,
    notifier: Arc<NotifierDispatch>,
    progress: Option<ProgressObserver>,
}

impl Drop for AsyncJobHandle {
    fn drop(&mut self) {
        let already_done = state_lock(&self.state).worker_done;
        if !already_done {
            let _ = self.stop.abandon();
            self.reply_wake.notify_waiters();
        }
    }
}

impl AsyncJobHandle {
    /// Start `session` on the bounded [`AsyncMediaRuntime`] with optional `deadline` (zero-cost
    /// default: progress disabled unless caller opted in on `session`).
    pub fn start(
        session: MediaSession,
        hooks: AsyncJobHooks,
        deadline: Option<Duration>,
    ) -> std::result::Result<Self, (AsyncStartError, MediaSession, AsyncJobHooks)> {
        Self::start_internal(session, hooks, deadline, true, AsyncProgressMode::Disabled)
    }

    /// Start `session` with host notification delivery initially paused until the caller
    /// releases its own outer handle lock and calls [`NotificationActivator::activate`].
    pub fn start_paused(
        session: MediaSession,
        hooks: AsyncJobHooks,
        deadline: Option<Duration>,
    ) -> std::result::Result<(Self, NotificationActivator), (AsyncStartError, MediaSession, AsyncJobHooks)>
    {
        Self::start_paused_with_progress(session, hooks, deadline, AsyncProgressMode::Disabled)
    }

    /// Start `session` paused with an explicit opt-in [`AsyncProgressMode`].
    pub fn start_paused_with_progress(
        session: MediaSession,
        hooks: AsyncJobHooks,
        deadline: Option<Duration>,
        progress_mode: AsyncProgressMode,
    ) -> std::result::Result<(Self, NotificationActivator), (AsyncStartError, MediaSession, AsyncJobHooks)>
    {
        let handle = Self::start_internal(session, hooks, deadline, false, progress_mode)?;
        let activator = NotificationActivator { notifier: Arc::clone(&handle.notifier) };
        Ok((handle, activator))
    }

    /// Return the lock-free [`ProgressObserver`] if progress tracking was opted in (`None` by default).
    #[must_use]
    pub fn progress_observer(&self) -> Option<&ProgressObserver> {
        self.progress.as_ref()
    }

    fn start_internal(
        mut session: MediaSession,
        hooks: AsyncJobHooks,
        deadline: Option<Duration>,
        start_activated: bool,
        progress_mode: AsyncProgressMode,
    ) -> std::result::Result<Self, (AsyncStartError, MediaSession, AsyncJobHooks)> {
        let rt = match AsyncMediaRuntime::global() {
            Ok(r) => r,
            Err(e) => return Err((AsyncStartError::RuntimeUnavailable(e), session, hooks)),
        };
        if rt.is_draining() {
            return Err((
                AsyncStartError::RuntimeUnavailable("async media runtime is draining".into()),
                session,
                hooks,
            ));
        }

        // Synchronously enforce bounded admission ceiling BEFORE spawning any Tokio task!
        let Some(admission_permit) = rt.try_admit_job() else {
            return Err((AsyncStartError::QueueFull, session, hooks));
        };

        if let Some(cap) = session.max_memory_bytes()
            && cap < 2
        {
            drop(admission_permit);
            return Err((
                AsyncStartError::RuntimeUnavailable(
                    "max_memory_bytes too small for bounded async transport buffer".into(),
                ),
                session,
                hooks,
            ));
        }

        // Unify caller deadline and session deadline onto ONE single absolute monotonic `Instant`!
        let caller_deadline_at =
            deadline.and_then(|d| std::time::Instant::now().checked_add(d));
        let effective_deadline_at = match (caller_deadline_at, session.deadline_at()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) | (None, Some(a)) => Some(a),
            (None, None) => None,
        };
        session.set_deadline_at(effective_deadline_at);
        let notifier = Arc::new(NotifierDispatch::new(start_activated));

        // Enable opt-in progress tracking strictly BEFORE converting into a movable plan.
        let progress_observer = match progress_mode {
            AsyncProgressMode::Disabled => session.progress_observer(),
            AsyncProgressMode::PullOnly => match session.enable_progress(ProgressMode::PullOnly) {
                Ok(obs) => Some(obs),
                Err(_) => {
                    drop(admission_permit);
                    return Err((AsyncStartError::AlreadyStepped, session, hooks));
                }
            },
            AsyncProgressMode::PacedNotify { min_interval_ms } => {
                let p_notif = Arc::clone(&notifier);
                let mode = ProgressMode::PacedNotify {
                    on_notify: Arc::new(move || p_notif.signal(NOTIFY_PROGRESS)),
                    min_interval: Duration::from_millis(u64::from(min_interval_ms.max(1))),
                };
                match session.enable_progress(mode) {
                    Ok(obs) => Some(obs),
                    Err(_) => {
                        drop(admission_permit);
                        return Err((AsyncStartError::AlreadyStepped, session, hooks));
                    }
                }
            }
        };

        let stop = session.stop_handle();
        let mut plan = match session.try_into_plan() {
            Ok(p) => p,
            Err(returned) => {
                drop(admission_permit);
                return Err((AsyncStartError::AlreadyStepped, returned, hooks));
            }
        };
        // Reserve one mutually-exclusive bounded transport copy (`<= transfer_max <= cap / 8`)
        // permanently inside `MediaSession::live_mem_bytes` so async `PendingOutputSnapshot` and
        // `OwnedReply::Bytes` NEVER overlap past `max_memory_bytes`!
        plan.reserve_async_transport_budget();

        let release_once = Arc::new(ReleaseOnce::new(hooks.release));
        let notify_fn = hooks.notify;
        let state = Arc::new(Mutex::new(MailboxState {
            visible_event: None,
            pending_output: None,
            terminal_json: None,
            incoming_reply: None,
            reply_in_flight: false,
            worker_done: false,
        }));
        let reply_wake = Arc::new(Notify::new());

        // Wire lock-free stop hook: NEVER invokes host callback synchronously!
        {
            let w_wake = Arc::clone(&reply_wake);
            let w_notif = Arc::clone(&notifier);
            stop.on_stop(move |_cause| {
                w_wake.notify_waiters();
                w_notif.signal(NOTIFY_STOP);
            });
        }

        let active_counter = rt.track_job_start();
        let resident_permits = rt.resident_permits();
        let compute_permits = rt.compute_permits();
        let w_state = Arc::clone(&state);
        let w_stop = stop.clone();
        let w_reply_wake = Arc::clone(&reply_wake);
        let w_notif = Arc::clone(&notifier);
        let w_release = Arc::clone(&release_once);
        let coord_rt_handle = rt.handle().clone();

        // Spawn dedicated serialized notifier task (zero mutex held across notify_fn!).
        // Stays parked strictly until `activated` becomes true (set ONLY by caller!).
        let n_disp = Arc::clone(&notifier);
        let notifier_handle = rt.handle().spawn(async move {
            loop {
                let notified = n_disp.wake.notified();
                if n_disp.activated.load(Ordering::Acquire) {
                    break;
                }
                notified.await;
            }
            loop {
                let flags = n_disp.pending_flags.swap(0, Ordering::AcqRel);
                if flags != 0 {
                    let _ = catch_unwind(AssertUnwindSafe(|| {
                        (notify_fn)(flags);
                    }));
                    continue;
                }
                if n_disp.closed.load(Ordering::Acquire) {
                    let tail = n_disp.pending_flags.swap(0, Ordering::AcqRel);
                    if tail != 0 {
                        let _ = catch_unwind(AssertUnwindSafe(|| {
                            (notify_fn)(tail);
                        }));
                    }
                    break;
                }
                let notified = n_disp.wake.notified();
                if n_disp.pending_flags.load(Ordering::Acquire) != 0
                    || n_disp.closed.load(Ordering::Acquire)
                {
                    continue;
                }
                notified.await;
            }
        });

        // Spawn coordinator task: arms deadline BEFORE resident/compute admission!
        rt.handle().spawn(async move {
            let finalizer = JobFinalizer {
                rt_handle: coord_rt_handle,
                state: Arc::clone(&w_state),
                notifier: Arc::clone(&w_notif),
                notifier_handle: Some(notifier_handle),
                release: Arc::clone(&w_release),
                active_counter,
                admission_permit: Some(admission_permit),
                finished_normally: false,
            };

            let deadline_handle = effective_deadline_at.map(|at| {
                let d_stop = w_stop.clone();
                tokio::spawn(async move {
                    tokio::time::sleep_until(tokio::time::Instant::from_std(at)).await;
                    let _ = d_stop.trip(StopCause::Deadline);
                })
            });

            // Acquire resident-job permit while racing against stop latch.
            let resident_permit =
                acquire_permit_or_stop(&resident_permits, &w_stop, &w_reply_wake).await;

            // Construct and drive thread-affine session on one 2 MiB-stack worker thread.
            let b_state = Arc::clone(&w_state);
            let b_notif = Arc::clone(&w_notif);
            let b_stop = w_stop.clone();
            let b_reply_wake = Arc::clone(&w_reply_wake);
            let b_compute = Arc::clone(&compute_permits);
            let final_session_res = tokio::task::spawn_blocking(move || {
                let _resident_guard = resident_permit;
                let session = MediaSession::from_plan(plan);
                drive_session_worker(
                    session,
                    &b_state,
                    &b_stop,
                    &b_reply_wake,
                    &b_notif,
                    &b_compute,
                );
            })
            .await;

            if let Some(dh) = deadline_handle {
                dh.abort();
            }

            if final_session_res.is_err() {
                let pending = {
                    let mut g = state_lock(&w_state);
                    if !g.worker_done {
                        g.visible_event = Some(IoEvent::Failed {
                            category: ErrorCategory::InternalError,
                            cause: StopCause::None,
                        });
                        g.pending_output = None;
                        g.terminal_json = Some(
                            br#"{"version":2,"success":false,"category":18,"stop_reason":0,"message":"async worker panicked","publications":[]}"#
                                .to_vec(),
                        );
                        g.worker_done = true;
                    }
                    g.incoming_reply.take()
                };
                if let Some(env) = pending {
                    env.completion.complete(Err(SessionError::Internal(
                        "async worker panicked".to_string(),
                    )));
                }
                w_notif.signal(NOTIFY_EVENT);
            }

            // Quiescence barrier: close notifier, await notifier_task exit, then fire release_user!
            finalizer.finish_quiesced().await;
        });

        Ok(Self { state, stop, reply_wake, notifier, progress: progress_observer })
    }

    /// Non-blocking poll for the currently published event (`None` => `IFX_WOULD_BLOCK`).
    /// Never runs codec compute on the caller thread.
    #[must_use]
    pub fn poll_event(&self) -> Option<IoEvent> {
        let g = state_lock(&self.state);
        g.visible_event
    }

    /// Copy pending output/patch bytes for `ticket` into `dst` (non-blocking, idempotent).
    pub fn copy_pending(
        &self,
        ticket: IoTicket,
        dst: &mut [u8],
    ) -> std::result::Result<usize, CopyPendingError> {
        let g = state_lock(&self.state);
        if matches!(g.visible_event, Some(IoEvent::Complete | IoEvent::Failed { .. })) {
            return Err(CopyPendingError::Closed);
        }
        let Some(ref snap) = g.pending_output else {
            return Err(CopyPendingError::StaleTicket);
        };
        if snap.ticket != ticket {
            return Err(CopyPendingError::StaleTicket);
        }
        if dst.len() < snap.bytes.len() {
            return Err(CopyPendingError::BufferTooSmall(snap.bytes.len()));
        }
        dst[..snap.bytes.len()].copy_from_slice(&snap.bytes);
        Ok(snap.bytes.len())
    }

    /// Submit a host reply for `ticket` and wait briefly for worker state-machine validation.
    ///
    /// - Validates terminal state, ticket match, and byte-length credit **BEFORE** copying any
    ///   input bytes (so stale/oversized replies allocate zero bytes).
    /// - Never runs codec compute before returning: the worker validates the reply immediately
    ///   and signals this reply's dedicated completion latch before acquiring a compute permit
    ///   for the next `session.next` burst.
    pub fn reply(&self, ticket: IoTicket, reply: Reply<'_>) -> std::result::Result<(), SessionError> {
        let completion = Arc::new(ReplyCompletion::new());
        {
            let mut g = state_lock(&self.state);
            if g.worker_done
                || matches!(g.visible_event, Some(IoEvent::Complete | IoEvent::Failed { .. }))
            {
                return Err(SessionError::Closed);
            }
            let active_ev = g.visible_event.ok_or(SessionError::StaleTicket)?;
            let expected_ticket = match active_ev {
                IoEvent::NeedInput { ticket: t, .. }
                | IoEvent::Output { ticket: t, .. }
                | IoEvent::Patch { ticket: t, .. }
                | IoEvent::Flush { ticket: t, .. }
                | IoEvent::Commit { ticket: t } => t,
                _ => return Err(SessionError::StaleTicket),
            };
            if expected_ticket != ticket {
                return Err(SessionError::StaleTicket);
            }
            if g.incoming_reply.is_some() || g.reply_in_flight {
                return Err(SessionError::InvalidArgument("reply already pending or in flight".into()));
            }

            // Pre-validate byte length against active transfer credit BEFORE cloning bytes!
            if let Reply::Bytes(b) = reply {
                let max_allowed = match active_ev {
                    IoEvent::NeedInput {
                        request: IoInput::AtLeast { max_transfer, .. },
                        ..
                    } => max_transfer as usize,
                    IoEvent::NeedInput { request: IoInput::Range { len, .. }, .. } => len as usize,
                    _ => {
                        return Err(SessionError::InvalidArgument(
                            "Reply::Bytes is only valid for NeedInput AtLeast/Range".into(),
                        ));
                    }
                };
                if b.is_empty() || b.len() > max_allowed {
                    return Err(SessionError::InvalidArgument(format!(
                        "Bytes reply length {} out of range 1..={}",
                        b.len(),
                        max_allowed
                    )));
                }
            }

            let owned = match reply {
                Reply::Bytes(b) => {
                    // Exact-capacity allocation bounded by active credit (<= reserved transport_cap).
                    let mut owned_bytes = Vec::new();
                    owned_bytes.try_reserve_exact(b.len()).map_err(|_| {
                        SessionError::ResourceLimit("async reply bytes reserve failed".into())
                    })?;
                    owned_bytes.extend_from_slice(b);
                    OwnedReply::Bytes(owned_bytes)
                }
                Reply::End { total_len } => OwnedReply::End { total_len },
                Reply::Length(l) => OwnedReply::Length(l),
                Reply::UnknownLength => OwnedReply::UnknownLength,
                Reply::Ack { bytes } => OwnedReply::Ack { bytes },
                Reply::Aborted => OwnedReply::Aborted,
                Reply::IoError { accepted_prefix, diagnostic } => {
                    let n = diagnostic.len().min(512);
                    let mut buf = [0u8; 512];
                    buf[..n].copy_from_slice(&diagnostic[..n]);
                    OwnedReply::IoError {
                        accepted_prefix,
                        diagnostic: buf,
                        diagnostic_len: n as u16,
                    }
                }
            };

            g.incoming_reply =
                Some(ReplyEnvelope { ticket, kind: owned, completion: Arc::clone(&completion) });
        }

        // Wake worker outside MailboxState mutex, then wait on this reply's dedicated completion!
        self.reply_wake.notify_waiters();
        completion.wait()
    }

    /// Copy finalized terminal JSON into an owned vector if terminal (`None` => `WOULD_BLOCK`).
    #[must_use]
    pub fn result_json_cloned(&self) -> Option<Vec<u8>> {
        let g = state_lock(&self.state);
        g.terminal_json.clone()
    }

    /// Thread-safe instant cancellation (may race any driver call and may be called safely
    /// even from inside the `notify` callback!).
    pub fn cancel(&self) {
        let _ = self.stop.trip(StopCause::CallerCancel);
    }

    /// Explicitly release the caller handle (non-blocking: abandons any unfinished job —
    /// including an in-flight `Commit` — so the worker always exits and fires `release_user`
    /// once on quiescence).
    pub fn release_caller_handle(self) {
        let _ = self.stop.abandon();
        self.reply_wake.notify_waiters();
    }
}

async fn acquire_permit_or_stop(
    sem: &Arc<tokio::sync::Semaphore>,
    stop: &JobStop,
    wake: &Notify,
) -> Option<tokio::sync::OwnedSemaphorePermit> {
    use std::future::Future as _;
    let mut acq = Box::pin(Arc::clone(sem).acquire_owned());
    let mut stopped = Box::pin(wait_until_stopped(stop, wake));
    std::future::poll_fn(|cx| {
        if stop.is_stopped() {
            return std::task::Poll::Ready(None);
        }
        if let std::task::Poll::Ready(res) = acq.as_mut().poll(cx) {
            return std::task::Poll::Ready(res.ok());
        }
        if stopped.as_mut().poll(cx).is_ready() {
            return std::task::Poll::Ready(None);
        }
        std::task::Poll::Pending
    })
    .await
}

async fn wait_until_stopped(stop: &JobStop, wake: &Notify) {
    loop {
        let notified = wake.notified();
        if stop.is_stopped() {
            return;
        }
        notified.await;
    }
}

fn state_lock(m: &Mutex<MailboxState>) -> std::sync::MutexGuard<'_, MailboxState> {
    match m.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    }
}

fn drive_session_worker(
    mut session: MediaSession,
    state: &Mutex<MailboxState>,
    stop: &JobStop,
    reply_wake: &Notify,
    notifier: &NotifierDispatch,
    compute_permits: &Arc<tokio::sync::Semaphore>,
) {
    let rt_handle = tokio::runtime::Handle::current();

    loop {
        // Acquire compute permit ONLY for this bounded `session.next` burst — immediately
        // dropped before waiting for host input or output ACK!
        let ev = {
            let _compute_permit =
                rt_handle.block_on(acquire_permit_or_stop(compute_permits, stop, reply_wake));
            match session.next(WorkQuota::DEFAULT) {
                Ok(ev) => ev,
                Err(e) => {
                    let term_ev = session.fail_unpublished_ticket(
                        ErrorCategory::InternalError,
                        format!("{e:?}"),
                    );
                    let json = session.result_json().map(|b| b.to_vec());
                    let pending = {
                        let mut g = state_lock(state);
                        g.visible_event = Some(term_ev);
                        g.pending_output = None;
                        g.terminal_json = json;
                        g.worker_done = true;
                        g.incoming_reply.take()
                    };
                    if let Some(env) = pending {
                        env.completion.complete(Err(SessionError::Closed));
                    }
                    notifier.signal(NOTIFY_EVENT);
                    return;
                }
            }
        };

        match ev {
            IoEvent::Yield => continue,
            IoEvent::Complete | IoEvent::Failed { .. } => {
                let json = session.result_json().map(|b| b.to_vec());
                let pending = {
                    let mut g = state_lock(state);
                    g.visible_event = Some(ev);
                    g.pending_output = None;
                    g.terminal_json = json;
                    g.worker_done = true;
                    g.incoming_reply.take()
                };
                if let Some(env) = pending {
                    env.completion.complete(Err(SessionError::Closed));
                }
                notifier.signal(NOTIFY_EVENT);
                return;
            }
            IoEvent::Output { ticket, len, .. } | IoEvent::Patch { ticket, len, .. } => {
                // Exact-capacity snapshot bounded strictly by `transfer_max <= reserved transport_cap`
                // and mutually exclusive with `OwnedReply::Bytes`. Any allocation or copy failure
                // terminates through `session.fail_unpublished_ticket` BEFORE publishing the event
                // so previously acknowledged per-sink publication receipts are preserved verbatim
                // and the unpublished ticket has zero external effect!
                let snap_res = capture_output_snapshot(&session, ticket, len as usize);
                let buf = match snap_res {
                    Ok(buf) => buf,
                    Err((cat, msg)) => {
                        let term_ev = session.fail_unpublished_ticket(cat, msg.to_string());
                        let json = session.result_json().map(|b| b.to_vec());
                        let pending = {
                            let mut g = state_lock(state);
                            g.visible_event = Some(term_ev);
                            g.pending_output = None;
                            g.terminal_json = json;
                            g.worker_done = true;
                            g.incoming_reply.take()
                        };
                        if let Some(env) = pending {
                            env.completion.complete(Err(SessionError::Closed));
                        }
                        notifier.signal(NOTIFY_EVENT);
                        return;
                    }
                };
                {
                    let mut g = state_lock(state);
                    g.visible_event = Some(ev);
                    g.pending_output = Some(PendingOutputSnapshot { ticket, bytes: buf });
                }
                notifier.signal(NOTIFY_EVENT);
            }
            IoEvent::NeedInput { .. } | IoEvent::Flush { .. } | IoEvent::Commit { .. } => {
                {
                    let mut g = state_lock(state);
                    g.visible_event = Some(ev);
                    g.pending_output = None;
                }
                notifier.signal(NOTIFY_EVENT);
            }
        }

        // Wait for host reply OR stop (holding ZERO compute permits!).
        let is_commit = matches!(ev, IoEvent::Commit { .. });
        loop {
            let reply_opt = {
                let mut g = state_lock(state);
                if let Some(env) = g.incoming_reply.take() {
                    g.reply_in_flight = true;
                    Some(env)
                } else {
                    None
                }
            };
            if let Some(env) = reply_opt {
                // Declare `guard` BEFORE `kind` so on unwind Rust drops `kind` (freeing any
                // `OwnedReply::Bytes` buffer) STRICTLY BEFORE `guard::drop` clears `reply_in_flight`!
                let guard = InFlightReplyGuard::new(state, env.completion);
                let ticket = env.ticket;
                let kind = env.kind;
                let res = match &kind {
                    OwnedReply::Bytes(b) => session.reply(ticket, Reply::Bytes(b)),
                    OwnedReply::End { total_len } => {
                        session.reply(ticket, Reply::End { total_len: *total_len })
                    }
                    OwnedReply::Length(l) => session.reply(ticket, Reply::Length(*l)),
                    OwnedReply::UnknownLength => session.reply(ticket, Reply::UnknownLength),
                    OwnedReply::Ack { bytes } => {
                        session.reply(ticket, Reply::Ack { bytes: *bytes })
                    }
                    OwnedReply::Aborted => session.reply(ticket, Reply::Aborted),
                    OwnedReply::IoError { accepted_prefix, diagnostic, diagnostic_len } => {
                        session.reply(
                            ticket,
                            Reply::IoError {
                                accepted_prefix: *accepted_prefix,
                                diagnostic: &diagnostic[..usize::from(*diagnostic_len)],
                            },
                        )
                    }
                };
                // Explicitly free any owned reply buffer BEFORE releasing `reply_in_flight`!
                drop(kind);
                match res {
                    Ok(()) => {
                        {
                            let mut g = state_lock(state);
                            g.visible_event = None;
                            g.pending_output = None;
                            g.reply_in_flight = false;
                        }
                        guard.complete(Ok(()));
                        break;
                    }
                    Err(e) => {
                        // If the session transitioned to a terminal state during `reply`
                        // (e.g., cancellation/deadline raced `Reply::IoError` and returned
                        // `SessionError::Closed`), publish the terminal event and exact
                        // `StreamErrorReportV2` receipt BEFORE waking the caller so `poll_event`
                        // can NEVER observe the stale pre-failure non-terminal event!
                        if let Some(term_ev) = session.current_terminal_event_if_done() {
                            let json = session.result_json().map(|b| b.to_vec());
                            let extra_pending = {
                                let mut g = state_lock(state);
                                g.visible_event = Some(term_ev);
                                g.pending_output = None;
                                g.terminal_json = json;
                                g.worker_done = true;
                                g.reply_in_flight = false;
                                g.incoming_reply.take()
                            };
                            guard.complete(Err(e));
                            if let Some(extra) = extra_pending {
                                extra.completion.complete(Err(SessionError::Closed));
                            }
                            notifier.signal(NOTIFY_EVENT);
                            return;
                        }
                        state_lock(state).reply_in_flight = false;
                        guard.complete(Err(e));
                    }
                }
            }

            if is_commit {
                if stop.is_abandoned() {
                    session.abandon_commit();
                    break;
                }
            } else if stop.is_stopped() {
                break;
            }

            rt_handle.block_on(async {
                let fut = reply_wake.notified();
                let has_reply = state_lock(state).incoming_reply.is_some();
                let should_wake = has_reply
                    || if is_commit { stop.is_abandoned() } else { stop.is_stopped() };
                if should_wake {
                    return;
                }
                fut.await;
            });
        }
    }
}

#[cfg(test)]
static TEST_FAIL_SNAPSHOT_REMAINING: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

fn capture_output_snapshot(
    session: &MediaSession,
    ticket: IoTicket,
    len: usize,
) -> std::result::Result<Vec<u8>, (ErrorCategory, &'static str)> {
    #[cfg(test)]
    {
        // Fault-injection hook used strictly by unit tests to trigger deterministic OOM
        // on the Nth snapshot attempt (e.g. after 1 successful incremental ACK).
        loop {
            let cur = TEST_FAIL_SNAPSHOT_REMAINING.load(Ordering::Acquire);
            if cur == 0 {
                break;
            }
            if TEST_FAIL_SNAPSHOT_REMAINING
                .compare_exchange(cur, cur - 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                if cur == 1 {
                    return Err((
                        ErrorCategory::OutOfMemory,
                        "injected async output snapshot reserve failure",
                    ));
                }
                break;
            }
        }
    }
    let mut buf = Vec::new();
    if buf.try_reserve_exact(len).is_err() {
        return Err((
            ErrorCategory::OutOfMemory,
            "out of memory allocating async output snapshot",
        ));
    }
    buf.resize(len, 0);
    match session.copy_pending(ticket, &mut buf) {
        Ok(n) if n == len => Ok(buf),
        _ => Err((
            ErrorCategory::InternalError,
            "failed to copy pending output bytes for async snapshot",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::{BindingSpec, OperationKind, Visibility, shared_modern_engine};
    use std::sync::atomic::AtomicUsize;
    use std::time::{Duration, Instant};

    /// Fault-injection witness (R6 #1): after acknowledging the first 16-byte credit of an
    /// incremental output, simulate a snapshot reserve failure on the second Output chunk
    /// BEFORE publishing it. Asserts terminal `Failed(OutOfMemory)` preserves exact
    /// `accepted_bytes == "16"`, `exact_unknown_tail == false`, and `committed == false`!
    #[test]
    fn snapshot_failure_after_ack_preserves_incremental_publication_receipt() {
        let engine = shared_modern_engine().unwrap();
        let best_effort = zenpipe::canonical::HostServices::new()
            .with_workspace_policy(zencodecs::engine::WorkspacePolicy::BestEffort);
        let canvas_req = br#"{
            "inputs": {},
            "outputs": {"out": 1},
            "pipeline": {
                "steps": [
                    {"canvas": {"width": 16, "height": 16, "fill": [40, 120, 200, 255]}},
                    {"encode": {"output": "out", "format": "png"}}
                ]
            }
        }"#;
        let bindings = [BindingSpec::forward_sink(1, Visibility::Incremental)];
        let mut sess = MediaSession::create(
            engine,
            best_effort,
            OperationKind::Execute,
            canvas_req,
            &bindings,
        )
        .unwrap();
        // Force multi-credit incremental output so chunk #1 (16 B) succeeds and chunk #2 faults.
        sess.set_transfer_max(16);

        // Fail on the 2nd snapshot capture (chunk #1 succeeds, chunk #2 triggers OOM before publish).
        TEST_FAIL_SNAPSHOT_REMAINING.store(2, Ordering::Release);

        let rel = Arc::new(AtomicUsize::new(0));
        let rel_cb = Arc::clone(&rel);
        let handle = AsyncJobHandle::start(
            sess,
            AsyncJobHooks {
                notify: Box::new(|_| {}),
                release: Box::new(move || {
                    rel_cb.fetch_add(1, Ordering::SeqCst);
                }),
            },
            None,
        )
        .map_err(|(e, _, _)| e)
        .unwrap();

        // 1. Wait for first 16-byte incremental Output chunk and acknowledge it!
        let dl = Instant::now() + Duration::from_secs(2);
        let (t1, len1) = loop {
            if let Some(IoEvent::Output { ticket, len, incremental, .. }) = handle.poll_event() {
                assert!(incremental);
                assert_eq!(len, 16);
                break (ticket, len);
            }
            assert!(Instant::now() < dl, "timed out waiting for first Output chunk");
            std::thread::yield_now();
        };
        let mut first_buf = vec![0u8; len1 as usize];
        assert_eq!(handle.copy_pending(t1, &mut first_buf).unwrap(), 16);
        handle.reply(t1, Reply::Ack { bytes: 16 }).unwrap();

        // 2. Second chunk fails during snapshot capture BEFORE publishing: must see Failed(OutOfMemory)
        //    (NEVER a second Output event!) with preserved receipt `accepted_bytes == "16"`!
        let dl2 = Instant::now() + Duration::from_secs(2);
        let term = loop {
            match handle.poll_event() {
                Some(ev @ IoEvent::Failed { .. }) => break ev,
                Some(IoEvent::Output { ticket, .. }) => {
                    panic!("unpublished failing second Output ticket {ticket:?} must never be exposed");
                }
                _ => {
                    assert!(Instant::now() < dl2, "timed out waiting for terminal Failed");
                    std::thread::yield_now();
                }
            }
        };
        assert_eq!(
            term,
            IoEvent::Failed {
                category: ErrorCategory::OutOfMemory,
                cause: StopCause::None,
            }
        );

        let receipt_bytes = handle
            .result_json_cloned()
            .expect("terminal StreamErrorReportV2 must be present");
        let rv: serde_json::Value = serde_json::from_slice(&receipt_bytes).unwrap();
        assert_eq!(rv["success"], false);
        assert_eq!(rv["publications"][0]["slot"], "out");
        assert_eq!(rv["publications"][0]["visibility"], "incremental");
        assert_eq!(
            rv["publications"][0]["accepted_bytes"], "16",
            "previously acknowledged incremental bytes MUST be preserved in terminal receipt"
        );
        assert_eq!(
            rv["publications"][0]["exact_unknown_tail"], false,
            "unpublished ticket must NOT mark incremental tail unknown"
        );
        assert_eq!(rv["publications"][0]["committed"], false);

        // Reset hook cleanly and verify worker quiescence.
        TEST_FAIL_SNAPSHOT_REMAINING.store(0, Ordering::Release);
        let dl3 = Instant::now() + Duration::from_secs(2);
        while rel.load(Ordering::SeqCst) == 0 && Instant::now() < dl3 {
            std::thread::yield_now();
        }
        assert_eq!(rel.load(Ordering::SeqCst), 1);
    }
}
