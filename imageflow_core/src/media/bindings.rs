//! Shared sans-I/O protocol types, `ReadAtSource` adapters, and `JobStop` controller.
//!
//! Used uniformly across Rust sync, C manual stepping (`streaming-ffi`), and Tokio/.NET
//! async (`streaming-async` / `async-ffi`) transport drivers.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use enough::{Stop, StopReason};

use crate::{ErrorCategory, ErrorKind, Result};

/// Opaque nonzero monotonically increasing I/O ticket scoped to one job.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IoTicket(NonZeroU64);

impl IoTicket {
    /// Construct from a nonzero wire value.
    #[must_use]
    pub const fn new(raw: NonZeroU64) -> Self {
        Self(raw)
    }

    /// Reconstruct from a raw `u64`, returning `None` if zero.
    #[must_use]
    pub const fn from_raw(raw: u64) -> Option<Self> {
        match NonZeroU64::new(raw) {
            Some(nz) => Some(Self(nz)),
            None => None,
        }
    }

    /// Return the nonzero wire integer value.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0.get()
    }
}

/// Streaming job operation kind (matches `IFX_OP_*_V1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum OperationKind {
    /// Full pipeline execution (`v1/execute_zen` / `v2/execute` envelope).
    Execute = 1,
    /// Bounded header/index inspection (`v2/inspect` envelope).
    Inspect = 2,
    /// Admission & cost/dependency estimation without encoding (`v2/estimate` envelope).
    Estimate = 3,
}

impl OperationKind {
    /// Decode wire operation integer (`1 = Execute`, `2 = Inspect`, `3 = Estimate`).
    #[must_use]
    pub const fn from_wire(raw: u32) -> Option<Self> {
        match raw {
            1 => Some(Self::Execute),
            2 => Some(Self::Inspect),
            3 => Some(Self::Estimate),
            _ => None,
        }
    }
}

/// Real transport access capability advertised for a bound source or sink slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BindingAccess {
    /// Forward-only sequential input stream (no backward seeking).
    ForwardSource {
        /// Whether the host can report exact total byte length up front via `NeedLen`.
        known_length: bool,
    },
    /// Random-access input stream supporting absolute byte-offset ranges.
    ReadAtSource {
        /// Whether the host can report exact total byte length up front via `NeedLen`.
        known_length: bool,
    },
    /// Sequential append-only output sink (no seek-back patching).
    ForwardSink,
    /// Seekable or write-at output sink supporting checked byte-offset patches.
    PatchSink,
}

impl BindingAccess {
    /// Whether this source advertises an up-front known length query (`NeedLen`).
    #[must_use]
    pub const fn has_known_length(self) -> bool {
        match self {
            Self::ForwardSource { known_length } | Self::ReadAtSource { known_length } => {
                known_length
            }
            Self::ForwardSink | Self::PatchSink => false,
        }
    }
}

/// Publication visibility contract of an output sink binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Visibility {
    /// Transactional staged sink: no externally visible output until authorized `Commit`.
    Staged,
    /// Externally incremental sink: bytes become visible as flushed chunks arrive.
    Incremental,
}

/// Declared transport binding for one numeric `io_id` slot on a streaming job.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BindingSpec {
    /// Numeric `io_id` referenced by the job JSON envelope's `"inputs"` / `"outputs"` maps.
    pub io_id: i32,
    /// Real transport access capability.
    pub access: BindingAccess,
    /// Publication visibility (`Some(...)` for sinks, `None` for sources).
    pub visibility: Option<Visibility>,
}

impl BindingSpec {
    /// Construct a forward-only sequential source binding.
    #[must_use]
    pub const fn forward_source(io_id: i32, known_length: bool) -> Self {
        Self { io_id, access: BindingAccess::ForwardSource { known_length }, visibility: None }
    }

    /// Construct a random-access `ReadAt` source binding.
    #[must_use]
    pub const fn read_at_source(io_id: i32, known_length: bool) -> Self {
        Self { io_id, access: BindingAccess::ReadAtSource { known_length }, visibility: None }
    }

