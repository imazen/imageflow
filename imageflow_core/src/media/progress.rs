//! Zero-cost opt-in progress tracking over `how-far-along` (`PulseTree` / `FnPulse` / `Observer`).
//!
//! Design guarantees:
//! - When progress is NOT opted in (`Option<ProgressTracker> == None` on `MediaSession`), ZERO
//!   `PulseTree`, `Observer`, atomic progress counter, or notification hook is allocated; codecs
//!   receive plain `how_far::StopOnly`.
//! - In [`ProgressMode::PullOnly`], tracking uses `how_far_along::PulseTree` with lock-free atomic
//!   counters and ZERO callbacks or wake notifications — callers poll [`ProgressObserver::try_summary`]
//!   at their own display cadence.
//! - In [`ProgressMode::PacedNotify`], tracking uses `how_far_along::FnPulse` with an atomic
//!   monotonic interval gate so wake notifications fire at most once per `min_interval` (plus on
//!   stage/terminal transitions) and never run user callbacks on codec worker threads.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use how_far_along::{
    Execution, FnPulse, Observer, Outcome, Phase, PhaseSpec, Pulse, PulseTree, Report, Reporter,
    SharedPulse, Status, Stop, Total,
};

use crate::media::bindings::StopCause;

/// Caller-selected progress observation mode (opt-in; omitted means zero cost).
#[derive(Clone)]
pub enum ProgressMode {
    /// Lock-free atomic `PulseTree` + `Observer` ONLY — zero callbacks, zero wake notifications.
    /// Callers poll [`ProgressObserver::try_summary`] / `imageflow_stream_job_progress_v1` at
    /// whatever cadence they want.
    PullOnly,
    /// Atomic `FnPulse` + `Observer` with monotonic pacing so `on_notify` fires at most once per
    /// `min_interval` (plus on stage and terminal transitions).
    PacedNotify {
        on_notify: Arc<dyn Fn() + Send + Sync>,
        min_interval: Duration,
    },
}

/// Wire-stable lifecycle state independent of counted fraction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ProgressStatus {
    Disabled = 0,
    NotStarted = 1,
    Running = 2,
    Succeeded = 3,
    Failed = 4,
    Cancelled = 5,
    Abandoned = 6,
}

/// Zero-allocation point-in-time progress summary read from [`ProgressObserver`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProgressSummary {
    pub status: ProgressStatus,
    pub fraction: Option<f64>,
    pub unresolved_fraction: f64,
    pub is_terminal: bool,
    pub generation: u64,
}

/// Thread-safe, lock-free-friendly observer handle safe to share across FFI and UI threads.
#[derive(Clone, Debug)]
pub struct ProgressObserver {
    observer: Observer,
    generation: Arc<AtomicU64>,
}

impl ProgressObserver {
    /// Non-blocking, allocation-free summary read. Returns `None` only if `how-far-along`'s
    /// metadata cell is momentarily contended by a stage transition (never spins).
    #[must_use]
    pub fn try_summary(&self) -> Option<ProgressSummary> {
        let sum = self.observer.try_summary()?;
        Some(self.encode_summary(sum))
    }

    /// Allocation-free summary read.
    #[must_use]
    pub fn summary(&self) -> ProgressSummary {
        self.encode_summary(self.observer.summary())
    }

    /// Non-blocking export of `how-far-along`'s versioned (`schema_version: 2`) hierarchical
    /// phase-tree snapshot JSON. Returns `None` if metadata is momentarily contended.
    #[must_use]
    pub fn try_snapshot_json(&self) -> Option<String> {
        let snap = self.observer.try_snapshot()?;
        let mut out = String::with_capacity(512);
        let _ = snap.write_json(&mut out);
        Some(out)
    }

    /// Export `how-far-along`'s versioned (`schema_version: 2`) phase-tree snapshot JSON.
    #[must_use]
    pub fn snapshot_json(&self) -> String {
        let snap = self.observer.snapshot();
        let mut out = String::with_capacity(512);
        let _ = snap.write_json(&mut out);
        out
    }

    fn encode_summary(&self, sum: how_far_along::Summary) -> ProgressSummary {
        let (status, is_terminal) = match sum.status {
            Status::NotStarted => (ProgressStatus::NotStarted, false),
            Status::Running => (ProgressStatus::Running, false),
            Status::Finished(Outcome::Succeeded | Outcome::Skipped) => {
                (ProgressStatus::Succeeded, true)
            }
            Status::Finished(Outcome::Cancelled) => (ProgressStatus::Cancelled, true),
            Status::Finished(Outcome::Abandoned) => (ProgressStatus::Abandoned, true),
            Status::Finished(_) => (ProgressStatus::Failed, true),
            _ => (ProgressStatus::Running, false),
        };
        ProgressSummary {
            status,
            fraction: sum.fraction,
            unresolved_fraction: sum.unresolved_fraction,
            is_terminal,
            generation: self.generation.load(Ordering::Acquire),
        }
    }
}

