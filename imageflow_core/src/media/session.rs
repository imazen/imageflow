//! Sans-I/O demand-driven `MediaSession` state machine (`§4.1` & `§5.3`).
//!
//! Drives the same canonical `zenpipe::canonical` job and `zencodecs::media::MediaPump`
//! across Rust manual stepping, C `streaming-ffi`, and Tokio/.NET `async-ffi` with:
//! - One outstanding monotonically increasing nonzero [`IoTicket`] at a time,
//! - Level-triggered stable events (`next` before `reply` repeats the identical event),
//! - Arbitrary short/1-byte input chunks (including aggregated exact `Range` reads wider than
//!   a single transfer credit!) and credit-bounded output/patch chunking with partial positive ACKs,
//! - Single-atomic `Running -> Committing` vs `Running -> Stopped` race on [`JobStop`],
//! - Unified request-level `limits` enforcement & live memory budgeting matching sync endpoints,
//! - Exact per-sink publication receipts on both success and failure (marking in-flight
//!   incremental writes / partial patches / abandoned commits as `exact_unknown_tail: true`),
//! - Zero Tokio dependencies and 100% safe Rust.

use std::collections::{BTreeMap, HashSet};
use std::num::NonZeroU64;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::Value;
use zc::decode::{Input, InputRequest};
use zc::source::SourceAccess;
use zencodec_media::registry::{ContainerId, ProbeLimits};
use zencodecs::engine::{Engine, OutputIntent, PublicationMode, Run, TargetFormat, TrackPolicy};
use zencodecs::media::probe::{ContainerProbeOutcome, probe_container_prefix};
use zencodecs::media::pump::{IoTicket as PumpTicket, MediaPump, PumpEvent, WriteOffer};
use zenpipe::canonical::json::parse_pipeline_value;
use zenpipe::canonical::{
    Bindings, CanonicalJob, HostServices, ImageOp, OutputOp, OutputV2, PipelineRuntime,
    WireLimitConfig,
};

use crate::media::bindings::{
    BindingAccess, BindingSpec, DeadlineJobStop, IoEvent, IoInput, IoTicket, JobStop,
    OperationKind, Reply, StopCause, Visibility, WorkQuota,
};
use crate::media::report::{
    PublicationReceiptV2, StreamErrorReportV2, build_estimate_report, inspect_bytes_with_stop,
    probe_selected_video_track_descriptor, select_mux_offer_for_route, video_codec_token,
};
use crate::media::{ModernLimitsV2, apply_modern_limits};
use crate::{ErrorCategory, FlowError};

/// Default per-transfer credit cap (64 KiB) for bounded streaming chunks.
pub const DEFAULT_TRANSFER_MAX: u32 = 64 * 1024;
const DEFAULT_MAX_INPUT_BYTES: u64 = 256 * 1024 * 1024;
const MAX_PROBE_PREFIX: usize = 64 * 1024;
const OUT_HEADER_PROBE_CAP: usize = 64 * 1024;

/// Protocol/API errors returned synchronously by [`MediaSession`] calls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionError {
    /// Invalid argument or malformed reply (leaves session state unchanged).
    InvalidArgument(String),
    /// Unsupported configuration or capability mismatch.
    Unsupported(String),
    /// Exceeded configured byte/count resource cap.
    ResourceLimit(String),
    /// Ticket does not match the active outstanding event (leaves state unchanged).
    StaleTicket,
    /// Session is already terminal/closed; no further replies accepted.
    Closed,
    /// Pending output/result buffer is too small; carries exact required byte count.
    BufferTooSmall(usize),
    /// Internal failure.
    Internal(String),
}

/// Result of [`MediaSession::copy_pending`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CopyPendingError {
    /// Ticket does not match an active `Output` or `Patch` event.
    StaleTicket,
    /// Destination buffer is smaller than the pending chunk; carries exact required bytes.
    BufferTooSmall(usize),
    /// Session is already closed/terminal.
    Closed,
}

#[derive(Clone, Debug)]
struct InputBufferState {
    slot: String,
    io_id: i32,
    spec: BindingSpec,
    data: Vec<u8>,
    /// Charged live memory capacity bytes for `data` inside `MediaSession::live_mem_bytes`.
    charged_cap: u64,
    known_len: Option<u64>,
    len_queried: bool,
    eof: bool,
    consumed_bytes: u64,
}

#[derive(Clone, Debug)]
struct OutputSinkState {
    slot: String,
    io_id: i32,
    spec: BindingSpec,
    accepted_bytes: u64,
    exact_unknown_tail: bool,
    flushed: bool,
    committed: bool,
}

#[derive(Clone, Debug)]
struct PendingChunk {
    ticket: IoTicket,
    sink_idx: usize,
    offset: u64,
    is_patch: bool,
    incremental: bool,
    bytes: Vec<u8>,
    sent_offset: usize,
    /// Exposed credit window length for the current `ticket` (always `1..=transfer_max`).
    window_len: usize,
    /// Charged live memory bytes for `bytes` (released on drop/completion).
    charged_bytes: u64,
    /// For direct `MediaPump` streaming: original pump ticket to ACK once all bytes are ACKed.
    pump_ticket: Option<PumpTicket>,
    pump_total_acked: usize,
}

#[derive(Clone, Debug)]
struct PendingRangeAccum {
    pump_ticket: PumpTicket,
    base_offset: u64,
    target_len: usize,
    buf: Vec<u8>,
    charged_bytes: u64,
}

#[derive(Clone, Debug)]
enum ActiveWait {
    None,
    Input {
        ticket: IoTicket,
        input_idx: usize,
        request: IoInput,
        pump_ticket: Option<PumpTicket>,
    },
    Output(PendingChunk),
    Flush {
        ticket: IoTicket,
        sink_idx: usize,
    },
    Commit {
        ticket: IoTicket,
    },
}

enum SessionMode {
    Execute {
        job: CanonicalJob,
        /// Optional direct streaming `MediaPump` candidate (single video in -> single video out).
        direct_pump_intent: Option<OutputIntent>,
        direct_pump: Option<DirectPumpDriver>,
    },
    Inspect {
        target_slot: String,
        expected_container: Option<String>,
        bounded_index_scan: bool,
    },
    Estimate {
        job: CanonicalJob,
    },
}

enum SessionModePlan {
    Execute {
        job: CanonicalJob,
        direct_pump_intent: Option<OutputIntent>,
    },
    Inspect {
        target_slot: String,
        expected_container: Option<String>,
        bounded_index_scan: bool,
    },
    Estimate {
        job: CanonicalJob,
    },
}

/// Movable (`Send + 'static`) pre-stepped blueprint extracted from an un-stepped [`MediaSession`].
///
/// Allows async workers to construct, drive, and drop thread-affine (`!Send`) `MediaPump` /
/// codec instances strictly on a single owned blocking worker thread (§4.2).
pub struct MediaSessionPlan {
    engine: Arc<Engine>,
    services: HostServices,
    stop: JobStop,
    mode: SessionModePlan,
    inputs: Vec<InputBufferState>,
    sinks: Vec<OutputSinkState>,
    max_input_bytes: u64,
    max_output_bytes: Option<u64>,
    max_memory_bytes: Option<u64>,
    live_mem_bytes: u64,
    deadline: Option<Duration>,
    deadline_at: Option<Instant>,
    transfer_max: u32,
}

#[cfg(feature = "streaming-async")]
impl MediaSessionPlan {
    /// Reserve bounded async transport copy overhead (one mutually-exclusive transfer snapshot
    /// or input reply copy `<= transfer_max`) inside `live_mem_bytes` BEFORE starting the worker,
    /// shrinking `transfer_max` if needed so `transport_cap <= max_memory_bytes / 8`.
    pub(crate) fn reserve_async_transport_budget(&mut self) {
        if let Some(cap) = self.max_memory_bytes {
            let transport_cap = u64::from(self.transfer_max).min(cap / 8).max(1);
            self.transfer_max = u32::try_from(transport_cap).unwrap_or(u32::MAX);
            self.live_mem_bytes = self.live_mem_bytes.saturating_add(transport_cap);
        }
    }
}

struct DirectPumpDriver {
    pump: MediaPump,
    /// Conservative memory ceiling reserved for `pump` inside `MediaSession::live_mem_bytes`
    /// for the entire lifetime of `pump` (released ONLY when `pump` is dropped!).
    pump_reserved_bytes: u64,
    out_offset: u64,
    out_total_bytes: u64,
    in_buf: Vec<u8>,
    in_buf_charged: u64,
    in_eof: bool,
    in_known_len: Option<u64>,
    pending_range: Option<PendingRangeAccum>,
    /// Contiguous emitted output prefix (`0..<=64 KiB`, assembled strictly from emitted
    /// WriteOffer chunks and finish patches, charged in `live_mem_bytes`) inspected ONCE
    /// after `pump` drops!
    out_header: Vec<u8>,
    out_header_charged: u64,
    finished: bool,
    target_container: ContainerId,
    target_codec: zencodec_media::track::CodecId,
    output_slot: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lifecycle {
    Running,
    Finishing,
    Committing,
    Complete,
    Failed { category: ErrorCategory, cause: StopCause },
}

/// Sans-I/O demand-driven execution session over one frozen `Engine` + `HostServices`.
pub struct MediaSession {
    engine: Arc<Engine>,
    services: HostServices,
    stop: JobStop,
    next_ticket: u64,
    lifecycle: Lifecycle,
    mode: SessionMode,
    inputs: Vec<InputBufferState>,
    sinks: Vec<OutputSinkState>,
    active_wait: ActiveWait,
    /// Queued finalized payloads for DAG/still/multi-output publishing.
    queued_outputs: Vec<(usize, Vec<u8>)>,
    /// Pre-built success `OutputV2` / inspect / estimate JSON held until commit/complete.
    staged_result_json: Option<Vec<u8>>,
    /// Final terminal JSON exposed via `result_json()`.
    terminal_json: Option<Vec<u8>>,
    max_input_bytes: u64,
    max_output_bytes: Option<u64>,
    max_memory_bytes: Option<u64>,
    live_mem_bytes: u64,
    deadline: Option<Duration>,
    deadline_at: Option<Instant>,
    transfer_max: u32,
}

impl MediaSession {
    /// Create a frozen sans-I/O [`MediaSession`] from operation kind, strict JSON envelope,
    /// binding descriptors, and snapshotted engine/host policy.
    ///
    /// Retains ZERO reference to any `Context`.
    pub fn create(
        engine: Arc<Engine>,
        services: HostServices,
        operation: OperationKind,
        json: &[u8],
        bindings: &[BindingSpec],
    ) -> std::result::Result<Self, SessionError> {
        let stop = JobStop::new();
        Self::create_with_stop_and_deadline(engine, services, operation, json, bindings, stop, None)
    }

    /// Create a [`MediaSession`] bound to a caller-provided [`JobStop`] controller.
    pub fn create_with_stop(
        engine: Arc<Engine>,
        services: HostServices,
        operation: OperationKind,
        json: &[u8],
        bindings: &[BindingSpec],
        stop: JobStop,
    ) -> std::result::Result<Self, SessionError> {
        Self::create_with_stop_and_deadline(engine, services, operation, json, bindings, stop, None)
    }

    /// Create a [`MediaSession`] with an optional context-level wall-clock timeout ceiling.
    pub fn create_with_stop_and_deadline(
        engine: Arc<Engine>,
        services: HostServices,
        operation: OperationKind,
        json: &[u8],
        bindings: &[BindingSpec],
        stop: JobStop,
        base_timeout_ms: Option<u64>,
    ) -> std::result::Result<Self, SessionError> {
        let base_deadline_at =
            base_timeout_ms.and_then(|ms| Instant::now().checked_add(Duration::from_millis(ms)));
        Self::create_with_stop_and_deadline_at(
            engine,
            services,
            operation,
            json,
            bindings,
            stop,
            base_timeout_ms.map(Duration::from_millis),
            base_deadline_at,
        )
    }