    /// Construct a source binding from explicit `(range, known_length)` capabilities.
    ///
    /// Preserves `known_length` for both forward-only and random-access sources.
    #[must_use]
    pub const fn source(io_id: i32, range: bool, known_length: bool) -> Self {
        if range {
            Self::read_at_source(io_id, known_length)
        } else {
            Self::forward_source(io_id, known_length)
        }
    }

    /// Construct an append-only forward sink binding.
    #[must_use]
    pub const fn forward_sink(io_id: i32, visibility: Visibility) -> Self {
        Self { io_id, access: BindingAccess::ForwardSink, visibility: Some(visibility) }
    }

    /// Construct a seek-back patch-capable sink binding.
    #[must_use]
    pub const fn patch_sink(io_id: i32, visibility: Visibility) -> Self {
        Self { io_id, access: BindingAccess::PatchSink, visibility: Some(visibility) }
    }

    /// Construct a sink binding from explicit `(patch, visibility)` capabilities.
    #[must_use]
    pub const fn sink(io_id: i32, patch: bool, visibility: Visibility) -> Self {
        if patch { Self::patch_sink(io_id, visibility) } else { Self::forward_sink(io_id, visibility) }
    }

    /// Whether this binding is an input source.
    #[must_use]
    pub const fn is_source(&self) -> bool {
        matches!(
            self.access,
            BindingAccess::ForwardSource { .. } | BindingAccess::ReadAtSource { .. }
        )
    }

    /// Whether this binding is an output sink.
    #[must_use]
    pub const fn is_sink(&self) -> bool {
        matches!(self.access, BindingAccess::ForwardSink | BindingAccess::PatchSink)
    }

    /// Decode from `IfxBindingV1` wire flags (`§5.2`).
    pub fn from_wire(io_id: i32, flags: u32, reserved: u32) -> Result<Self> {
        if reserved != 0 {
            return Err(nerror!(ErrorKind::InvalidArgument, "binding reserved field must be zero"));
        }
        // Only bits 0..=3 are defined in v1.
        if (flags & !0x0F) != 0 {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "unknown binding flags bits: 0x{:X}",
                flags
            ));
        }
        let is_src = (flags & (1 << 0)) != 0;
        let is_sink = (flags & (1 << 1)) != 0;
        if is_src == is_sink {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "binding for io_id {} must set exactly one of source or sink flag",
                io_id
            ));
        }
        if is_src {
            let range = (flags & (1 << 2)) != 0;
            let known_len = (flags & (1 << 3)) != 0;
            Ok(Self::source(io_id, range, known_len))
        } else {
            let patch = (flags & (1 << 2)) != 0;
            let incremental = (flags & (1 << 3)) != 0;
            let visibility = if incremental { Visibility::Incremental } else { Visibility::Staged };
            Ok(Self::sink(io_id, patch, visibility))
        }
    }
}

/// Bounded compute work quota for one manual `MediaSession::next` step.
///
/// Each unit buys at most one internal state-machine transition on the direct streaming
/// path (`MediaPump::advance`, one bounded input pull, or one bounded output/flush/commit
/// transition). Multi-node buffered still/animation DAG execution completes its prepared
/// `zenpipe` graph in one execution unit after checking cooperative stop and deadline.
/// When exhausted before an I/O or terminal event occurs, `next` returns `IoEvent::Yield`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkQuota(pub u32);

impl WorkQuota {
    /// Default bounded manual quantum (64 state transitions).
    pub const DEFAULT: Self = Self(64);
}

/// First-cause stop reason recorded on a job's [`JobStop`] controller (`§5.2`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum StopCause {
    /// No stop condition latched.
    None = 0,
    /// Caller explicitly requested cancellation.
    CallerCancel = 1,
    /// Job wall-clock deadline expired.
    Deadline = 2,
    /// Async runtime is shutting down.
    RuntimeShutdown = 3,
    /// Host aborted the dialogue via `Reply::Aborted` (or abandoned an active handle).
    HostAbort = 4,
}