enum ProcessPulse {
    Pull(PulseTree),
    Notify(FnPulse),
}

impl ProcessPulse {
    fn start(&self) {
        match self {
            Self::Pull(t) => {
                let _ = t.start();
            }
            Self::Notify(f) => {
                let _ = f.start();
            }
        }
    }

    fn share(&self) -> Option<SharedPulse> {
        match self {
            Self::Pull(t) => t.share().ok(),
            Self::Notify(f) => f.share().ok(),
        }
    }

    fn finish(self, outcome: Outcome) {
        match self {
            Self::Pull(t) => {
                let _ = t.finish(outcome);
            }
            Self::Notify(f) => {
                let _ = f.finish(outcome);
            }
        }
    }
}

/// Session-owned progress tracker created ONLY when the caller explicitly opts in.
pub(crate) struct ProgressTracker {
    root: Option<Phase>,
    input_phase: Option<Phase>,
    input_reporter: Option<Reporter>,
    process_pulse: Option<ProcessPulse>,
    output_phase: Option<Phase>,
    output_reporter: Option<Reporter>,
    observer: ProgressObserver,
    generation: Arc<AtomicU64>,
    on_notify: Option<Arc<dyn Fn() + Send + Sync>>,
    started_at: Instant,
    last_notify_ns: Arc<AtomicU64>,
    min_interval_ns: u64,
}