    /// Create a [`MediaSession`] carrying an existing absolute monotonic deadline `Instant`
    /// (preventing clock restarts when a caller already elapsed time before session setup).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn create_with_stop_and_deadline_at(
        engine: Arc<Engine>,
        mut services: HostServices,
        operation: OperationKind,
        json: &[u8],
        bindings: &[BindingSpec],
        stop: JobStop,
        base_deadline: Option<Duration>,
        base_deadline_at: Option<Instant>,
    ) -> std::result::Result<Self, SessionError> {
        if json.is_empty() {
            return Err(SessionError::InvalidArgument("JSON envelope is empty".to_string()));
        }
        let max_json = 256 * 1024usize;
        if json.len() > max_json {
            return Err(SessionError::ResourceLimit(format!(
                "JSON envelope {} B exceeds max {}",
                json.len(),
                max_json
            )));
        }

        // Validate unique binding io_ids.
        let mut by_id: BTreeMap<i32, BindingSpec> = BTreeMap::new();
        for &b in bindings {
            if by_id.insert(b.io_id, b).is_some() {
                return Err(SessionError::InvalidArgument(format!(
                    "duplicate binding io_id {}",
                    b.io_id
                )));
            }
        }

        let root: Value = serde_json::from_slice(json)
            .map_err(|e| SessionError::InvalidArgument(format!("invalid JSON: {e}")))?;
        let obj = root
            .as_object()
            .ok_or_else(|| SessionError::InvalidArgument("envelope root must be an object".into()))?;

        // Strictly parse and intersect envelope `limits` IDENTICALLY to sync endpoints,
        // without restarting any already-established `base_deadline_at`!
        let req_limits: Option<ModernLimitsV2> = match obj.get("limits") {
            None | Some(Value::Null) => None,
            Some(v) => Some(serde_json::from_value(v.clone()).map_err(|e| {
                SessionError::InvalidArgument(format!("invalid envelope 'limits': {e}"))
            })?),
        };
        let env_deadline_ms = apply_modern_limits(&mut services, req_limits.as_ref(), None);
        let env_deadline = env_deadline_ms.map(Duration::from_millis);
        let env_deadline_at = env_deadline.and_then(|d| Instant::now().checked_add(d));
        let deadline = match (base_deadline, env_deadline) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) | (None, Some(a)) => Some(a),
            (None, None) => None,
        };
        let deadline_at = match (base_deadline_at, env_deadline_at) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) | (None, Some(a)) => Some(a),
            (None, None) => None,
        };

        let input_map = parse_slot_id_map(obj.get("inputs"), "inputs")?;
        let output_map = parse_slot_id_map(obj.get("outputs"), "outputs")?;

        // Match inputs and outputs 1:1 against declared transport bindings.
        let mut used_ids: HashSet<i32> = HashSet::new();
        let mut inputs = Vec::new();
        for (slot, io_id) in &input_map {
            let spec = *by_id.get(io_id).ok_or_else(|| {
                SessionError::InvalidArgument(format!(
                    "input slot '{}' references unbound io_id {}",
                    slot, io_id
                ))
            })?;
            if !spec.is_source() {
                return Err(SessionError::InvalidArgument(format!(
                    "input slot '{}' io_id {} is bound as sink, expected source",
                    slot, io_id
                )));
            }
            if !used_ids.insert(*io_id) {
                return Err(SessionError::InvalidArgument(format!(
                    "io_id {} mapped more than once in envelope",
                    io_id
                )));
            }
            inputs.push(InputBufferState {
                slot: slot.clone(),
                io_id: *io_id,
                spec,
                data: Vec::new(),
                charged_cap: 0,
                known_len: None,
                len_queried: false,
                eof: false,
                consumed_bytes: 0,
            });
        }

        let mut sinks = Vec::new();
        if operation == OperationKind::Inspect {
            if !output_map.is_empty() {
                return Err(SessionError::InvalidArgument(
                    "Inspect operation does not accept output slots".to_string(),
                ));
            }
        } else {
            for (slot, io_id) in &output_map {
                let spec = *by_id.get(io_id).ok_or_else(|| {
                    SessionError::InvalidArgument(format!(
                        "output slot '{}' references unbound io_id {}",
                        slot, io_id
                    ))
                })?;
                if !spec.is_sink() {
                    return Err(SessionError::InvalidArgument(format!(
                        "output slot '{}' io_id {} is bound as source, expected sink",
                        slot, io_id
                    )));
                }
                if !used_ids.insert(*io_id) {
                    return Err(SessionError::InvalidArgument(format!(
                        "io_id {} mapped more than once in envelope",
                        io_id
                    )));
                }
                sinks.push(OutputSinkState {
                    slot: slot.clone(),
                    io_id: *io_id,
                    spec,
                    accepted_bytes: 0,
                    exact_unknown_tail: false,
                    flushed: false,
                    committed: false,
                });
            }
        }

        // Reject unused/extraneous bindings so transport contracts stay exact.
        for &io_id in by_id.keys() {
            if !used_ids.contains(&io_id) {
                return Err(SessionError::InvalidArgument(format!(
                    "binding io_id {} is not referenced by envelope inputs/outputs",
                    io_id
                )));
            }
        }

        let max_input_bytes = services
            .limits
            .and_then(|l| l.max_input_bytes)
            .unwrap_or(DEFAULT_MAX_INPUT_BYTES);
        let max_output_bytes = services.limits.and_then(|l| l.max_output_bytes);
        let max_memory_bytes = services.limits.and_then(|l| l.max_memory_bytes);

        let mode = match operation {
            OperationKind::Inspect => {
                for k in obj.keys() {
                    if !matches!(k.as_str(), "inputs" | "input" | "limits" | "video" | "scan") {
                        return Err(SessionError::InvalidArgument(format!(
                            "unknown key '{}' in Inspect envelope",
                            k
                        )));
                    }
                }
                let target_slot = obj
                    .get("input")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        SessionError::InvalidArgument(
                            "Inspect envelope requires string 'input' slot name".to_string(),
                        )
                    })?
                    .to_string();
                if !inputs.iter().any(|i| i.slot == target_slot) {
                    return Err(SessionError::InvalidArgument(format!(
                        "Inspect 'input' slot '{}' not found in 'inputs'",
                        target_slot
                    )));
                }
                let expected_container = obj
                    .get("video")
                    .and_then(Value::as_object)
                    .and_then(|v| v.get("container"))
                    .and_then(Value::as_str)
                    .map(String::from);
                let bounded_index_scan = match obj.get("scan").and_then(Value::as_str) {
                    None | Some("headers") => false,
                    Some("bounded_index") => true,
                    Some(other) => {
                        return Err(SessionError::InvalidArgument(format!(
                            "invalid Inspect scan mode '{}' (expected 'headers' or 'bounded_index')",
                            other
                        )));
                    }
                };
                SessionMode::Inspect { target_slot, expected_container, bounded_index_scan }
            }
            OperationKind::Execute | OperationKind::Estimate => {
                for k in obj.keys() {
                    if !matches!(k.as_str(), "inputs" | "outputs" | "pipeline" | "limits") {
                        return Err(SessionError::InvalidArgument(format!(
                            "unknown key '{}' in execute/estimate envelope",
                            k
                        )));
                    }
                }
                let pipe_val = obj.get("pipeline").ok_or_else(|| {
                    SessionError::InvalidArgument(
                        "execute/estimate envelope requires 'pipeline' object".to_string(),
                    )
                })?;
                let job = parse_pipeline_value(pipe_val, WireLimitConfig::default())
                    .map_err(|e| SessionError::InvalidArgument(e.to_string()))?;

                // Validate graph output slots match declared output bindings.
                for out_op in job.graph().outputs() {
                    let slot = match out_op {
                        OutputOp::Encode(enc) => enc.slot.as_str(),
                        OutputOp::Pixels { slot, .. } => slot.as_str(),
                        _ => continue,
                    };
                    if !sinks.iter().any(|s| s.slot == slot) {
                        return Err(SessionError::InvalidArgument(format!(
                            "pipeline output slot '{}' is missing from 'outputs' bindings",
                            slot
                        )));
                    }
                }

                if operation == OperationKind::Estimate {
                    SessionMode::Estimate { job }
                } else {
                    let direct_pump_intent = extract_direct_video_pump_intent(&job, &inputs, &sinks);
                    SessionMode::Execute { job, direct_pump_intent, direct_pump: None }
                }
            }
        };

        Ok(Self {
            engine,
            services,
            stop,
            next_ticket: 1,
            lifecycle: Lifecycle::Running,
            mode,
            inputs,
            sinks,
            active_wait: ActiveWait::None,
            queued_outputs: Vec::new(),
            staged_result_json: None,
            terminal_json: None,
            max_input_bytes,
            max_output_bytes,
            max_memory_bytes,
            live_mem_bytes: 0,
            deadline,
            deadline_at,
            transfer_max: DEFAULT_TRANSFER_MAX,
        })
    }

    /// Borrow this session's thread-safe [`JobStop`] controller.
    #[must_use]
    pub fn stop_handle(&self) -> JobStop {
        self.stop.clone()
    }

    /// Optional resolved wall-clock deadline configured via context or envelope `limits.timeout_ms`.
    #[must_use]
    pub const fn configured_deadline(&self) -> Option<Duration> {
        self.deadline
    }

    /// Resolved absolute monotonic deadline instant, if any.
    #[cfg(feature = "streaming-async")]
    #[must_use]
    pub(crate) const fn deadline_at(&self) -> Option<Instant> {
        self.deadline_at
    }

    /// Install a unified absolute monotonic deadline instant (used by async driver so the Tokio
    /// timer and in-session `DeadlineJobStop` share one exact clock deadline).
    #[cfg(feature = "streaming-async")]
    pub(crate) fn set_deadline_at(&mut self, at: Option<Instant>) {
        self.deadline_at = at;
    }

    /// Configured hard memory ceiling (`max_memory_bytes`), if any.
    #[cfg(feature = "streaming-async")]
    #[must_use]
    pub(crate) const fn max_memory_bytes(&self) -> Option<u64> {
        self.max_memory_bytes
    }

    /// Return the terminal event if this session has already reached `Complete` or `Failed`.
    #[cfg(feature = "streaming-async")]
    #[must_use]
    pub(crate) fn current_terminal_event_if_done(&self) -> Option<IoEvent> {
        match self.lifecycle {
            Lifecycle::Complete => Some(IoEvent::Complete),
            Lifecycle::Failed { category, cause } => Some(IoEvent::Failed { category, cause }),
            _ => None,
        }
    }

    /// Terminate this session when an internal transport failure occurs BEFORE the current
    /// active ticket (e.g. `Output` or `Patch`) was published to the host.
    ///
    /// Clears any unpublished `ActiveWait` WITHOUT marking the sink's tail unknown (because
    /// zero bytes of the unpublished ticket reached the host), then builds the authoritative
    /// `StreamErrorReportV2` receipt preserving every sink's previously acknowledged
    /// `accepted_bytes`, `exact_unknown_tail`, and `committed` flags!
    #[cfg(feature = "streaming-async")]
    pub(crate) fn fail_unpublished_ticket(
        &mut self,
        category: ErrorCategory,
        message: String,
    ) -> IoEvent {
        if let Some(term) = self.current_terminal_event_if_done() {
            return term;
        }
        if let ActiveWait::Output(chunk) = std::mem::replace(&mut self.active_wait, ActiveWait::None)
        {
            self.release_mem(chunk.charged_bytes);
        }
        let cause = self.stop.cause();
        self.fail_with(category, cause, message);
        self.current_terminal_event()
    }

    fn deadline_stop(&self) -> DeadlineJobStop {
        DeadlineJobStop::new(self.stop.clone(), self.deadline_at)
    }

    fn deadline_pulse(&self) -> zc::SharedPulse {
        zc::SharedPulse::new(zc::how_far::StopOnly::new(self.deadline_stop()))
    }

    /// Override per-transfer I/O credit ceiling (clamped to `1..=256 KiB`). Useful for testing
    /// sub-box range aggregation and bounded output credit splitting.
    pub fn set_transfer_max(&mut self, max_bytes: u32) {
        self.transfer_max = max_bytes.clamp(1, 256 * 1024);
    }

    /// Explicitly abandon an in-flight `Commit` transaction when the sole caller handle is
    /// released/dropped without resolving the `Commit` ticket.
    ///
    /// Marks all staged sinks `committed = false` and `exact_unknown_tail = true` (explicit
    /// unknown external publication outcome) and transitions the session to terminal `Failed`.
    pub fn abandon_commit(&mut self) {
        if self.is_terminal() {
            return;
        }
        let _ = self.stop.abandon();
        for s in &mut self.sinks {
            if s.spec.visibility == Some(Visibility::Staged) && !s.committed {
                s.exact_unknown_tail = true;
            }
        }
        self.fail_with(
            ErrorCategory::OperationCancelled,
            StopCause::HostAbort,
            "host abandoned in-flight staged Commit without resolving ticket".to_string(),
        );
    }

    /// Extract a movable (`Send + 'static`) [`MediaSessionPlan`] from a freshly created,
    /// un-stepped [`MediaSession`] so an async worker thread can construct and own all
    /// thread-affine (`!Send`) codec/mux state on one dedicated thread (§4.2).
    pub fn try_into_plan(self) -> std::result::Result<MediaSessionPlan, Self> {
        let can_move = self.next_ticket == 1
            && self.lifecycle == Lifecycle::Running
            && matches!(self.active_wait, ActiveWait::None)
            && !matches!(self.mode, SessionMode::Execute { direct_pump: Some(_), .. });
        if !can_move {
            return Err(self);
        }
        let mode = match self.mode {
            SessionMode::Execute { job, direct_pump_intent, direct_pump: None } => {
                SessionModePlan::Execute { job, direct_pump_intent }
            }
            SessionMode::Execute { .. } => unreachable!("checked direct_pump is None"),
            SessionMode::Inspect { target_slot, expected_container, bounded_index_scan } => {
                SessionModePlan::Inspect { target_slot, expected_container, bounded_index_scan }
            }
            SessionMode::Estimate { job } => SessionModePlan::Estimate { job },
        };
        Ok(MediaSessionPlan {
            engine: self.engine,
            services: self.services,
            stop: self.stop,
            mode,
            inputs: self.inputs,
            sinks: self.sinks,
            max_input_bytes: self.max_input_bytes,
            max_output_bytes: self.max_output_bytes,
            max_memory_bytes: self.max_memory_bytes,
            live_mem_bytes: self.live_mem_bytes,
            deadline: self.deadline,
            deadline_at: self.deadline_at,
            transfer_max: self.transfer_max,
        })
    }

    /// Materialize a thread-affine [`MediaSession`] on its owning worker thread from a [`MediaSessionPlan`].
    #[must_use]
    pub fn from_plan(plan: MediaSessionPlan) -> Self {
        let mode = match plan.mode {
            SessionModePlan::Execute { job, direct_pump_intent } => {
                SessionMode::Execute { job, direct_pump_intent, direct_pump: None }
            }
            SessionModePlan::Inspect { target_slot, expected_container, bounded_index_scan } => {
                SessionMode::Inspect { target_slot, expected_container, bounded_index_scan }
            }
            SessionModePlan::Estimate { job } => SessionMode::Estimate { job },
        };
        Self {
            engine: plan.engine,
            services: plan.services,
            stop: plan.stop,
            next_ticket: 1,
            lifecycle: Lifecycle::Running,
            mode,
            inputs: plan.inputs,
            sinks: plan.sinks,
            active_wait: ActiveWait::None,
            queued_outputs: Vec::new(),
            staged_result_json: None,
            terminal_json: None,
            max_input_bytes: plan.max_input_bytes,
            max_output_bytes: plan.max_output_bytes,
            max_memory_bytes: plan.max_memory_bytes,
            live_mem_bytes: plan.live_mem_bytes,
            deadline: plan.deadline,
            deadline_at: plan.deadline_at,
            transfer_max: plan.transfer_max,
        }
    }

    /// Return finalized UTF-8 result or structured error JSON once terminal (`Complete` or `Failed`).
    #[must_use]
    pub fn result_json(&self) -> Option<&[u8]> {
        self.terminal_json.as_deref()
    }

    /// Whether the session has reached a terminal state (`Complete` or `Failed`).
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self.lifecycle, Lifecycle::Complete | Lifecycle::Failed { .. })
    }

    fn check_deadline(&self) {
        if let Some(at) = self.deadline_at
            && Instant::now() >= at
        {
            let _ = self.stop.trip(StopCause::Deadline);
        }
    }

    /// Advance the sans-I/O state machine by up to `quota` bounded work units.
    ///
    /// Events are level-triggered and stable until their ticket resolves: calling `next`
    /// again before calling `reply` returns the exact same event without mutating state.
    pub fn next(&mut self, quota: WorkQuota) -> std::result::Result<IoEvent, SessionError> {
        // Stable terminal states always repeat identically.
        match self.lifecycle {
            Lifecycle::Complete => return Ok(IoEvent::Complete),
            Lifecycle::Failed { category, cause } => {
                return Ok(IoEvent::Failed { category, cause });
            }
            Lifecycle::Committing => {
                if self.stop.is_abandoned() {
                    self.abandon_commit();
                    return Ok(self.current_terminal_event());
                }
                // Once Commit has atomically won on JobStop, ordinary cancel/deadline cannot abort it!
                if let ActiveWait::Commit { ticket } = self.active_wait {
                    return Ok(IoEvent::Commit { ticket });
                }
            }
            Lifecycle::Running | Lifecycle::Finishing => {}
        }

        self.check_deadline();
        // Check cancellation/deadline BEFORE issuing new work (except active non-abandoned Committing).
        if self.stop.is_stopped() {
            let cause = self.stop.cause();
            self.fail_with(
                ErrorCategory::OperationCancelled,
                cause,
                format!("job stopped ({cause:?})"),
            );
            return Ok(self.current_terminal_event());
        }

        // If an I/O ticket is already outstanding, level-trigger repeat it identically.
        if let Some(ev) = self.current_wait_event() {
            return Ok(ev);
        }

        let budget = quota.0.max(1);
        for _ in 0..budget {
            self.check_deadline();
            if self.stop.is_stopped() {
                let cause = self.stop.cause();
                self.fail_with(
                    ErrorCategory::OperationCancelled,
                    cause,
                    format!("job stopped ({cause:?})"),
                );
                return Ok(self.current_terminal_event());
            }

            // 1. If we are publishing queued outputs or finishing sinks, drive publication first.
            if !self.queued_outputs.is_empty() || self.lifecycle == Lifecycle::Finishing {
                if let Some(ev) = self.step_publication()? {
                    return Ok(ev);
                }
                continue;
            }

            // 2. Direct MediaPump streaming path (true sans-I/O demux -> decode -> encode -> mux).
            let has_direct_intent = matches!(
                self.mode,
                SessionMode::Execute { direct_pump_intent: Some(_), .. }
            );
            if has_direct_intent {
                if let Some(ev) = self.step_direct_pump()? {
                    return Ok(ev);
                }
                continue;
            }

            // 3. Ensure all declared inputs have been pulled within bounded credit & caps.
            if let Some(ev) = self.step_pull_inputs()? {
                return Ok(ev);
            }

            // 4. All inputs are ready: execute the requested operation!
            self.execute_buffered_operation();
            if self.is_terminal() {
                return Ok(self.current_terminal_event());
            }
        }

        Ok(IoEvent::Yield)
    }

    /// Copy the exact pending output or patch chunk window for `ticket` into `dst` (idempotent).
    pub fn copy_pending(
        &self,
        ticket: IoTicket,
        dst: &mut [u8],
    ) -> std::result::Result<usize, CopyPendingError> {
        if self.is_terminal() {
            return Err(CopyPendingError::Closed);
        }
        let ActiveWait::Output(ref chunk) = self.active_wait else {
            return Err(CopyPendingError::StaleTicket);
        };
        if chunk.ticket != ticket {
            return Err(CopyPendingError::StaleTicket);
        }
        let end = chunk.sent_offset + chunk.window_len;
        let rem = &chunk.bytes[chunk.sent_offset..end];
        if dst.len() < rem.len() {
            return Err(CopyPendingError::BufferTooSmall(rem.len()));
        }
        dst[..rem.len()].copy_from_slice(rem);
        Ok(rem.len())
    }

    /// Deliver a host reply for the active outstanding [`IoTicket`].
    ///
    /// Invalid or stale replies return an error without mutating session progress.
    pub fn reply(
        &mut self,
        ticket: IoTicket,
        reply: Reply<'_>,
    ) -> std::result::Result<(), SessionError> {
        if self.is_terminal() {
            return Err(SessionError::Closed);
        }
        // Check stop unless we are resolving an already-authorized (non-abandoned) Commit ticket!
        if self.lifecycle == Lifecycle::Committing {
            if self.stop.is_abandoned() {
                self.abandon_commit();
                return Err(SessionError::Closed);
            }
        } else {
            self.check_deadline();
            if self.stop.is_stopped() {
                let cause = self.stop.cause();
                self.fail_with(
                    ErrorCategory::OperationCancelled,
                    cause,
                    format!("job stopped ({cause:?})"),
                );
                return Err(SessionError::Closed);
            }
        }

        let active_ticket = match &self.active_wait {
            ActiveWait::None => return Err(SessionError::StaleTicket),
            ActiveWait::Input { ticket: t, .. }
            | ActiveWait::Output(PendingChunk { ticket: t, .. })
            | ActiveWait::Flush { ticket: t, .. }
            | ActiveWait::Commit { ticket: t } => *t,
        };
        if active_ticket != ticket {
            return Err(SessionError::StaleTicket);
        }

        // Handle Aborted or IoError uniformly for any live ticket.
        match reply {
            Reply::Aborted => {
                if self.lifecycle == Lifecycle::Committing {
                    for s in &mut self.sinks {
                        if s.spec.visibility == Some(Visibility::Staged) {
                            s.exact_unknown_tail = true;
                        }
                    }
                    self.fail_with(
                        ErrorCategory::IoError,
                        StopCause::HostAbort,
                        "host aborted staged Commit transaction".to_string(),
                    );
                    return Ok(());
                }
                let _ = self.stop.trip(StopCause::HostAbort);
                self.fail_with(
                    ErrorCategory::OperationCancelled,
                    StopCause::HostAbort,
                    "host aborted streaming dialogue".to_string(),
                );
                return Ok(());
            }
            Reply::IoError { accepted_prefix, diagnostic } => {
                // Validate prefix against active chunk window BEFORE mutating state!
                if let ActiveWait::Output(ref chunk) = self.active_wait
                    && let Some(prefix) = accepted_prefix
                    && prefix > chunk.window_len as u64
                {
                    return Err(SessionError::InvalidArgument(format!(
                        "IoError accepted_prefix {} exceeds offered chunk length {}",
                        prefix, chunk.window_len
                    )));
                }

                let diag = String::from_utf8_lossy(
                    &diagnostic[..diagnostic.len().min(512)],
                )
                .into_owned();

                // Record exact partial outcome BEFORE clearing active_wait!
                let active_taken = std::mem::replace(&mut self.active_wait, ActiveWait::None);
                match active_taken {
                    ActiveWait::Output(chunk) => {
                        self.release_mem(chunk.charged_bytes);
                        let sink = &mut self.sinks[chunk.sink_idx];
                        if chunk.is_patch {
                            // Patch writes inside earlier bytes without advancing tail length;
                            // any non-zero or unknown prefix leaves exact file state indeterminate.
                            if accepted_prefix != Some(0) {
                                sink.exact_unknown_tail = true;
                            }
                        } else if let Some(prefix) = accepted_prefix {
                            // Append prefix is relative to the current credit window!
                            sink.accepted_bytes = sink.accepted_bytes.saturating_add(prefix);
                        } else {
                            sink.exact_unknown_tail = true;
                        }
                    }
                    ActiveWait::Flush { sink_idx, .. } => {
                        if self.sinks[sink_idx].spec.visibility == Some(Visibility::Incremental) {
                            self.sinks[sink_idx].exact_unknown_tail = true;
                        }
                    }
                    ActiveWait::Commit { .. } => {
                        for s in &mut self.sinks {
                            if s.spec.visibility == Some(Visibility::Staged) {
                                s.exact_unknown_tail = true;
                            }
                        }
                    }
                    ActiveWait::Input { .. } | ActiveWait::None => {}
                }
                self.fail_with(
                    ErrorCategory::IoError,
                    StopCause::None,
                    if diag.is_empty() { "host I/O error".to_string() } else { diag },
                );
                return Ok(());
            }
            _ => {}
        }

        // Dispatch by active wait kind (validating BEFORE taking ownership of active_wait so
        // invalid replies leave state 100% untouched!).
        match &self.active_wait {
            ActiveWait::Input { input_idx, request, pump_ticket, .. } => {
                let input_idx = *input_idx;
                let request = *request;
                let pump_ticket = *pump_ticket;
                self.handle_input_reply(input_idx, request, pump_ticket, reply)
            }
            ActiveWait::Output(chunk) => {
                let Reply::Ack { bytes } = reply else {
                    return Err(SessionError::InvalidArgument(
                        "Output/Patch ticket requires Reply::Ack".to_string(),
                    ));
                };
                if bytes == 0 || bytes > chunk.window_len as u64 {
                    return Err(SessionError::InvalidArgument(format!(
                        "Output/Patch Ack bytes {} out of range 1..={}",
                        bytes, chunk.window_len
                    )));
                }
                let acked = bytes as usize;
                let mut taken = match std::mem::replace(&mut self.active_wait, ActiveWait::None) {
                    ActiveWait::Output(c) => c,
                    _ => unreachable!(),
                };
                let sink = &mut self.sinks[taken.sink_idx];
                if !taken.is_patch {
                    sink.accepted_bytes = sink.accepted_bytes.saturating_add(bytes);
                }
                taken.sent_offset += acked;
                taken.offset = taken.offset.saturating_add(bytes);
                taken.pump_total_acked += acked;

                if taken.sent_offset < taken.bytes.len() {
                    // Partial ACK or next credit window: fresh monotonic ticket!
                    let next_t = self.alloc_ticket()?;
                    taken.ticket = next_t;
                    taken.window_len =
                        (taken.bytes.len() - taken.sent_offset).min(self.transfer_max as usize);
                    self.active_wait = ActiveWait::Output(taken);
                    return Ok(());
                }

                // Full chunk acknowledged: release charged buffer memory and notify MediaPump!
                self.release_mem(taken.charged_bytes);
                if let Some(pt) = taken.pump_ticket
                    && let SessionMode::Execute { direct_pump: Some(ref mut drv), .. } = self.mode
                {
                    if !taken.is_patch {
                        drv.out_offset = drv.out_offset.saturating_add(taken.pump_total_acked as u64);
                        drv.out_total_bytes = drv.out_total_bytes.max(drv.out_offset);
                    }
                    if let Err(e) = drv.pump.confirm_written(pt, taken.pump_total_acked) {
                        let fe = crate::media::map_engine_error(e);
                        self.fail_with(fe.category(), StopCause::None, fe.message);
                    }
                }
                Ok(())
            }
            ActiveWait::Flush { sink_idx, .. } => {
                let Reply::Ack { bytes } = reply else {
                    return Err(SessionError::InvalidArgument(
                        "Flush ticket requires Reply::Ack { bytes: 0 }".to_string(),
                    ));
                };
                if bytes != 0 {
                    return Err(SessionError::InvalidArgument(
                        "Flush Ack bytes must be 0".to_string(),
                    ));
                }
                let idx = *sink_idx;
                self.active_wait = ActiveWait::None;
                self.sinks[idx].flushed = true;
                Ok(())
            }
            ActiveWait::Commit { .. } => {
                let Reply::Ack { bytes } = reply else {
                    return Err(SessionError::InvalidArgument(
                        "Commit ticket requires Reply::Ack { bytes: 0 }".to_string(),
                    ));
                };
                if bytes != 0 {
                    return Err(SessionError::InvalidArgument(
                        "Commit Ack bytes must be 0".to_string(),
                    ));
                }
                self.active_wait = ActiveWait::None;
                for s in &mut self.sinks {
                    if s.spec.visibility == Some(Visibility::Staged) {
                        s.committed = true;
                    }
                }
                self.terminal_json = self.staged_result_json.take();
                self.lifecycle = Lifecycle::Complete;
                Ok(())
            }
            ActiveWait::None => Err(SessionError::StaleTicket),
        }
    }

    fn handle_input_reply(
        &mut self,
        input_idx: usize,
        request: IoInput,
        pump_ticket: Option<PumpTicket>,
        reply: Reply<'_>,
    ) -> std::result::Result<(), SessionError> {
        match request {
            IoInput::Len => match reply {
                Reply::Length(len) => {
                    if len > self.max_input_bytes {
                        self.active_wait = ActiveWait::None;
                        self.fail_with(
                            ErrorCategory::ArgumentInvalid,
                            StopCause::None,
                            format!(
                                "source length {} exceeds max_input_bytes {}",
                                len, self.max_input_bytes
                            ),
                        );
                        return Ok(());
                    }
                    self.active_wait = ActiveWait::None;
                    self.inputs[input_idx].known_len = Some(len);
                    self.inputs[input_idx].len_queried = true;
                    if let Some(pt) = pump_ticket
                        && let SessionMode::Execute { direct_pump: Some(ref mut drv), .. } = self.mode
                    {
                        drv.in_known_len = Some(len);
                        if let Err(e) = drv.pump.supply_input(pt, Input::new(0, &[]).with_total_len(len)) {
                            let fe = crate::media::map_engine_error(e);
                            self.fail_with(fe.category(), StopCause::None, fe.message);
                        }
                    }
                    Ok(())
                }
                Reply::UnknownLength => {
                    self.active_wait = ActiveWait::None;
                    self.inputs[input_idx].known_len = None;
                    self.inputs[input_idx].len_queried = true;
                    if let Some(pt) = pump_ticket
                        && let SessionMode::Execute { direct_pump: Some(ref mut drv), .. } = self.mode
                    {
                        if let Err(e) = drv.pump.supply_input(pt, Input::new(0, &[])) {
                            let fe = crate::media::map_engine_error(e);
                            self.fail_with(fe.category(), StopCause::None, fe.message);
                        }
                    }
                    Ok(())
                }
                _ => Err(SessionError::InvalidArgument(
                    "NeedLen ticket requires Reply::Length or Reply::UnknownLength".to_string(),
                )),
            },
            IoInput::AtLeast { next_offset, max_transfer, .. } => match reply {
                Reply::Bytes(chunk) => {
                    if chunk.is_empty() || chunk.len() > max_transfer as usize {
                        return Err(SessionError::InvalidArgument(format!(
                            "Bytes reply length {} out of range 1..={}",
                            chunk.len(),
                            max_transfer
                        )));
                    }
                    let new_end = next_offset
                        .checked_add(chunk.len() as u64)
                        .ok_or_else(|| SessionError::ResourceLimit("input offset overflow".into()))?;
                    if new_end > self.max_input_bytes {
                        self.active_wait = ActiveWait::None;
                        self.fail_with(
                            ErrorCategory::ArgumentInvalid,
                            StopCause::None,
                            format!(
                                "input bytes {} exceed max_input_bytes {}",
                                new_end, self.max_input_bytes
                            ),
                        );
                        return Ok(());
                    }
                    if pump_ticket.is_some() {
                        self.active_wait = ActiveWait::None;
                        if let Some(pt) = pump_ticket
                            && let SessionMode::Execute { direct_pump: Some(ref mut drv), .. } =
                                self.mode
                        {
                            let inp = Input::new(next_offset, chunk)
                                .with_total_len_opt(drv.in_known_len);
                            if let Err(e) = drv.pump.supply_input(pt, inp) {
                                let fe = crate::media::map_engine_error(e);
                                self.fail_with(fe.category(), StopCause::None, fe.message);
                            }
                        }
                    } else {
                        // Reserve exact bounded capacity BEFORE mutating `inp.data` so Vec
                        // doubling can never overshoot `max_memory_bytes`!
                        let cur_len = self.inputs[input_idx].data.len();
                        let cur_cap = self.inputs[input_idx].data.capacity();
                        let prev_charged = self.inputs[input_idx].charged_cap;
                        let needed_len = cur_len.checked_add(chunk.len()).ok_or_else(|| {
                            SessionError::ResourceLimit("input buffer length overflow".into())
                        })?;
                        if needed_len > cur_cap {
                            let base_live = self.live_mem_bytes.saturating_sub(prev_charged);
                            let target_cap = if let Some(cap) = self.max_memory_bytes {
                                let headroom = cap.saturating_sub(base_live);
                                if needed_len as u64 > headroom {
                                    self.active_wait = ActiveWait::None;
                                    self.fail_with(
                                        ErrorCategory::OutOfMemory,
                                        StopCause::None,
                                        format!(
                                            "streaming live memory {} B exceeds max_memory_bytes {}",
                                            base_live.saturating_add(needed_len as u64),
                                            cap
                                        ),
                                    );
                                    return Err(SessionError::ResourceLimit(
                                        "streaming live memory exceeds max_memory_bytes".into(),
                                    ));
                                }
                                let headroom_u = usize::try_from(headroom).unwrap_or(usize::MAX);
                                cur_cap.saturating_mul(2).max(needed_len).min(headroom_u)
                            } else {
                                cur_cap.saturating_mul(2).max(needed_len)
                            };
                            if self.inputs[input_idx]
                                .data
                                .try_reserve_exact(target_cap - cur_len)
                                .is_err()
                            {
                                self.active_wait = ActiveWait::None;
                                self.fail_with(
                                    ErrorCategory::OutOfMemory,
                                    StopCause::None,
                                    "failed to allocate bounded input buffer".to_string(),
                                );
                                return Err(SessionError::ResourceLimit(
                                    "input buffer allocation failed".into(),
                                ));
                            }
                            let new_cap = self.inputs[input_idx].data.capacity() as u64;
                            let delta = new_cap.saturating_sub(prev_charged);
                            if delta > 0 {
                                self.charge_mem(delta)?;
                                self.inputs[input_idx].charged_cap = new_cap;
                            }
                        }
                        self.active_wait = ActiveWait::None;
                        let inp = &mut self.inputs[input_idx];
                        inp.data.extend_from_slice(chunk);
                        inp.consumed_bytes = inp.data.len() as u64;
                        if let Some(tot) = inp.known_len
                            && inp.consumed_bytes >= tot
                        {
                            inp.eof = true;
                        }
                    }
                    Ok(())
                }
                Reply::End { total_len } => {
                    self.active_wait = ActiveWait::None;
                    if let Some(pt) = pump_ticket {
                        if let SessionMode::Execute { direct_pump: Some(ref mut drv), .. } = self.mode {
                            let tot = total_len.or(drv.in_known_len).unwrap_or(next_offset);
                            drv.in_eof = true;
                            drv.in_known_len = Some(tot);
                            let inp = Input::new(next_offset, &[]).with_total_len(tot);
                            if let Err(e) = drv.pump.supply_input(pt, inp) {
                                let fe = crate::media::map_engine_error(e);
                                self.fail_with(fe.category(), StopCause::None, fe.message);
                            }
                        }
                    } else {
                        let inp = &mut self.inputs[input_idx];
                        let observed = inp.data.len() as u64;
                        if let Some(tot) = total_len
                            && tot != observed
                        {
                            self.fail_with(
                                ErrorCategory::IoError,
                                StopCause::None,
                                format!(
                                    "source EOF total_len {} disagrees with received {} bytes",
                                    tot, observed
                                ),
                            );
                            return Ok(());
                        }
                        inp.known_len = Some(observed);
                        inp.eof = true;
                    }
                    Ok(())
                }
                _ => Err(SessionError::InvalidArgument(
                    "NeedAtLeast ticket requires Reply::Bytes or Reply::End".to_string(),
                )),
            },
            IoInput::Range { offset, len } => match reply {
                Reply::Bytes(chunk) => {
                    if chunk.is_empty() || chunk.len() > len as usize {
                        return Err(SessionError::InvalidArgument(format!(
                            "Range Bytes length {} out of range 1..={}",
                            chunk.len(),
                            len
                        )));
                    }
                    let new_end = offset
                        .checked_add(chunk.len() as u64)
                        .ok_or_else(|| SessionError::ResourceLimit("range offset overflow".into()))?;
                    if new_end > self.max_input_bytes {
                        self.active_wait = ActiveWait::None;
                        self.release_pending_range_mem();
                        self.fail_with(
                            ErrorCategory::ArgumentInvalid,
                            StopCause::None,
                            "range read exceeds max_input_bytes".to_string(),
                        );
                        return Ok(());
                    }
                    self.active_wait = ActiveWait::None;

                    // Aggregate short or multi-credit range replies until the EXACT requested
                    // `MediaPump` range length is satisfied!
                    let mut next_range_wait: Option<(u64, u32, PumpTicket)> = None;
                    let mut release_charge: u64 = 0;
                    let mut pump_err: Option<FlowError> = None;

                    if let SessionMode::Execute { direct_pump: Some(ref mut drv), .. } = self.mode {
                        if let Some(ref mut acc) = drv.pending_range {
                            acc.buf.extend_from_slice(chunk);
                            if acc.buf.len() < acc.target_len {
                                let rem = acc.target_len - acc.buf.len();
                                let req_len = u32::try_from(rem)
                                    .unwrap_or(self.transfer_max)
                                    .min(self.transfer_max)
                                    .max(1);
                                let next_off = acc.base_offset.saturating_add(acc.buf.len() as u64);
                                next_range_wait = Some((next_off, req_len, acc.pump_ticket));
                            } else {
                                let done = drv.pending_range.take().expect("checked Some");
                                release_charge = done.charged_bytes;
                                let inp = Input::new(done.base_offset, &done.buf)
                                    .with_total_len_opt(drv.in_known_len);
                                if let Err(e) = drv.pump.supply_input(done.pump_ticket, inp) {
                                    pump_err = Some(crate::media::map_engine_error(e));
                                }
                            }
                        } else if let Some(pt) = pump_ticket {
                            let inp = Input::new(offset, chunk).with_total_len_opt(drv.in_known_len);
                            if let Err(e) = drv.pump.supply_input(pt, inp) {
                                pump_err = Some(crate::media::map_engine_error(e));
                            }
                        }
                    }

                    if release_charge > 0 {
                        self.release_mem(release_charge);
                    }
                    if let Some(fe) = pump_err {
                        self.fail_with(fe.category(), StopCause::None, fe.message);
                        return Ok(());
                    }
                    if let Some((next_off, req_len, pt)) = next_range_wait {
                        let ticket = self.alloc_ticket()?;
                        self.active_wait = ActiveWait::Input {
                            ticket,
                            input_idx,
                            request: IoInput::Range { offset: next_off, len: req_len },
                            pump_ticket: Some(pt),
                        };
                    }
                    Ok(())
                }
                Reply::End { total_len } => {
                    self.active_wait = ActiveWait::None;
                    let mut release_charge: u64 = 0;
                    let mut pump_err: Option<FlowError> = None;

                    if let SessionMode::Execute { direct_pump: Some(ref mut drv), .. } = self.mode {
                        if let Some(done) = drv.pending_range.take() {
                            release_charge = done.charged_bytes;
                            let eof_off = done.base_offset.saturating_add(done.buf.len() as u64);
                            let tot = total_len.or(drv.in_known_len).unwrap_or(eof_off);
                            drv.in_known_len = Some(tot);
                            // Supply truncated slice (or empty EOF) so MediaPump reports exact InvalidData.
                            let inp = Input::new(done.base_offset, &done.buf).with_total_len(tot);
                            if let Err(e) = drv.pump.supply_input(done.pump_ticket, inp) {
                                pump_err = Some(crate::media::map_engine_error(e));
                            }
                        } else if let Some(pt) = pump_ticket {
                            let tot = total_len.or(drv.in_known_len).unwrap_or(offset);
                            drv.in_known_len = Some(tot);
                            if let Err(e) =
                                drv.pump.supply_input(pt, Input::new(offset, &[]).with_total_len(tot))
                            {
                                pump_err = Some(crate::media::map_engine_error(e));
                            }
                        }
                    }
                    if release_charge > 0 {
                        self.release_mem(release_charge);
                    }
                    if let Some(fe) = pump_err {
                        self.fail_with(fe.category(), StopCause::None, fe.message);
                    }
                    Ok(())
                }
                _ => Err(SessionError::InvalidArgument(
                    "NeedRange ticket requires Reply::Bytes or Reply::End".to_string(),
                )),
            },
        }
    }

    fn step_direct_pump(&mut self) -> std::result::Result<Option<IoEvent>, SessionError> {
        // Phase A: Probe initial prefix if pump not opened yet.
        let need_open = matches!(
            self.mode,
            SessionMode::Execute { direct_pump: None, .. }
        );
        if need_open {
            // Query known length first whenever source advertises known_length (ReadAt OR Forward!).
            if self.inputs[0].spec.access.has_known_length() && !self.inputs[0].len_queried {
                let ticket = self.alloc_ticket()?;
                self.active_wait = ActiveWait::Input {
                    ticket,
                    input_idx: 0,
                    request: IoInput::Len,
                    pump_ticket: None,
                };
                return Ok(self.current_wait_event());
            }

            // Pull at least 64 bytes (or EOF) to probe container prefix.
            let probe_limits = ProbeLimits::default();
            let outcome = if self.inputs[0].data.is_empty() && !self.inputs[0].eof {
                ContainerProbeOutcome::NeedMore { min_bytes: 64 }
            } else {
                match probe_container_prefix(
                    self.engine.codecs(),
                    &self.inputs[0].data,
                    &probe_limits,
                ) {
                    Ok(o) => o,
                    Err(e) => {
                        let fe = crate::media::map_engine_error(e);
                        self.fail_with(fe.category(), StopCause::None, fe.message);
                        return Ok(Some(self.current_terminal_event()));
                    }
                }
            };

            match outcome {
                ContainerProbeOutcome::NeedMore { min_bytes } => {
                    if self.inputs[0].eof || self.inputs[0].data.len() >= MAX_PROBE_PREFIX {
                        // Fall back to unified DAG execution (e.g. still/animation -> video!).
                        if let SessionMode::Execute { ref mut direct_pump_intent, .. } = self.mode {
                            *direct_pump_intent = None;
                        }
                        return Ok(None);
                    }
                    let next_offset = self.inputs[0].data.len() as u64;
                    let remaining_cap = self.max_input_bytes.saturating_sub(next_offset);
                    if remaining_cap == 0 {
                        self.fail_with(
                            ErrorCategory::ArgumentInvalid,
                            StopCause::None,
                            format!(
                                "input slot '{}' reached max_input_bytes {} during probe",
                                self.inputs[0].slot, self.max_input_bytes
                            ),
                        );
                        return Ok(Some(self.current_terminal_event()));
                    }
                    let want = (min_bytes.max(64).saturating_sub(self.inputs[0].data.len())).max(1);
                    let max_transfer = self
                        .transfer_max
                        .min(u32::try_from(remaining_cap).unwrap_or(u32::MAX))
                        .max(1);
                    let ticket = self.alloc_ticket()?;
                    self.active_wait = ActiveWait::Input {
                        ticket,
                        input_idx: 0,
                        request: IoInput::AtLeast {
                            next_offset,
                            min_remaining: want as u64,
                            max_transfer,
                        },
                        pump_ticket: None,
                    };
                    return Ok(self.current_wait_event());
                }
                ContainerProbeOutcome::NoMatch => {
                    // Not a video container (e.g. animated GIF/PNG/WebP -> video): use unified DAG path!
                    if let SessionMode::Execute { ref mut direct_pump_intent, .. } = self.mode {
                        *direct_pump_intent = None;
                    }
                    return Ok(None);
                }
                ContainerProbeOutcome::Confirmed { container, .. } => {
                    let source_access = match self.inputs[0].spec.access {
                        BindingAccess::ForwardSource { .. } => SourceAccess::forward_stream(),
                        BindingAccess::ReadAtSource { .. } => {
                            SourceAccess::positional(self.inputs[0].known_len)
                        }
                        _ => SourceAccess::forward_stream(),
                    };
                    // Preserve the exact parsed OutputIntent publication mode (never overwrite!).
                    let intent = match &self.mode {
                        SessionMode::Execute { direct_pump_intent: Some(i), .. } => i.clone(),
                        _ => unreachable!(),
                    };
                    let (target_container, target_codec) = match &intent.target {
                        TargetFormat::Video { container: tc, codec: vc } => {
                            (tc.clone(), vc.clone())
                        }
                        _ => unreachable!(),
                    };

                    // Single unified memory ceiling reservation FIRST so both upfront admission
                    // and `MediaPump::prepare_transcode` quote against the EXACT same `pump_services`!
                    let mut pump_services = self.services_with_remaining_budget();
                    let pump_reserved_bytes = if let Some(cap) = self.max_memory_bytes {
                        let rem = cap.saturating_sub(self.live_mem_bytes);
                        let io_headroom = (rem / 4)
                            .min(2 * 1024 * 1024)
                            .max(u64::from(self.transfer_max).saturating_mul(2))
                            .min(rem / 2);
                        let pump_budget = rem.saturating_sub(io_headroom);
                        let mut lim = pump_services.limits.unwrap_or_default();
                        lim = lim.with_max_memory(
                            lim.max_memory_bytes.map_or(pump_budget, |c| c.min(pump_budget)),
                        );
                        pump_services.limits = Some(lim);
                        if let Err(SessionError::ResourceLimit(_)) = self.charge_mem(pump_budget) {
                            return Ok(Some(self.current_terminal_event()));
                        }
                        pump_budget
                    } else {
                        0
                    };

                    // Upfront route-accurate sink patch-compatibility admission: ONLY quote when
                    // the `intent.tracks`-selected input video track descriptor is already
                    // parseable from the prefix (NEVER synthesize a fake 64×64 @ 30 Hz track or
                    // assume video track 0!) and ALWAYS quote with `pump_services` (matching the
                    // exact workspace split passed to MediaPump!). When prefix headers are not
                    // yet complete or multi-track selection cannot be resolved upfront, MediaPump
                    // itself runs `select_muxer(..., Incremental)` under `pump_services` once real
                    // headers arrive, before emitting any byte!
                    if self.sinks[0].spec.access == BindingAccess::ForwardSink
                        && matches!(intent.publication, PublicationMode::Incremental)
                        && let Some((route_dims, route_tb)) = probe_selected_video_track_descriptor(
                            &self.engine,
                            &pump_services,
                            &self.inputs[0].data,
                            &intent.tracks,
                            Some(self.deadline_pulse()),
                        )
                    {
                        let fwd_check = select_mux_offer_for_route(
                            &self.engine,
                            &pump_services,
                            &target_container,
                            &target_codec,
                            PublicationMode::Incremental,
                            &intent.sequence,
                            route_dims,
                            route_tb,
                        );
                        let forward_non_patching = fwd_check.as_ref().is_ok_and(|(_, off)| {
                            off.timing.streams() && !off.timing.patches_at_finish()
                        });
                        if !forward_non_patching {
                            if pump_reserved_bytes > 0 {
                                self.release_mem(pump_reserved_bytes);
                            }
                            let detail = match fwd_check {
                                Err(reason) => format!(" ({reason})"),
                                Ok(_) => String::new(),
                            };
                            self.fail_with(
                                ErrorCategory::ActionNotSupported,
                                StopCause::None,
                                format!(
                                    "incremental output for '{}' requires a patch-capable or staged sink{}",
                                    crate::media::report::container_token(&target_container),
                                    detail
                                ),
                            );
                            return Ok(Some(self.current_terminal_event()));
                        }
                    }

                    let mut run = Run::new()
                        .with_workspace_policy(pump_services.workspace_policy)
                        .with_pulse(self.deadline_pulse());
                    if let Some(lim) = pump_services.limits {
                        run = run.with_limits(lim);
                    }
                    if let Some(ref cs) = pump_services.color_services {
                        run = run.with_color_services(cs.clone());
                    }
                    if let Some(ref routing) = pump_services.routing {
                        run = run.with_routing(routing);
                    }

                    let pump = match MediaPump::prepare_transcode(
                        (*self.engine).clone(),
                        container,
                        &source_access,
                        intent,
                        &run,
                    ) {
                        Ok(p) => p,
                        Err(e) => {
                            if pump_reserved_bytes > 0 {
                                self.release_mem(pump_reserved_bytes);
                            }
                            let fe = crate::media::map_engine_error(e);
                            self.fail_with(fe.category(), self.stop.cause(), fe.message);
                            return Ok(Some(self.current_terminal_event()));
                        }
                    };

                    let prefix_bytes = std::mem::take(&mut self.inputs[0].data);
                    let prefix_charged = std::mem::take(&mut self.inputs[0].charged_cap);
                    let in_eof = self.inputs[0].eof;
                    let in_known_len = self.inputs[0].known_len;
                    let output_slot = self.sinks[0].slot.clone();
                    let drv = DirectPumpDriver {
                        pump,
                        pump_reserved_bytes,
                        out_offset: 0,
                        out_total_bytes: 0,
                        in_buf: prefix_bytes,
                        in_buf_charged: prefix_charged,
                        in_eof,
                        in_known_len,
                        pending_range: None,
                        out_header: Vec::new(),
                        out_header_charged: 0,
                        finished: false,
                        target_container,
                        target_codec,
                        output_slot,
                    };
                    if let SessionMode::Execute { ref mut direct_pump, .. } = self.mode {
                        *direct_pump = Some(drv);
                    }
                }
            }
        }

        // Phase B: Advance active MediaPump!
        let step_outcome = {
            let is_range_capable =
                matches!(self.inputs[0].spec.access, BindingAccess::ReadAtSource { .. });
            let has_known_len = self.inputs[0].spec.access.has_known_length();
            let allow_patch = self.sinks[0].spec.access == BindingAccess::PatchSink;
            let max_out = self.max_output_bytes;
            let max_mem = self.max_memory_bytes;
            let live_mem = self.live_mem_bytes;
            let SessionMode::Execute { direct_pump: Some(ref mut drv), .. } = self.mode else {
                unreachable!();
            };
            match drv.pump.advance() {
                Ok(PumpEvent::Presented) => DirectStep::Progress,
                Ok(PumpEvent::Finished) => {
                    drv.finished = true;
                    DirectStep::Finished
                }
                Ok(PumpEvent::NeedInput { ticket, requests }) => {
                    let chosen = pick_compatible_pump_request(
                        requests,
                        is_range_capable,
                        has_known_len,
                    );
                    DirectStep::NeedInput(ticket, chosen)
                }
                Ok(PumpEvent::NeedWriter { ticket, write }) => {
                    let (offset, is_patch, bytes): (u64, bool, &[u8]) = match write {
                        WriteOffer::Append(b) => (drv.out_offset, false, b),
                        WriteOffer::Patch { offset, bytes } => (offset, true, bytes),
                    };
                    // Validate patch support, output cap, and unified memory ceiling BEFORE
                    // allocating or cloning any output bytes!
                    if is_patch && !allow_patch {
                        DirectStep::Fail(
                            ErrorCategory::ActionNotSupported,
                            "muxer emitted seek-back Patch on forward-only sink".to_string(),
                        )
                    } else if let Some(cap_out) = max_out
                        && offset.saturating_add(bytes.len() as u64) > cap_out
                    {
                        DirectStep::Fail(
                            ErrorCategory::ArgumentInvalid,
                            format!(
                                "output bytes {} exceed max_output_bytes {}",
                                offset.saturating_add(bytes.len() as u64),
                                cap_out
                            ),
                        )
                    } else {
                        // Release retained probe prefix once pump is emitting output on random-access
                        // sources (a forward-only source cannot re-read unread prefix tail bytes).
                        let freed_prefix = if is_range_capable && drv.in_buf_charged > 0 {
                            drv.in_buf.clear();
                            drv.in_buf.shrink_to_fit();
                            std::mem::take(&mut drv.in_buf_charged)
                        } else {
                            0
                        };
                        let live_after_free = live_mem.saturating_sub(freed_prefix);
                        if let Some(cap) = max_mem
                            && live_after_free.saturating_add(bytes.len() as u64) > cap
                        {
                            DirectStep::FailWithFree {
                                category: ErrorCategory::OutOfMemory,
                                message: format!(
                                    "streaming live memory {} B exceeds max_memory_bytes {}",
                                    live_after_free.saturating_add(bytes.len() as u64),
                                    cap
                                ),
                                freed_prefix,
                            }
                        } else {
                            let mut owned = Vec::new();
                            if owned.try_reserve_exact(bytes.len()).is_err() {
                                DirectStep::FailWithFree {
                                    category: ErrorCategory::OutOfMemory,
                                    message: "failed to allocate bounded output chunk".to_string(),
                                    freed_prefix,
                                }
                            } else {
                                let owned_cap = owned.capacity() as u64;
                                if let Some(cap) = max_mem
                                    && live_after_free.saturating_add(owned_cap) > cap
                                {
                                    drop(owned);
                                    DirectStep::FailWithFree {
                                        category: ErrorCategory::OutOfMemory,
                                        message: format!(
                                            "streaming live memory {} B exceeds max_memory_bytes {}",
                                            live_after_free.saturating_add(owned_cap),
                                            cap
                                        ),
                                        freed_prefix,
                                    }
                                } else {
                                    owned.extend_from_slice(bytes);
                                    // Accumulate contiguous output header prefix (`0..<=64 KiB`)
                                    // across split WriteOffer chunks and finish patches within the
                                    // unified memory ceiling so geometry is inspected ONCE after
                                    // MediaPump drops (zero overlapping demuxer!).
                                    let mut header_extra_charge = 0u64;
                                    if !is_patch
                                        && offset <= drv.out_header.len() as u64
                                        && (offset as usize) < OUT_HEADER_PROBE_CAP
                                    {
                                        let start = offset as usize;
                                        let copy_len =
                                            bytes.len().min(OUT_HEADER_PROBE_CAP - start);
                                        let end = start + copy_len;
                                        if end > drv.out_header.len() {
                                            let prev_cap = drv.out_header.capacity();
                                            let est_extra = end.saturating_sub(prev_cap) as u64;
                                            let projected = live_after_free
                                                .saturating_add(owned_cap)
                                                .saturating_add(est_extra);
                                            if max_mem.is_none_or(|cap| projected <= cap)
                                                && drv
                                                    .out_header
                                                    .try_reserve_exact(end - drv.out_header.len())
                                                    .is_ok()
                                            {
                                                let new_cap = drv.out_header.capacity() as u64;
                                                let actual_extra =
                                                    new_cap.saturating_sub(drv.out_header_charged);
                                                if max_mem.is_none_or(|cap| {
                                                    live_after_free
                                                        .saturating_add(owned_cap)
                                                        .saturating_add(actual_extra)
                                                        <= cap
                                                }) {
                                                    drv.out_header_charged = new_cap;
                                                    header_extra_charge = actual_extra;
                                                    if start < drv.out_header.len() {
                                                        let overlap = drv.out_header.len() - start;
                                                        drv.out_header[start..]
                                                            .copy_from_slice(&bytes[..overlap]);
                                                        drv.out_header.extend_from_slice(
                                                            &bytes[overlap..copy_len],
                                                        );
                                                    } else {
                                                        drv.out_header
                                                            .extend_from_slice(&bytes[..copy_len]);
                                                    }
                                                }
                                            }
                                        } else {
                                            drv.out_header[start..end]
                                                .copy_from_slice(&bytes[..copy_len]);
                                        }
                                    } else if is_patch
                                        && offset < drv.out_header.len() as u64
                                    {
                                        let start = offset as usize;
                                        let patch_len =
                                            bytes.len().min(drv.out_header.len() - start);
                                        drv.out_header[start..start + patch_len]
                                            .copy_from_slice(&bytes[..patch_len]);
                                    }
                                    DirectStep::Write {
                                        pump_ticket: ticket,
                                        offset,
                                        is_patch,
                                        bytes: owned,
                                        charged_bytes: owned_cap,
                                        freed_prefix,
                                        header_extra_charge,
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => DirectStep::Error(crate::media::map_engine_error(e)),
            }
        };

        match step_outcome {
            DirectStep::Progress => Ok(None),
            DirectStep::Error(fe) => {
                self.check_deadline();
                self.fail_with(fe.category(), self.stop.cause(), fe.message);
                Ok(Some(self.current_terminal_event()))
            }
            DirectStep::Fail(category, message) => {
                self.fail_with(category, StopCause::None, message);
                Ok(Some(self.current_terminal_event()))
            }
            DirectStep::FailWithFree { category, message, freed_prefix } => {
                if freed_prefix > 0 {
                    self.release_mem(freed_prefix);
                }
                self.fail_with(category, StopCause::None, message);
                Ok(Some(self.current_terminal_event()))
            }
            DirectStep::NeedInput(pt, req) => {
                // First try to satisfy directly from buffered probe prefix via disjoint field
                // borrow (ZERO `.to_vec()` clone allocation!).
                let served_from_prefix = {
                    let SessionMode::Execute { direct_pump: Some(ref mut drv), .. } = self.mode else {
                        unreachable!();
                    };
                    let in_buf = drv.in_buf.as_slice();
                    let in_eof = drv.in_eof;
                    let in_known_len = drv.in_known_len;
                    let eof_total_len = in_known_len.unwrap_or(in_buf.len() as u64);
                    let pump = &mut drv.pump;
                    match req {
                        InputRequest::AtLeast { offset, min_len: _ } => {
                            let off_u = usize::try_from(offset).unwrap_or(usize::MAX);
                            if off_u < in_buf.len() {
                                let tot = if in_eof {
                                    Some(eof_total_len)
                                } else {
                                    in_known_len
                                };
                                let inp = Input::new(offset, &in_buf[off_u..]).with_total_len_opt(tot);
                                Some(pump.supply_input(pt, inp))
                            } else if in_eof && offset >= eof_total_len {
                                let inp = Input::new(offset, &[]).with_total_len(eof_total_len);
                                Some(pump.supply_input(pt, inp))
                            } else {
                                None
                            }
                        }
                        InputRequest::Range(r) => {
                            let start = usize::try_from(r.offset).unwrap_or(usize::MAX);
                            let need = usize::try_from(r.length).unwrap_or(usize::MAX);
                            let end = start.saturating_add(need);
                            if start < in_buf.len() && end <= in_buf.len() {
                                let tot = if in_eof {
                                    Some(eof_total_len)
                                } else {
                                    in_known_len
                                };
                                let inp =
                                    Input::new(r.offset, &in_buf[start..end]).with_total_len_opt(tot);
                                Some(pump.supply_input(pt, inp))
                            } else if in_eof
                                && start < in_buf.len()
                                && in_buf.len() as u64 == eof_total_len
                            {
                                let inp = Input::new(r.offset, &in_buf[start..])
                                    .with_total_len(eof_total_len);
                                Some(pump.supply_input(pt, inp))
                            } else if in_eof && r.offset >= eof_total_len {
                                let inp = Input::new(r.offset, &[]).with_total_len(eof_total_len);
                                Some(pump.supply_input(pt, inp))
                            } else {
                                None
                            }
                        }
                        InputRequest::Len => in_known_len.map(|tot| {
                            pump.supply_input(pt, Input::new(0, &[]).with_total_len(tot))
                        }),
                        _ => None,
                    }
                };
                if let Some(res) = served_from_prefix {
                    if let Err(e) = res {
                        let fe = crate::media::map_engine_error(e);
                        self.fail_with(fe.category(), StopCause::None, fe.message);
                        return Ok(Some(self.current_terminal_event()));
                    }
                    return Ok(None);
                }

                // Emit bounded host IoEvent::NeedInput!
                let io_req = match req {
                    InputRequest::AtLeast { offset, min_len } => {
                        if offset > self.max_input_bytes {
                            self.fail_with(
                                ErrorCategory::ArgumentInvalid,
                                StopCause::None,
                                format!(
                                    "requested offset {} exceeds max_input_bytes {}",
                                    offset, self.max_input_bytes
                                ),
                            );
                            return Ok(Some(self.current_terminal_event()));
                        }
                        let rem_cap = self.max_input_bytes.saturating_sub(offset);
                        let max_transfer = if rem_cap == 0 {
                            self.transfer_max
                        } else {
                            self.transfer_max
                                .min(u32::try_from(rem_cap).unwrap_or(u32::MAX))
                                .max(1)
                        };
                        IoInput::AtLeast {
                            next_offset: offset,
                            min_remaining: min_len as u64,
                            max_transfer,
                        }
                    }
                    InputRequest::Range(r) => {
                        if !matches!(self.inputs[0].spec.access, BindingAccess::ReadAtSource { .. })
                        {
                            self.fail_with(
                                ErrorCategory::ActionNotSupported,
                                StopCause::None,
                                "demuxer requested random-access Range on forward-only source"
                                    .to_string(),
                            );
                            return Ok(Some(self.current_terminal_event()));
                        }
                        let end = r.offset.saturating_add(r.length);
                        if end > self.max_input_bytes {
                            self.fail_with(
                                ErrorCategory::ArgumentInvalid,
                                StopCause::None,
                                format!(
                                    "requested Range {}..{} exceeds max_input_bytes {}",
                                    r.offset, end, self.max_input_bytes
                                ),
                            );
                            return Ok(Some(self.current_terminal_event()));
                        }
                        let target_len = usize::try_from(r.length).map_err(|_| {
                            SessionError::ResourceLimit("range length exceeds address space".into())
                        })?;
                        if target_len == 0 {
                            let res = {
                                let SessionMode::Execute { direct_pump: Some(ref mut drv), .. } =
                                    self.mode
                                else {
                                    unreachable!();
                                };
                                drv.pump.supply_input(
                                    pt,
                                    Input::new(r.offset, &[]).with_total_len_opt(drv.in_known_len),
                                )
                            };
                            if let Err(e) = res {
                                let fe = crate::media::map_engine_error(e);
                                self.fail_with(fe.category(), StopCause::None, fe.message);
                                return Ok(Some(self.current_terminal_event()));
                            }
                            return Ok(None);
                        }
                        // Charge range accumulator against unified live memory cap (which ALREADY
                        // includes `drv.pump_reserved_bytes`) using exact capacity reservation!
                        if let Err(SessionError::ResourceLimit(_)) = self.charge_mem(r.length) {
                            return Ok(Some(self.current_terminal_event()));
                        }
                        let mut buf = Vec::new();
                        if buf.try_reserve_exact(target_len).is_err() {
                            self.release_mem(r.length);
                            self.fail_with(
                                ErrorCategory::OutOfMemory,
                                StopCause::None,
                                "failed to allocate bounded range accumulation buffer".to_string(),
                            );
                            return Ok(Some(self.current_terminal_event()));
                        }
                        let cap_bytes = buf.capacity() as u64;
                        let extra_cap = cap_bytes.saturating_sub(r.length);
                        if extra_cap > 0
                            && let Err(SessionError::ResourceLimit(_)) = self.charge_mem(extra_cap)
                        {
                            self.release_mem(r.length);
                            return Ok(Some(self.current_terminal_event()));
                        }
                        if let SessionMode::Execute { direct_pump: Some(ref mut drv), .. } =
                            self.mode
                        {
                            drv.pending_range = Some(PendingRangeAccum {
                                pump_ticket: pt,
                                base_offset: r.offset,
                                target_len,
                                buf,
                                charged_bytes: cap_bytes,
                            });
                        }
                        let first_len = u32::try_from(target_len)
                            .unwrap_or(self.transfer_max)
                            .min(self.transfer_max)
                            .max(1);
                        IoInput::Range { offset: r.offset, len: first_len }
                    }
                    InputRequest::Len => IoInput::Len,
                    _ => IoInput::AtLeast {
                        next_offset: 0,
                        min_remaining: 1,
                        max_transfer: self.transfer_max,
                    },
                };
                let ticket = self.alloc_ticket()?;
                self.active_wait = ActiveWait::Input {
                    ticket,
                    input_idx: 0,
                    request: io_req,
                    pump_ticket: Some(pt),
                };
                Ok(self.current_wait_event())
            }
            DirectStep::Write {
                pump_ticket: pt,
                offset,
                is_patch,
                bytes,
                charged_bytes,
                freed_prefix,
                header_extra_charge,
            } => {
                if freed_prefix > 0 {
                    self.release_mem(freed_prefix);
                }
                if header_extra_charge > 0
                    && let Err(SessionError::ResourceLimit(_)) =
                        self.charge_mem(header_extra_charge)
                {
                    return Ok(Some(self.current_terminal_event()));
                }
                if let Err(SessionError::ResourceLimit(_)) = self.charge_mem(charged_bytes) {
                    return Ok(Some(self.current_terminal_event()));
                }
                let ticket = self.alloc_ticket()?;
                let incremental = self.sinks[0].spec.visibility == Some(Visibility::Incremental);
                let window_len = bytes.len().min(self.transfer_max as usize);
                self.active_wait = ActiveWait::Output(PendingChunk {
                    ticket,
                    sink_idx: 0,
                    offset,
                    is_patch,
                    incremental,
                    bytes,
                    sent_offset: 0,
                    window_len,
                    charged_bytes,
                    pump_ticket: Some(pt),
                    pump_total_acked: 0,
                });
                Ok(self.current_wait_event())
            }
            DirectStep::Finished => {
                // Take and DROP `MediaPump` FIRST (releasing `pump_reserved_bytes` and `in_buf`)
                // BEFORE inspecting the accumulated output header prefix so ZERO second demuxer
                // can ever overlap with a live `MediaPump`!
                let drv = match &mut self.mode {
                    SessionMode::Execute { direct_pump, .. } => {
                        direct_pump.take().expect("direct_pump present on Finished")
                    }
                    _ => unreachable!(),
                };
                let DirectPumpDriver {
                    pump,
                    pump_reserved_bytes,
                    out_total_bytes: total_bytes,
                    in_buf: _,
                    in_buf_charged,
                    pending_range,
                    out_header,
                    out_header_charged,
                    target_container: container,
                    target_codec: codec,
                    output_slot: slot,
                    ..
                } = drv;
                let report = pump.report().clone();
                drop(pump);
                let pending_range_charged = pending_range.map_or(0, |r| r.charged_bytes);
                self.release_mem(
                    pump_reserved_bytes
                        .saturating_add(in_buf_charged)
                        .saturating_add(pending_range_charged),
                );

                self.check_deadline();
                if self.stop.is_stopped() {
                    drop(out_header);
                    self.release_mem(out_header_charged);
                    let cause = self.stop.cause();
                    self.fail_with(
                        ErrorCategory::OperationCancelled,
                        cause,
                        format!("job stopped before finalizing output ({cause:?})"),
                    );
                    return Ok(Some(self.current_terminal_event()));
                }

                // Inspect strictly the assembled emitted output header prefix (covering split
                // WriteOffer chunks AND finish patches!) under the remaining memory budget + live
                // deadline pulse. NEVER fabricate output dimensions from input track geometry: if
                // headers were not complete within the bounded 64 KiB prefix, emit an explicit
                // warning reporting geometry unavailable!
                let rem_services = self.services_with_remaining_budget();
                let mut warnings = Vec::new();
                let dims = if let Some((d, _)) = probe_selected_video_track_descriptor(
                    &self.engine,
                    &rem_services,
                    &out_header,
                    &TrackPolicy::DropNonVideo,
                    Some(self.deadline_pulse()),
                ) {
                    d
                } else {
                    warnings.push(
                        "output video dimensions unavailable from bounded 64 KiB output header prefix"
                            .to_string(),
                    );
                    (0, 0)
                };
                drop(out_header);
                self.release_mem(out_header_charged);

                self.check_deadline();
                if self.stop.is_stopped() {
                    let cause = self.stop.cause();
                    self.fail_with(
                        ErrorCategory::OperationCancelled,
                        cause,
                        format!("job stopped before finishing ({cause:?})"),
                    );
                    return Ok(Some(self.current_terminal_event()));
                }

                let mime = match container {
                    ContainerId::Mp4 => "video/mp4",
                    ContainerId::Webm => "video/webm",
                    ContainerId::Ivf => "video/x-ivf",
                    _ => "application/octet-stream",
                }
                .to_string();
                let artifact = OutputV2::artifact_from_report(
                    slot,
                    "video",
                    mime,
                    total_bytes,
                    dims.0,
                    dims.1,
                    None,
                    Some(video_codec_token(&codec)),
                    &report,
                );
                let out_v2 = OutputV2::new(vec![artifact], warnings);
                self.staged_result_json = Some(
                    serde_json::to_vec(&out_v2)
                        .map_err(|e| SessionError::Internal(e.to_string()))?,
                );
                self.lifecycle = Lifecycle::Finishing;
                Ok(None)
            }
        }
    }

    fn step_pull_inputs(&mut self) -> std::result::Result<Option<IoEvent>, SessionError> {
        for idx in 0..self.inputs.len() {
            if self.inputs[idx].eof {
                continue;
            }
            if self.inputs[idx].spec.access.has_known_length() && !self.inputs[idx].len_queried {
                let ticket = self.alloc_ticket()?;
                self.active_wait = ActiveWait::Input {
                    ticket,
                    input_idx: idx,
                    request: IoInput::Len,
                    pump_ticket: None,
                };
                return Ok(self.current_wait_event());
            }
            let next_offset = self.inputs[idx].data.len() as u64;
            let remaining_cap = self.max_input_bytes.saturating_sub(next_offset);
            if remaining_cap == 0 {
                self.fail_with(
                    ErrorCategory::ArgumentInvalid,
                    StopCause::None,
                    format!(
                        "input slot '{}' reached max_input_bytes {} without EOF",
                        self.inputs[idx].slot, self.max_input_bytes
                    ),
                );
                return Ok(Some(self.current_terminal_event()));
            }
            let max_transfer = self
                .transfer_max
                .min(u32::try_from(remaining_cap).unwrap_or(u32::MAX))
                .max(1);
            let ticket = self.alloc_ticket()?;
            self.active_wait = ActiveWait::Input {
                ticket,
                input_idx: idx,
                request: IoInput::AtLeast {
                    next_offset,
                    min_remaining: 1,
                    max_transfer,
                },
                pump_ticket: None,
            };
            return Ok(self.current_wait_event());
        }
        Ok(None)
    }

    fn services_with_remaining_budget(&self) -> HostServices {
        let mut s = self.services.clone();
        if let Some(cap) = self.max_memory_bytes {
            let rem = cap.saturating_sub(self.live_mem_bytes);
            let mut lim = s.limits.unwrap_or_default();
            lim = lim.with_max_memory(lim.max_memory_bytes.map_or(rem, |c| c.min(rem)));
            s.limits = Some(lim);
        }
        s
    }

    fn execute_buffered_operation(&mut self) {
        self.check_deadline();
        if self.stop.is_stopped() {
            let cause = self.stop.cause();
            self.fail_with(
                ErrorCategory::OperationCancelled,
                cause,
                format!("job stopped before execution ({cause:?})"),
            );
            return;
        }
        let deadline_stop = self.deadline_stop();
        let pulse = zc::SharedPulse::new(zc::how_far::StopOnly::new(deadline_stop.clone()));
        let rem_services = self.services_with_remaining_budget();

        match &self.mode {
            SessionMode::Inspect { target_slot, expected_container, bounded_index_scan } => {
                let Some(inp) = self.inputs.iter().find(|i| &i.slot == target_slot) else {
                    self.fail_with(
                        ErrorCategory::ArgumentInvalid,
                        StopCause::None,
                        format!("inspect slot '{}' not bound", target_slot),
                    );
                    return;
                };
                match inspect_bytes_with_stop(
                    &self.engine,
                    &rem_services,
                    &inp.data,
                    expected_container.as_deref(),
                    *bounded_index_scan,
                    Some(pulse),
                ) {
                    Ok(report) => {
                        self.check_deadline();
                        if self.stop.is_stopped() {
                            let cause = self.stop.cause();
                            self.fail_with(
                                ErrorCategory::OperationCancelled,
                                cause,
                                format!("inspect stopped before completion ({cause:?})"),
                            );
                            return;
                        }
                        match serde_json::to_vec(&report) {
                            Ok(json) => {
                                self.terminal_json = Some(json);
                                self.lifecycle = Lifecycle::Complete;
                            }
                            Err(e) => self.fail_with(
                                ErrorCategory::InternalError,
                                StopCause::None,
                                e.to_string(),
                            ),
                        }
                    }
                    Err(e) => {
                        self.check_deadline();
                        self.fail_with(e.category(), self.stop.cause(), e.message);
                    }
                }
            }
            SessionMode::Estimate { job } => {
                let mut b = Bindings::new();
                let mut prefixes = Vec::with_capacity(self.inputs.len());
                for inp in &self.inputs {
                    prefixes.push((inp.slot.as_str(), inp.data.as_slice()));
                    match b.with_bytes(&inp.slot, &inp.data) {
                        Ok(next_b) => b = next_b,
                        Err(e) => {
                            let fe = crate::media::map_pipe_error(e);
                            self.fail_with(fe.category(), StopCause::None, fe.message);
                            return;
                        }
                    }
                }
                let rt = PipelineRuntime::new(Arc::clone(&self.engine), rem_services.clone());
                match build_estimate_report(
                    &self.engine,
                    &rt,
                    job,
                    b,
                    &prefixes,
                    &rem_services,
                    Some(&deadline_stop),
                ) {
                    Ok(report) => {
                        self.check_deadline();
                        if self.stop.is_stopped() {
                            let cause = self.stop.cause();
                            self.fail_with(
                                ErrorCategory::OperationCancelled,
                                cause,
                                format!("estimate stopped before completion ({cause:?})"),
                            );
                            return;
                        }
                        match serde_json::to_vec(&report) {
                            Ok(json) => {
                                self.terminal_json = Some(json);
                                self.lifecycle = Lifecycle::Complete;
                            }
                            Err(e) => self.fail_with(
                                ErrorCategory::InternalError,
                                StopCause::None,
                                e.to_string(),
                            ),
                        }
                    }
                    Err(e) => {
                        self.check_deadline();
                        self.fail_with(e.category(), self.stop.cause(), e.message);
                    }
                }
            }
            SessionMode::Execute { job, .. } => {
                // Pinned zenpipe `PreparedJob::run()` cannot observe a cooperative `Pulse` mid-DAG
                // or yield per `WorkQuota`. Already-expired deadlines failed above with
                // `StopCause::Deadline`; explicitly refuse live `timeout_ms`/deadlines on buffered
                // multi-node Execute pipelines rather than running un-interruptibly past deadline!
                if self.deadline_at.is_some() {
                    self.fail_with(
                        ErrorCategory::ActionNotSupported,
                        StopCause::None,
                        "deadline/timeout_ms cannot be enforced mid-execution on buffered multi-node Execute pipelines with the pinned zenpipe runtime; use a direct video streaming route, Inspect, or Estimate, or omit timeout_ms".to_string(),
                    );
                    return;
                }
                let mut b = Bindings::new();
                for inp in &self.inputs {
                    match b.with_bytes(&inp.slot, &inp.data) {
                        Ok(next_b) => b = next_b,
                        Err(e) => {
                            let fe = crate::media::map_pipe_error(e);
                            self.fail_with(fe.category(), StopCause::None, fe.message);
                            return;
                        }
                    }
                }
                let rt = PipelineRuntime::new(Arc::clone(&self.engine), rem_services);
                let prepared = match rt.prepare(job, b) {
                    Ok(p) => p,
                    Err(e) => {
                        let fe = crate::media::map_pipe_error(e);
                        self.fail_with(fe.category(), StopCause::None, fe.message);
                        return;
                    }
                };
                self.check_deadline();
                if self.stop.is_stopped() {
                    let cause = self.stop.cause();
                    self.fail_with(
                        ErrorCategory::OperationCancelled,
                        cause,
                        format!("job stopped after prepare ({cause:?})"),
                    );
                    return;
                }
                let job_res = match prepared.run() {
                    Ok(r) => r,
                    Err(e) => {
                        let fe = crate::media::map_pipe_error(e);
                        self.fail_with(fe.category(), self.stop.cause(), fe.message);
                        return;
                    }
                };
                self.check_deadline();
                if self.stop.is_stopped() {
                    let cause = self.stop.cause();
                    self.fail_with(
                        ErrorCategory::OperationCancelled,
                        cause,
                        format!("job stopped before staging output ({cause:?})"),
                    );
                    return;
                }
                // Release retained buffered inputs now that execution succeeded.
                let mut freed_in = 0u64;
                for inp in &mut self.inputs {
                    freed_in = freed_in.saturating_add(std::mem::take(&mut inp.charged_cap));
                    inp.data.clear();
                    inp.data.shrink_to_fit();
                }
                self.release_mem(freed_in);

                let out_v2 = job_res.output_v2();
                match serde_json::to_vec(&out_v2) {
                    Ok(json) => self.staged_result_json = Some(json),
                    Err(e) => {
                        self.fail_with(
                            ErrorCategory::InternalError,
                            StopCause::None,
                            e.to_string(),
                        );
                        return;
                    }
                }
                for art in job_res.outputs {
                    let Some(sink_idx) =
                        self.sinks.iter().position(|s| s.slot == art.slot.as_str())
                    else {
                        self.fail_with(
                            ErrorCategory::InternalError,
                            StopCause::None,
                            format!("unmapped output artifact slot '{}'", art.slot),
                        );
                        return;
                    };
                    let payload = if !art.bytes.is_empty() {
                        art.bytes
                    } else if let Some(px) = art.pixels {
                        px.as_view().row(0).to_vec()
                    } else {
                        Vec::new()
                    };
                    if let Some(max_out) = self.max_output_bytes
                        && payload.len() as u64 > max_out
                    {
                        self.fail_with(
                            ErrorCategory::ArgumentInvalid,
                            StopCause::None,
                            format!(
                                "output slot '{}' ({} B) exceeds max_output_bytes {}",
                                art.slot,
                                payload.len(),
                                max_out
                            ),
                        );
                        return;
                    }
                    if let Err(SessionError::ResourceLimit(_)) =
                        self.charge_mem(payload.len() as u64)
                    {
                        return;
                    }
                    self.queued_outputs.push((sink_idx, payload));
                }
                self.lifecycle = Lifecycle::Finishing;
            }
        }
    }

    fn step_publication(&mut self) -> std::result::Result<Option<IoEvent>, SessionError> {
        // 1. Emit next bounded chunk from queued_outputs.
        if !self.queued_outputs.is_empty() {
            let (sink_idx, payload) = self.queued_outputs.remove(0);
            if !payload.is_empty() {
                let offset = self.sinks[sink_idx].accepted_bytes;
                let incremental =
                    self.sinks[sink_idx].spec.visibility == Some(Visibility::Incremental);
                let ticket = self.alloc_ticket()?;
                let charged_bytes = payload.len() as u64;
                let window_len = payload.len().min(self.transfer_max as usize);
                self.active_wait = ActiveWait::Output(PendingChunk {
                    ticket,
                    sink_idx,
                    offset,
                    is_patch: false,
                    incremental,
                    bytes: payload,
                    sent_offset: 0,
                    window_len,
                    charged_bytes,
                    pump_ticket: None,
                    pump_total_acked: 0,
                });
                return Ok(self.current_wait_event());
            }
            return Ok(None);
        }

        // 2. Flush every sink in deterministic order.
        for idx in 0..self.sinks.len() {
            if !self.sinks[idx].flushed {
                let ticket = self.alloc_ticket()?;
                self.active_wait = ActiveWait::Flush { ticket, sink_idx: idx };
                return Ok(self.current_wait_event());
            }
        }

        // 3. Single-atomic `Running -> Committing` vs `Running -> Stopped` race on JobStop!
        self.check_deadline();
        let has_staged = self.sinks.iter().any(|s| s.spec.visibility == Some(Visibility::Staged));
        if has_staged {
            match self.stop.try_enter_committing() {
                Ok(()) => {
                    let ticket = self.alloc_ticket()?;
                    self.lifecycle = Lifecycle::Committing;
                    self.active_wait = ActiveWait::Commit { ticket };
                    return Ok(Some(IoEvent::Commit { ticket }));
                }
                Err(cause) => {
                    self.fail_with(
                        ErrorCategory::OperationCancelled,
                        cause,
                        format!("job stopped before publication commit ({cause:?})"),
                    );
                    return Ok(Some(self.current_terminal_event()));
                }
            }
        }

        // Incremental-only route: final deadline + stop check before Complete.
        self.check_deadline();
        if self.stop.is_stopped() {
            let cause = self.stop.cause();
            self.fail_with(
                ErrorCategory::OperationCancelled,
                cause,
                format!("job stopped before completion ({cause:?})"),
            );
            return Ok(Some(self.current_terminal_event()));
        }

        self.terminal_json = self.staged_result_json.take();
        self.lifecycle = Lifecycle::Complete;
        Ok(Some(IoEvent::Complete))
    }

    fn alloc_ticket(&mut self) -> std::result::Result<IoTicket, SessionError> {
        let cur = self.next_ticket;
        self.next_ticket = cur
            .checked_add(1)
            .ok_or_else(|| SessionError::ResourceLimit("IoTicket sequence exhausted".into()))?;
        let nz = NonZeroU64::new(cur).expect("ticket starts at 1");
        Ok(IoTicket::new(nz))
    }

    fn charge_mem(&mut self, bytes: u64) -> std::result::Result<(), SessionError> {
        let next = self.live_mem_bytes.saturating_add(bytes);
        if let Some(cap) = self.max_memory_bytes
            && next > cap
        {
            self.fail_with(
                ErrorCategory::OutOfMemory,
                StopCause::None,
                format!("streaming live memory {} B exceeds max_memory_bytes {}", next, cap),
            );
            return Err(SessionError::ResourceLimit(
                "streaming live memory exceeds max_memory_bytes".into(),
            ));
        }
        self.live_mem_bytes = next;
        Ok(())
    }

    fn release_mem(&mut self, bytes: u64) {
        self.live_mem_bytes = self.live_mem_bytes.saturating_sub(bytes);
    }

    fn release_pending_range_mem(&mut self) {
        let mut freed = 0u64;
        if let SessionMode::Execute { direct_pump: Some(ref mut drv), .. } = self.mode {
            if let Some(acc) = drv.pending_range.take() {
                freed = freed.saturating_add(acc.charged_bytes);
            }
            if drv.in_buf_charged > 0 {
                drv.in_buf.clear();
                drv.in_buf.shrink_to_fit();
                freed = freed.saturating_add(std::mem::take(&mut drv.in_buf_charged));
            }
        }
        if freed > 0 {
            self.release_mem(freed);
        }
    }

    fn release_all_direct_and_input_mem(&mut self) {
        let mut freed = 0u64;
        if let SessionMode::Execute { ref mut direct_pump, .. } = self.mode
            && let Some(drv) = direct_pump.take()
        {
            freed = freed
                .saturating_add(drv.pump_reserved_bytes)
                .saturating_add(drv.in_buf_charged)
                .saturating_add(drv.out_header_charged)
                .saturating_add(drv.pending_range.map_or(0, |a| a.charged_bytes));
            drop(drv.pump);
        }
        for inp in &mut self.inputs {
            freed = freed.saturating_add(std::mem::take(&mut inp.charged_cap));
            inp.data.clear();
            inp.data.shrink_to_fit();
        }
        if freed > 0 {
            self.release_mem(freed);
        }
    }

    fn current_wait_event(&self) -> Option<IoEvent> {
        match &self.active_wait {
            ActiveWait::None => None,
            ActiveWait::Input { ticket, input_idx, request, .. } => Some(IoEvent::NeedInput {
                ticket: *ticket,
                io_id: self.inputs[*input_idx].io_id,
                request: *request,
            }),
            ActiveWait::Output(chunk) => {
                let len = u32::try_from(chunk.window_len).unwrap_or(u32::MAX);
                let io_id = self.sinks[chunk.sink_idx].io_id;
                if chunk.is_patch {
                    Some(IoEvent::Patch {
                        ticket: chunk.ticket,
                        io_id,
                        offset: chunk.offset,
                        len,
                        incremental: chunk.incremental,
                    })
                } else {
                    Some(IoEvent::Output {
                        ticket: chunk.ticket,
                        io_id,
                        offset: chunk.offset,
                        len,
                        incremental: chunk.incremental,
                    })
                }
            }
            ActiveWait::Flush { ticket, sink_idx } => {
                Some(IoEvent::Flush { ticket: *ticket, io_id: self.sinks[*sink_idx].io_id })
            }
            ActiveWait::Commit { ticket } => Some(IoEvent::Commit { ticket: *ticket }),
        }
    }

    fn current_terminal_event(&self) -> IoEvent {
        match self.lifecycle {
            Lifecycle::Complete => IoEvent::Complete,
            Lifecycle::Failed { category, cause } => IoEvent::Failed { category, cause },
            _ => IoEvent::Yield,
        }
    }

    fn fail_with(&mut self, category: ErrorCategory, cause: StopCause, message: String) {
        // Before clearing active_wait, account for any interrupted in-flight incremental write,
        // flush, or commit so receipts NEVER falsely claim exact bytes!
        match std::mem::replace(&mut self.active_wait, ActiveWait::None) {
            ActiveWait::Output(chunk) => {
                self.release_mem(chunk.charged_bytes);
                if chunk.incremental {
                    self.sinks[chunk.sink_idx].exact_unknown_tail = true;
                }
            }
            ActiveWait::Flush { sink_idx, .. } => {
                if self.sinks[sink_idx].spec.visibility == Some(Visibility::Incremental) {
                    self.sinks[sink_idx].exact_unknown_tail = true;
                }
            }
            ActiveWait::Commit { .. } => {
                for s in &mut self.sinks {
                    if s.spec.visibility == Some(Visibility::Staged) && !s.committed {
                        s.exact_unknown_tail = true;
                    }
                }
            }
            ActiveWait::Input { .. } | ActiveWait::None => {}
        }
        self.release_all_direct_and_input_mem();
        let freed_queued: u64 = self.queued_outputs.drain(..).map(|(_, buf)| buf.len() as u64).sum();
        self.release_mem(freed_queued);
        self.lifecycle = Lifecycle::Failed { category, cause };
        let publications = self
            .sinks
            .iter()
            .map(|s| PublicationReceiptV2 {
                io_id: s.io_id,
                slot: s.slot.clone(),
                visibility: match s.spec.visibility {
                    Some(Visibility::Incremental) => "incremental".to_string(),
                    _ => "staged".to_string(),
                },
                accepted_bytes: s.accepted_bytes.to_string(),
                exact_unknown_tail: s.exact_unknown_tail,
                committed: s.committed,
            })
            .collect();
        let report = StreamErrorReportV2 {
            version: 2,
            success: false,
            category: category.to_outward_error_code(),
            stop_reason: cause.as_u32(),
            message,
            publications,
        };
        self.terminal_json = serde_json::to_vec(&report).ok();
    }
}

/// Choose the most compatible request from `MediaPump::NeedInput`'s alternatives so a
/// forward-only binding picks `AtLeast` (or `Len` if advertised) instead of failing on `Range`.
fn pick_compatible_pump_request(
    requests: &[InputRequest],
    is_range_capable: bool,
    has_known_len: bool,
) -> InputRequest {
    if requests.is_empty() {
        return InputRequest::at_least(0, 1);
    }
    if is_range_capable {
        if has_known_len
            && let Some(len_req) = requests.iter().copied().find(|r| matches!(r, InputRequest::Len))
        {
            return len_req;
        }
        return requests[0];
    }
    // Forward-only binding: prefer AtLeast, then Len (if known_length), only Range if no alternative exists.
    if let Some(at_least) =
        requests.iter().copied().find(|r| matches!(r, InputRequest::AtLeast { .. }))
    {
        return at_least;
    }
    if has_known_len
        && let Some(len_req) = requests.iter().copied().find(|r| matches!(r, InputRequest::Len))
    {
        return len_req;
    }
    requests[0]
}

enum DirectStep {
    Progress,
    Finished,
    NeedInput(PumpTicket, InputRequest),
    Write {
        pump_ticket: PumpTicket,
        offset: u64,
        is_patch: bool,
        bytes: Vec<u8>,
        charged_bytes: u64,
        freed_prefix: u64,
        header_extra_charge: u64,
    },
    Fail(ErrorCategory, String),
    FailWithFree {
        category: ErrorCategory,
        message: String,
        freed_prefix: u64,
    },
    Error(FlowError),
}

fn parse_slot_id_map(
    val: Option<&Value>,
    field: &str,
) -> std::result::Result<Vec<(String, i32)>, SessionError> {
    let Some(v) = val else {
        return Ok(Vec::new());
    };
    let obj = v.as_object().ok_or_else(|| {
        SessionError::InvalidArgument(format!("envelope '{}' must be an object", field))
    })?;
    let mut out = Vec::with_capacity(obj.len());
    for (slot, id_val) in obj {
        let Some(id_i64) = id_val.as_i64() else {
            return Err(SessionError::InvalidArgument(format!(
                "envelope '{}[{}]' must be an i32 io_id",
                field, slot
            )));
        };
        let io_id = i32::try_from(id_i64).map_err(|_| {
            SessionError::InvalidArgument(format!(
                "envelope '{}[{}]' io_id out of i32 range",
                field, slot
            ))
        })?;
        out.push((slot.clone(), io_id));
    }
    Ok(out)
}

/// Detect whether `job` is a pure 1-input -> 1-output video-to-video direct streaming route
/// (single `Decode(select: None)` -> single `Encode(Video)`), eligible for direct `MediaPump`
/// sans-I/O streaming without buffering the full input!
pub(crate) fn extract_direct_video_pump_intent_slots(
    job: &CanonicalJob,
    in_slot: &str,
    out_slot: &str,
) -> Option<OutputIntent> {
    if job.graph().nodes().len() != 1 || job.graph().outputs().len() != 1 {
        return None;
    }
    let ImageOp::Decode(ref dec) = job.graph().nodes()[0] else {
        return None;
    };
    if dec.slot.as_str() != in_slot || dec.select.is_some() {
        return None;
    }
    let OutputOp::Encode(ref enc) = job.graph().outputs()[0] else {
        return None;
    };
    if enc.slot.as_str() != out_slot {
        return None;
    }
    if matches!(enc.intent.target, TargetFormat::Video { .. }) {
        Some(enc.intent.clone())
    } else {
        None
    }
}

fn extract_direct_video_pump_intent(
    job: &CanonicalJob,
    inputs: &[InputBufferState],
    sinks: &[OutputSinkState],
) -> Option<OutputIntent> {
    if inputs.len() != 1 || sinks.len() != 1 {
        return None;
    }
    extract_direct_video_pump_intent_slots(job, &inputs[0].slot, &sinks[0].slot)
}