impl StopCause {
    /// Convert from internal `u8` tag.
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::CallerCancel,
            2 => Self::Deadline,
            3 => Self::RuntimeShutdown,
            4 => Self::HostAbort,
            _ => Self::None,
        }
    }

    /// Convert to wire `u32` (`IfxEventV1.reason`).
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }
}

/// Specific input request emitted in [`IoEvent::NeedInput`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IoInput {
    /// Supply bytes starting at `next_offset`, up to `max_transfer`, toward `min_remaining`.
    AtLeast {
        next_offset: u64,
        min_remaining: u64,
        max_transfer: u32,
    },
    /// Supply an explicit random-access byte range starting at `offset` of length `len`.
    Range {
        offset: u64,
        len: u32,
    },
    /// Supply total input length (`Reply::Length` or `Reply::UnknownLength`).
    Len,
}

/// Level-triggered sans-I/O event emitted by [`MediaSession::next`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IoEvent {
    /// Session needs input bytes or length for `io_id`.
    NeedInput {
        ticket: IoTicket,
        io_id: i32,
        request: IoInput,
    },
    /// Session has `len` pending append output bytes at `offset` for `io_id`.
    Output {
        ticket: IoTicket,
        io_id: i32,
        offset: u64,
        len: u32,
        incremental: bool,
    },
    /// Session has `len` pending seek-back patch bytes at `offset` for `io_id`.
    Patch {
        ticket: IoTicket,
        io_id: i32,
        offset: u64,
        len: u32,
        incremental: bool,
    },
    /// Session requests sink flush on `io_id`.
    Flush {
        ticket: IoTicket,
        io_id: i32,
    },
    /// Session has won the `Finishing -> Committing` race and authorizes transactional commit.
    Commit {
        ticket: IoTicket,
    },
    /// Bounded `WorkQuota` exhausted; caller should step again.
    Yield,
    /// Job completed cleanly; `result_json()` is now readable.
    Complete,
    /// Job terminated with failure; `result_json()` contains structured diagnostics.
    Failed {
        category: ErrorCategory,
        cause: StopCause,
    },
}

/// Host reply to an active [`IoTicket`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reply<'a> {
    /// Positive bounded byte chunk for `NeedAtLeast` or `NeedRange`.
    Bytes(&'a [u8]),
    /// End-of-stream reached, optionally carrying exact total source length.
    End { total_len: Option<u64> },
    /// Exact total source byte length for `NeedLen`.
    Length(u64),
    /// Source length is unknown (only legal when forward-only plan is admissible).
    UnknownLength,
    /// Positive accepted byte count for `Output`/`Patch`, or `0` for `Flush`/`Commit`.
    Ack { bytes: u64 },
    /// Host explicitly aborts the dialogue.
    Aborted,
    /// Host encountered an I/O failure, optionally reporting known accepted prefix bytes
    /// within the currently offered transfer chunk.
    IoError {
        accepted_prefix: Option<u64>,
        diagnostic: &'a [u8],
    },
}

// Single-atomic JobStop state machine unifying stop causes, Commit authorization, and abandonment!
const STATE_RUNNING: u8 = 0;
// 1..=4 map directly to StopCause::{CallerCancel, Deadline, RuntimeShutdown, HostAbort}.
const STATE_COMMITTING: u8 = 5;
const STATE_ABANDONED: u8 = 6;

type StopHook = Box<dyn Fn(StopCause) + Send + Sync>;

struct JobStopInner {
    state: AtomicU8,
    hooks: Mutex<Vec<StopHook>>,
}

/// Shared, thread-safe, single-atomic cancellation and commit-authorization controller
/// implementing [`enough::Stop`].
///
/// Unifies first-cause cancellation (`trip`) and staged commit authorization
/// (`try_enter_committing`) on a SINGLE `AtomicU8` so cancellation and Commit can NEVER
/// both win, while also providing an explicit `abandon()` transition so releasing a handle
/// during an in-flight `Commit` still guarantees worker quiescence.
#[derive(Clone)]
pub struct JobStop {
    inner: Arc<JobStopInner>,
}

impl Default for JobStop {
    fn default() -> Self {
        Self::new()
    }
}