impl ProgressTracker {
    pub(crate) fn new<S: Stop + Clone + Send + Sync + 'static>(
        stop: S,
        mode: ProgressMode,
    ) -> (Self, ProgressObserver) {
        let generation = Arc::new(AtomicU64::new(1));
        let started_at = Instant::now();
        let last_notify_ns = Arc::new(AtomicU64::new(0));

        let mut root = Phase::new("job", Total::Unknown);
        let obs = ProgressObserver {
            observer: root.observer(),
            generation: Arc::clone(&generation),
        };
        let split_res = root.split(
            Execution::Sequence,
            [
                PhaseSpec::new("input", 10, Total::Unknown),
                PhaseSpec::new("process", 80, Total::Unknown),
                PhaseSpec::new("output", 10, Total::Unknown),
            ],
        );

        let (input_phase, input_reporter, process_phase, output_phase, output_reporter) =
            match split_res {
                Ok([mut i, p, mut o]) => {
                    let _ = i.set_units("bytes");
                    let _ = o.set_units("bytes");
                    let ir = i.reporter();
                    let or = o.reporter();
                    (Some(i), Some(ir), Some(p), Some(o), Some(or))
                }
                Err(_) => (None, None, None, None, None),
            };

        let (process_pulse, on_notify, min_interval_ns) = match mode {
            ProgressMode::PullOnly => (
                process_phase.map(|p| ProcessPulse::Pull(PulseTree::new(p, stop))),
                None,
                0u64,
            ),
            ProgressMode::PacedNotify { on_notify, min_interval } => {
                let interval_ns = u64::try_from(min_interval.as_nanos())
                    .unwrap_or(u64::MAX)
                    .max(1_000_000); // floor 1 ms to prevent storms
                let cb_stop = stop;
                let cb_gen = Arc::clone(&generation);
                let cb_notify = Arc::clone(&on_notify);
                let cb_last = Arc::clone(&last_notify_ns);
                let cb_start = started_at;
                let pp = process_phase.map(|p| {
                    ProcessPulse::Notify(FnPulse::with_phase(p, move |_cp| {
                        cb_stop.check()?;
                        let now_ns =
                            u64::try_from(cb_start.elapsed().as_nanos()).unwrap_or(u64::MAX);
                        let prev = cb_last.load(Ordering::Relaxed);
                        if (prev == 0 || now_ns.saturating_sub(prev) >= interval_ns)
                            && cb_last
                                .compare_exchange(
                                    prev,
                                    now_ns.max(1),
                                    Ordering::AcqRel,
                                    Ordering::Relaxed,
                                )
                                .is_ok()
                        {
                            cb_gen.fetch_add(1, Ordering::Release);
                            (cb_notify)();
                        }
                        Ok(())
                    }))
                });
                (pp, Some(on_notify), interval_ns)
            }
        };

        let tracker = Self {
            root: Some(root),
            input_phase,
            input_reporter,
            process_pulse,
            output_phase,
            output_reporter,
            observer: obs.clone(),
            generation,
            on_notify,
            started_at,
            last_notify_ns,
            min_interval_ns,
        };
        (tracker, obs)
    }

    pub(crate) fn observer(&self) -> ProgressObserver {
        self.observer.clone()
    }

    pub(crate) fn is_notify_enabled(&self) -> bool {
        self.on_notify.is_some()
    }

    fn bump_and_maybe_notify(&self, force: bool) {
        self.generation.fetch_add(1, Ordering::Release);
        let Some(ref cb) = self.on_notify else {
            return;
        };
        if force {
            let now_ns = u64::try_from(self.started_at.elapsed().as_nanos()).unwrap_or(u64::MAX);
            self.last_notify_ns.store(now_ns.max(1), Ordering::Release);
            (cb)();
            return;
        }
        let now_ns = u64::try_from(self.started_at.elapsed().as_nanos()).unwrap_or(u64::MAX);
        let prev = self.last_notify_ns.load(Ordering::Relaxed);
        if (prev == 0 || now_ns.saturating_sub(prev) >= self.min_interval_ns)
            && self
                .last_notify_ns
                .compare_exchange(prev, now_ns.max(1), Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
        {
            (cb)();
        }
    }

    pub(crate) fn set_input_total(&mut self, total_bytes: u64) {
        if let Some(ref mut inp) = self.input_phase {
            let _ = inp.start();
            let _ = inp.set_total(Total::Exact(total_bytes));
            self.bump_and_maybe_notify(false);
        }
    }

    pub(crate) fn advance_input(&mut self, bytes: u64) {
        if let Some(ref mut inp) = self.input_phase {
            let _ = inp.start();
            if let Some(ref rep) = self.input_reporter {
                rep.advance(bytes);
            }
            self.bump_and_maybe_notify(false);
        }
    }

    /// Finish input stage, activate process stage, and return a `SharedPulse` for `zenpipe`/codecs.
    pub(crate) fn begin_process(&mut self) -> Option<SharedPulse> {
        if let Some(mut inp) = self.input_phase.take() {
            let _ = inp.finish_with(Outcome::Succeeded);
        }
        if let Some(ref proc) = self.process_pulse {
            proc.start();
            self.bump_and_maybe_notify(true);
            return proc.share();
        }
        None
    }

    pub(crate) fn begin_output(&mut self, total_bytes: Option<u64>) {
        if let Some(mut inp) = self.input_phase.take() {
            let _ = inp.finish_with(Outcome::Succeeded);
        }
        if let Some(proc) = self.process_pulse.take() {
            proc.finish(Outcome::Succeeded);
        }
        if let Some(ref mut out) = self.output_phase {
            let _ = out.start();
            if let Some(tb) = total_bytes {
                let _ = out.set_total(Total::Exact(tb));
            }
            self.bump_and_maybe_notify(true);
        }
    }

    pub(crate) fn advance_output(&mut self, bytes: u64) {
        if self.process_pulse.is_some() {
            self.begin_output(None);
        }
        if let Some(ref rep) = self.output_reporter {
            rep.advance(bytes);
            self.bump_and_maybe_notify(false);
        }
    }

    pub(crate) fn finish_succeeded(&mut self) {
        if let Some(mut inp) = self.input_phase.take() {
            let _ = inp.finish_with(Outcome::Succeeded);
        }
        if let Some(proc) = self.process_pulse.take() {
            proc.finish(Outcome::Succeeded);
        }
        if let Some(mut out) = self.output_phase.take() {
            let _ = out.finish_with(Outcome::Succeeded);
        }
        if let Some(mut root) = self.root.take() {
            let _ = root.finish_with(Outcome::Succeeded);
        }
        self.bump_and_maybe_notify(true);
    }

    pub(crate) fn finish_failed(&mut self, cause: StopCause) {
        let outcome = match cause {
            StopCause::CallerCancel | StopCause::Deadline | StopCause::RuntimeShutdown => {
                Outcome::Cancelled
            }
            StopCause::HostAbort => Outcome::Abandoned,
            StopCause::None => Outcome::Failed,
        };
        if let Some(mut inp) = self.input_phase.take() {
            let _ = inp.finish_with(outcome);
        }
        if let Some(proc) = self.process_pulse.take() {
            proc.finish(outcome);
        }
        if let Some(mut out) = self.output_phase.take() {
            let _ = out.finish_with(outcome);
        }
        if let Some(mut root) = self.root.take() {
            let _ = root.finish_with(outcome);
        }
        self.bump_and_maybe_notify(true);
    }
}