impl JobStop {
    /// Create an un-stopped [`JobStop`] controller in `Running` state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(JobStopInner {
                state: AtomicU8::new(STATE_RUNNING),
                hooks: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Trip this stop controller with `cause`. Returns `true` iff this call won the
    /// `Running -> Stopped(cause)` transition.
    ///
    /// Returns `false` (and does NOT latch stop) if `try_enter_committing()` already won!
    pub fn trip(&self, cause: StopCause) -> bool {
        if cause == StopCause::None {
            return false;
        }
        let won = self
            .inner
            .state
            .compare_exchange(
                STATE_RUNNING,
                cause as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok();
        if won {
            self.fire_hooks(cause);
        }
        won
    }

    /// Atomically attempt the `Running -> Committing` transition on the shared state atomic.
    ///
    /// - Returns `Ok(())` iff Commit won the race (subsequent `trip()` calls will fail and
    ///   return `false`, keeping `is_stopped() == false`).
    /// - Returns `Err(cause)` iff a stop cause (or abandonment) already won before Commit.
    pub fn try_enter_committing(&self) -> std::result::Result<(), StopCause> {
        match self.inner.state.compare_exchange(
            STATE_RUNNING,
            STATE_COMMITTING,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(()),
            Err(STATE_COMMITTING) => Ok(()),
            Err(STATE_ABANDONED) => Err(StopCause::HostAbort),
            Err(other) => Err(StopCause::from_u8(other)),
        }
    }

    /// Explicitly abandon this job when the sole host handle is released/dropped.
    ///
    /// - From `Running`: transitions to `CallerCancel` and fires stop hooks.
    /// - From `Committing`: transitions to `Abandoned` and fires stop hooks so an async worker
    ///   waiting for an impossible `Commit` reply exits cleanly and reaches quiescence.
    pub fn abandon(&self) -> bool {
        loop {
            let cur = self.inner.state.load(Ordering::Acquire);
            match cur {
                STATE_RUNNING => {
                    if self
                        .inner
                        .state
                        .compare_exchange(
                            STATE_RUNNING,
                            StopCause::CallerCancel as u8,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        self.fire_hooks(StopCause::CallerCancel);
                        return true;
                    }
                }
                STATE_COMMITTING => {
                    if self
                        .inner
                        .state
                        .compare_exchange(
                            STATE_COMMITTING,
                            STATE_ABANDONED,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        self.fire_hooks(StopCause::HostAbort);
                        return true;
                    }
                }
                _ => return false,
            }
        }
    }

    /// Whether an already-authorized `Commit` was abandoned by host handle release/drop.
    #[must_use]
    pub fn is_abandoned(&self) -> bool {
        self.inner.state.load(Ordering::Acquire) == STATE_ABANDONED
    }

    /// Whether `Committing` currently owns the transition (and has not been abandoned).
    #[must_use]
    pub fn is_committing(&self) -> bool {
        self.inner.state.load(Ordering::Acquire) == STATE_COMMITTING
    }

    fn fire_hooks(&self, cause: StopCause) {
        let hooks: Vec<StopHook> = match self.inner.hooks.lock() {
            Ok(mut guard) => std::mem::take(&mut *guard),
            Err(poisoned) => std::mem::take(&mut *poisoned.into_inner()),
        };
        for hook in hooks {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                hook(cause);
            }));
        }
    }

    /// Register a one-shot hook fired immediately on first stop/abandon (or right now if already stopped).
    pub fn on_stop(&self, hook: impl Fn(StopCause) + Send + Sync + 'static) {
        let current = self.cause();
        if current != StopCause::None {
            hook(current);
            return;
        }
        let fire_now;
        {
            let mut guard = match self.inner.hooks.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            let recheck = self.cause();
            if recheck != StopCause::None {
                fire_now = recheck;
            } else {
                guard.push(Box::new(hook));
                return;
            }
        }
        hook(fire_now);
    }

    /// Return the latched first [`StopCause`] (or [`StopCause::None`] when `Running` or `Committing`).
    #[must_use]
    pub fn cause(&self) -> StopCause {
        match self.inner.state.load(Ordering::Acquire) {
            STATE_RUNNING | STATE_COMMITTING => StopCause::None,
            STATE_ABANDONED => StopCause::HostAbort,
            other => StopCause::from_u8(other),
        }
    }

    /// Returns `true` if any stop cause (or abandonment) has latched.
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.cause() != StopCause::None
    }
}

impl Stop for JobStop {
    #[inline]
    fn check(&self) -> std::result::Result<(), StopReason> {
        match self.cause() {
            StopCause::None => Ok(()),
            StopCause::Deadline => Err(StopReason::TimedOut),
            StopCause::CallerCancel | StopCause::RuntimeShutdown | StopCause::HostAbort => {
                Err(StopReason::Cancelled)
            }
        }
    }

    #[inline]
    fn should_stop(&self) -> bool {
        self.is_stopped()
    }
}

/// Cooperative [`Stop`] wrapper carrying one absolute monotonic `deadline_at` alongside a
/// [`JobStop`] controller so demuxers, decoders, encoders, muxers, and estimators observe
/// wall-clock deadline expiry mid-operation and latch [`StopCause::Deadline`] immediately.
#[derive(Clone)]
pub(crate) struct DeadlineJobStop {
    stop: JobStop,
    deadline_at: Option<Instant>,
}

impl DeadlineJobStop {
    #[must_use]
    pub(crate) fn new(stop: JobStop, deadline_at: Option<Instant>) -> Self {
        Self { stop, deadline_at }
    }

    #[inline]
    pub(crate) fn check_clock(&self) {
        if let Some(at) = self.deadline_at
            && Instant::now() >= at
        {
            let _ = self.stop.trip(StopCause::Deadline);
        }
    }
}

impl Stop for DeadlineJobStop {
    #[inline]
    fn check(&self) -> std::result::Result<(), StopReason> {
        self.check_clock();
        self.stop.check()
    }

    #[inline]
    fn should_stop(&self) -> bool {
        self.check_clock();
        self.stop.is_stopped()
    }
}

/// Safe synchronous random-access byte source for deferred [`Context::add_input_read_at`]
/// and C `imageflow_context_add_input_read_at_v1`.
pub trait ReadAtSource: Send + Sync {
    /// Read up to `dst.len()` bytes starting at `offset`.
    /// Returns `Ok(0)` exclusively on EOF at/beyond end of source.
    fn read_at(&self, offset: u64, dst: &mut [u8]) -> Result<usize>;

    /// Return exact total byte length if known, or `Ok(None)` if unknown.
    fn length(&self) -> Result<Option<u64>>;
}

/// Portable safe `ReadAtSource` wrapping an opened `std::fs::File`.
pub struct FileReadAtSource {
    file: Mutex<File>,
}

impl FileReadAtSource {
    /// Wrap an already-opened file handle.
    #[must_use]
    pub fn new(file: File) -> Self {
        Self { file: Mutex::new(file) }
    }
}

impl ReadAtSource for FileReadAtSource {
    fn read_at(&self, offset: u64, dst: &mut [u8]) -> Result<usize> {
        if dst.is_empty() {
            return Ok(0);
        }
        let mut f = self.file.lock().map_err(|_| {
            nerror!(ErrorKind::DecodingIoError, "FileReadAtSource lock poisoned")
        })?;
        f.seek(SeekFrom::Start(offset))
            .map_err(|e| nerror!(ErrorKind::DecodingIoError, "seek failed at {}: {}", offset, e))?;
        f.read(dst)
            .map_err(|e| nerror!(ErrorKind::DecodingIoError, "read failed at {}: {}", offset, e))
    }

    fn length(&self) -> Result<Option<u64>> {
        let f = self.file.lock().map_err(|_| {
            nerror!(ErrorKind::DecodingIoError, "FileReadAtSource lock poisoned")
        })?;
        let meta = f
            .metadata()
            .map_err(|e| nerror!(ErrorKind::DecodingIoError, "metadata failed: {}", e))?;
        Ok(Some(meta.len()))
    }
}

/// In-memory `ReadAtSource` backed by a shared byte vector (used for tests & replayable leases).
pub struct SliceReadAtSource {
    bytes: Arc<Vec<u8>>,
    report_length: bool,
}

impl SliceReadAtSource {
    /// Create a `ReadAtSource` with known length.
    #[must_use]
    pub fn new(bytes: Arc<Vec<u8>>) -> Self {
        Self { bytes, report_length: true }
    }

    /// Create a `ReadAtSource` that reports unknown length (for testing forward/unknown-length paths).
    #[must_use]
    pub fn with_unknown_length(bytes: Arc<Vec<u8>>) -> Self {
        Self { bytes, report_length: false }
    }
}

impl ReadAtSource for SliceReadAtSource {
    fn read_at(&self, offset: u64, dst: &mut [u8]) -> Result<usize> {
        let Ok(off) = usize::try_from(offset) else {
            return Ok(0);
        };
        if off >= self.bytes.len() || dst.is_empty() {
            return Ok(0);
        }
        let avail = &self.bytes[off..];
        let n = avail.len().min(dst.len());
        dst[..n].copy_from_slice(&avail[..n]);
        Ok(n)
    }

    fn length(&self) -> Result<Option<u64>> {
        if self.report_length {
            Ok(Some(self.bytes.len() as u64))
        } else {
            Ok(None)
        }
    }
}

/// Read all bytes from a [`ReadAtSource`] respecting `max_bytes` ceiling and cooperative stop.
pub(crate) fn read_all_bounded_with_stop(
    src: &dyn ReadAtSource,
    max_bytes: Option<u64>,
    stop: Option<&dyn Stop>,
) -> Result<Vec<u8>> {
    let cap_u64 = max_bytes.unwrap_or(256 * 1024 * 1024);
    if let Some(s) = stop
        && s.should_stop()
    {
        return Err(nerror!(ErrorKind::OperationCancelled, "cancelled before reading source"));
    }
    if let Some(len) = src.length()?
        && len > cap_u64
    {
        return Err(nerror!(
            ErrorKind::SizeLimitExceeded,
            "input length {} exceeds byte limit {}",
            len,
            cap_u64
        ));
    }
    let mut out = Vec::new();
    let mut offset: u64 = 0;
    let mut buf = [0u8; 65536];
    loop {
        if let Some(s) = stop
            && s.should_stop()
        {
            return Err(nerror!(ErrorKind::OperationCancelled, "cancelled while reading source"));
        }
        let n = src.read_at(offset, &mut buf)?;
        if n == 0 {
            break;
        }
        let next_total = offset.saturating_add(n as u64);
        if next_total > cap_u64 {
            return Err(nerror!(
                ErrorKind::SizeLimitExceeded,
                "input bytes {} exceed byte limit {}",
                next_total,
                cap_u64
            ));
        }
        out.try_reserve(n)
            .map_err(|_| nerror!(ErrorKind::AllocationFailed, "input buffer reserve failed"))?;
        out.extend_from_slice(&buf[..n]);
        offset = next_total;
    }
    Ok(out)
}

/// Read an exact byte range `[offset, offset + len)` from a [`ReadAtSource`] in bounded loops,
/// returning fewer than `len` bytes ONLY on genuine EOF.
pub(crate) fn read_exact_range_with_stop(
    src: &dyn ReadAtSource,
    offset: u64,
    len: usize,
    stop: Option<&dyn Stop>,
) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    out.try_reserve(len)
        .map_err(|_| nerror!(ErrorKind::AllocationFailed, "range buffer reserve failed"))?;
    let mut cur = offset;
    let mut tmp = [0u8; 65536];
    while out.len() < len {
        if let Some(s) = stop
            && s.should_stop()
        {
            return Err(nerror!(ErrorKind::OperationCancelled, "cancelled during range read"));
        }
        let want = (len - out.len()).min(tmp.len());
        let n = src.read_at(cur, &mut tmp[..want])?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&tmp[..n]);
        cur = cur.saturating_add(n as u64);
    }
    Ok(out)
}
