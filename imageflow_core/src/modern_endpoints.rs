//! Modern JSON v2 / `v1/execute_zen` endpoint dispatcher (`§2.3` & Decision 1).
//!
//! Implements the canonical `zenpipe-canonical-v1` wire contract (`encode.format`,
//! `encode.video_codec`, `decode.select`, `encode.sequence`, `encode.tracks`,
//! `encode.publication`, `encode.metadata`, `encode.coding`, `OutputV2`) across
//! `v1/execute_zen`, `v2/execute`, `v2/inspect`, `v2/estimate`, `v2/configure_context`,
//! and `v2/capabilities`, while leaving legacy v1 endpoints 100% untouched.

use serde::Deserialize;

use crate::json::{parse_json_with_limit, JsonResponse};
use crate::{Context, ErrorCategory, ErrorKind, Result};

/// Cross-platform ceiling on `limits.timeout_ms` (~100 years) so unrepresentable or
/// overflow deadlines fail deterministically as `InvalidArgument` on Linux, macOS, and Windows.
pub(crate) const MAX_TIMEOUT_MS: u64 = 100 * 365 * 86_400 * 1000;

/// Resolve a `timeout_ms` ceiling into an absolute monotonic [`std::time::Instant`], returning
/// `None` if it exceeds [`MAX_TIMEOUT_MS`] or overflows the host monotonic clock.
pub(crate) fn deadline_instant_from_timeout_ms(ms: u64) -> Option<std::time::Instant> {
    if ms > MAX_TIMEOUT_MS {
        return None;
    }
    std::time::Instant::now().checked_add(std::time::Duration::from_millis(ms))
}

/// Attempt to dispatch a modern (`v2/*` or `v1/execute_zen`) endpoint on `context`.
/// Returns `Ok(None)` for all legacy endpoints so `v1::invoke` runs untouched.
pub fn try_invoke(
    context: &mut Context,
    method: &str,
    json: &[u8],
) -> Result<Option<JsonResponse>> {
    let max_json = context.security.max_json_bytes;
    match method {
        "v2/configure_context" => {
            let req = parse_json_with_limit::<ConfigureContextV2>(json, max_json)?;
            apply_configure_context(context, req)?;
            Ok(Some(JsonResponse::ok(serde_json::json!({
                "input_registration": if context.is_input_registration_deferred() { "deferred" } else { "eager" },
                "workspace_policy": if context.modern_config().workspace_policy_best_effort { "best_effort" } else { "strict" }
            }))))
        }
        "v2/capabilities" => {
            let _ = parse_json_with_limit::<serde_json::Value>(json, max_json)?;
            Ok(Some(JsonResponse::ok(capabilities_json())))
        }
        "v1/execute_zen" | "v2/execute" => {
            #[cfg(feature = "zen-media")]
            {
                let resp = invoke_execute_modern(context, json, max_json)?;
                Ok(Some(resp))
            }
            #[cfg(not(feature = "zen-media"))]
            {
                let _ = (json, max_json);
                Err(nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "endpoint '{}' requires feature 'zen-media' in this build",
                    method
                ))
            }
        }
        "v2/inspect" => {
            #[cfg(feature = "zen-media")]
            {
                let resp = invoke_inspect_modern(context, json, max_json)?;
                Ok(Some(resp))
            }
            #[cfg(not(feature = "zen-media"))]
            {
                let _ = (json, max_json);
                Err(nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "endpoint 'v2/inspect' requires feature 'zen-media' in this build"
                ))
            }
        }
        "v2/estimate" => {
            #[cfg(feature = "zen-media")]
            {
                let resp = invoke_estimate_modern(context, json, max_json)?;
                Ok(Some(resp))
            }
            #[cfg(not(feature = "zen-media"))]
            {
                let _ = (json, max_json);
                Err(nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "endpoint 'v2/estimate' requires feature 'zen-media' in this build"
                ))
            }
        }
        "v2/compare" => {
            #[cfg(feature = "zen-media")]
            {
                let resp = invoke_compare_modern(context, json, max_json)?;
                Ok(Some(resp))
            }
            #[cfg(not(feature = "zen-media"))]
            {
                let _ = (json, max_json);
                Err(nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "endpoint 'v2/compare' requires feature 'zen-media' in this build"
                ))
            }
        }
        _ => Ok(None),
    }
}

/// Static modern endpoint dispatch (`v2/capabilities` works even without a `Context`).
pub fn try_invoke_static(method: &str, json: &[u8]) -> Result<Option<JsonResponse>> {
    if method == "v2/capabilities" {
        let _ = parse_json_with_limit::<serde_json::Value>(json, Some(64 * 1024))?;
        return Ok(Some(JsonResponse::ok(capabilities_json())));
    }
    Ok(None)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigureContextV2 {
    #[serde(default)]
    input_registration: Option<String>,
    #[serde(default)]
    workspace_policy: Option<String>,
    #[serde(default)]
    limits: Option<ConfigureLimitsV2>,
    #[serde(default)]
    trace_capacity: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigureLimitsV2 {
    #[serde(default)]
    max_memory_bytes: Option<u64>,
    #[serde(default)]
    max_output_bytes: Option<u64>,
    #[serde(default)]
    max_input_bytes: Option<u64>,
    #[serde(default)]
    max_frames: Option<u32>,
    #[serde(default)]
    max_dimension: Option<u32>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

fn apply_configure_context(context: &mut Context, req: ConfigureContextV2) -> Result<()> {
    // Validate ALL fields before mutating context so a rejected configure_context is atomic.
    let deferred = match req.input_registration.as_deref() {
        None => None,
        Some("deferred") => Some(true),
        Some("eager") => Some(false),
        Some(other) => {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "invalid input_registration '{}' (expected 'eager' or 'deferred')",
                other
            ));
        }
    };
    let best_effort = match req.workspace_policy.as_deref() {
        None => None,
        Some("strict") => Some(false),
        Some("best_effort") => Some(true),
        Some(other) => {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "invalid workspace_policy '{}' (expected 'strict' or 'best_effort')",
                other
            ));
        }
    };
    if let Some(ms) = req.limits.as_ref().and_then(|l| l.timeout_ms)
        && deadline_instant_from_timeout_ms(ms).is_none()
    {
        return Err(nerror!(
            ErrorKind::InvalidArgument,
            "limits.timeout_ms {} cannot be represented as a monotonic deadline",
            ms
        ));
    }

    if let Some(d) = deferred {
        context.set_input_registration_deferred(d);
    }
    if let Some(be) = best_effort {
        context.modern_config_mut().workspace_policy_best_effort = be;
    }
    if let Some(lim) = req.limits {
        let cfg = context.modern_config_mut();
        if lim.max_memory_bytes.is_some() {
            cfg.max_memory_bytes = lim.max_memory_bytes;
        }
        if lim.max_output_bytes.is_some() {
            cfg.max_output_bytes = lim.max_output_bytes;
        }
        if lim.max_input_bytes.is_some() {
            cfg.max_input_bytes = lim.max_input_bytes;
        }
        if lim.max_frames.is_some() {
            cfg.max_frames = lim.max_frames;
        }
        if lim.max_dimension.is_some() {
            cfg.max_dimension = lim.max_dimension;
        }
        if lim.timeout_ms.is_some() {
            cfg.timeout_ms = lim.timeout_ms;
        }
    }
    if let Some(tc) = req.trace_capacity {
        context.modern_config_mut().trace_capacity = Some(tc);
    }
    Ok(())
}

fn capabilities_json() -> serde_json::Value {
    #[cfg(feature = "zen-media")]
    {
        serde_json::to_value(crate::media::build_capabilities_report())
            .unwrap_or_else(|_| serde_json::json!({ "version": 2, "zen_media": true }))
    }
    #[cfg(not(feature = "zen-media"))]
    {
        serde_json::json!({
            "version": 2,
            "zen_media": false,
            "streaming_manual": false,
            "streaming_async": false,
            "cms_provider": null,
            "still_formats": if cfg!(feature = "zen-codecs") {
                vec!["jpeg", "png", "webp", "gif", "avif", "jxl", "bmp"]
            } else if cfg!(feature = "bmp") {
                vec!["bmp"]
            } else {
                vec![]
            },
            "video_codecs": Vec::<&str>::new(),
            "media_containers": Vec::<&str>::new(),
            "verify_metrics": Vec::<&str>::new(),
            "iqa_metrics": Vec::<serde_json::Value>::new(),
            "media_routes": Vec::<serde_json::Value>::new(),
        })
    }
}

#[cfg(feature = "zen-media")]
mod media_impl {
    use super::*;
    use std::collections::{BTreeMap, HashSet};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use enough::{Stop, StopReason};
    use serde_json::Value;
    use zenpipe::canonical::json::parse_pipeline_value;
    use zenpipe::canonical::{Bindings, PipelineRuntime, WireLimitConfig};

    use crate::context::{CancellationToken, ModernInputLease};
    use crate::media::{
        host_services_from_context, map_pipe_error,
        report::{build_estimate_report, inspect_bytes_with_stop},
        shared_modern_engine, validate_and_apply_modern_limits, ModernLimitsV2,
    };

    /// Cooperative stop combining live `Context` cancellation with resolved `timeout_ms` deadline.
    #[derive(Clone)]
    struct DeadlineStop {
        cancel: CancellationToken,
        deadline: Option<Instant>,
    }

    impl DeadlineStop {
        fn new(cancel: CancellationToken, timeout_ms: Option<u64>) -> Result<Self> {
            let deadline = match timeout_ms {
                Some(ms) => {
                    Some(crate::media::deadline_instant_from_timeout_ms(ms).ok_or_else(|| {
                        nerror!(
                            ErrorKind::InvalidArgument,
                            "limits.timeout_ms {} cannot be represented as a monotonic deadline",
                            ms
                        )
                    })?)
                }
                None => None,
            };
            Ok(Self { cancel, deadline })
        }

        fn deadline_at(&self) -> Option<Instant> {
            self.deadline
        }

        fn check_flow(&self) -> Result<()> {
            if self.cancel.should_stop() {
                return Err(nerror!(ErrorKind::OperationCancelled, "operation cancelled"));
            }
            if let Some(dl) = self.deadline
                && Instant::now() >= dl
            {
                return Err(nerror!(
                    ErrorKind::OperationCancelled,
                    "operation timed out (timeout_ms)"
                ));
            }
            Ok(())
        }
    }

    impl Stop for DeadlineStop {
        fn check(&self) -> std::result::Result<(), StopReason> {
            self.cancel.check()?;
            if let Some(dl) = self.deadline
                && Instant::now() >= dl
            {
                return Err(StopReason::TimedOut);
            }
            Ok(())
        }

        fn should_stop(&self) -> bool {
            self.cancel.should_stop() || self.deadline.is_some_and(|dl| Instant::now() >= dl)
        }
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ExecuteEnvelopeV2 {
        #[serde(default)]
        inputs: BTreeMap<String, i32>,
        #[serde(default)]
        outputs: BTreeMap<String, i32>,
        pipeline: Value,
        #[serde(default)]
        limits: Option<ModernLimitsV2>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct InspectVideoHintV2 {
        #[serde(default)]
        container: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct InspectEnvelopeV2 {
        inputs: BTreeMap<String, i32>,
        input: String,
        #[serde(default)]
        limits: Option<ModernLimitsV2>,
        #[serde(default)]
        video: Option<InspectVideoHintV2>,
        #[serde(default)]
        scan: Option<String>,
    }

    enum MaterializedInput<'a> {
        Borrowed(&'a [u8]),
        Owned(Vec<u8>),
    }

    impl<'a> MaterializedInput<'a> {
        fn as_slice(&self) -> &[u8] {
            match self {
                Self::Borrowed(b) => b,
                Self::Owned(v) => v.as_slice(),
            }
        }
    }

    fn resolve_inputs<'a>(
        context: &'a Context,
        map: &BTreeMap<String, i32>,
        max_input_bytes: Option<u64>,
        max_memory_bytes: Option<u64>,
        stop: &DeadlineStop,
    ) -> Result<(Vec<(String, MaterializedInput<'a>)>, u64)> {
        let mut live_read_mem: u64 = 0;
        let mut out = Vec::with_capacity(map.len());
        for (slot, &io_id) in map {
            stop.check_flow()?;
            let Some(lease) = context.modern_input(io_id) else {
                return Err(nerror!(
                    ErrorKind::IoIdNotFound,
                    "input slot '{}' references unregistered io_id {} (for MP4/WebM/IVF call v2/configure_context with input_registration='deferred' before registering)",
                    slot,
                    io_id
                ));
            };
            let mat = match lease {
                ModernInputLease::Borrowed(slice) => {
                    if let Some(max) = max_input_bytes
                        && slice.len() as u64 > max
                    {
                        return Err(nerror!(
                            ErrorKind::SizeLimitExceeded,
                            "input slot '{}' ({} B) exceeds max_input_bytes {}",
                            slot,
                            slice.len(),
                            max
                        ));
                    }
                    MaterializedInput::Borrowed(slice)
                }
                ModernInputLease::Shared(arc) => {
                    if let Some(max) = max_input_bytes
                        && arc.len() as u64 > max
                    {
                        return Err(nerror!(
                            ErrorKind::SizeLimitExceeded,
                            "input slot '{}' ({} B) exceeds max_input_bytes {}",
                            slot,
                            arc.len(),
                            max
                        ));
                    }
                    MaterializedInput::Borrowed(arc.as_slice())
                }
                #[cfg(feature = "streaming")]
                ModernInputLease::ReadAt(src) => {
                    let rem_mem = max_memory_bytes.map(|m| m.saturating_sub(live_read_mem));
                    let effective_cap = match (max_input_bytes, rem_mem) {
                        (Some(a), Some(b)) => Some(a.min(b)),
                        (Some(a), None) | (None, Some(a)) => Some(a),
                        (None, None) => None,
                    };
                    let bytes = crate::media::bindings::read_all_bounded_with_stop(
                        src.as_ref(),
                        effective_cap,
                        Some(stop),
                    )?;
                    live_read_mem = live_read_mem.saturating_add(bytes.len() as u64);
                    MaterializedInput::Owned(bytes)
                }
            };
            out.push((slot.clone(), mat));
        }
        Ok((out, live_read_mem))
    }

    fn deduct_mem_budget(services: &mut zenpipe::canonical::HostServices, used_bytes: u64) {
        if used_bytes == 0 {
            return;
        }
        if let Some(mut lim) = services.limits
            && let Some(cap) = lim.max_memory_bytes
        {
            lim = lim.with_max_memory(cap.saturating_sub(used_bytes));
            services.limits = Some(lim);
        }
    }

    /// Drive a 1-input -> 1-output direct video transcode over a `ReadAtSource` using
    /// `MediaSession` directly WITHOUT materializing the full input file into RAM!
    ///
    /// Enforces both `timeout_ms` and a non-overlapping memory split between the session
    /// and the staged output `Vec` so combined live allocations stay under `max_memory_bytes`.
    #[cfg(feature = "streaming")]
    fn try_execute_direct_read_at(
        context: &mut Context,
        engine: Arc<zencodecs::engine::Engine>,
        services: &zenpipe::canonical::HostServices,
        stop: &DeadlineStop,
        timeout_ms: Option<u64>,
        env: &ExecuteEnvelopeV2,
        json: &[u8],
        job: &zenpipe::canonical::CanonicalJob,
    ) -> Result<Option<JsonResponse>> {
        use crate::media::{
            BindingSpec, IoEvent, IoInput, JobStop, MediaSession, OperationKind, Reply, StopCause,
            Visibility, WorkQuota,
        };

        if env.inputs.len() != 1 || env.outputs.len() != 1 {
            return Ok(None);
        }
        let (in_slot, &in_io_id) = env.inputs.iter().next().expect("len == 1");
        let (out_slot, &out_io_id) = env.outputs.iter().next().expect("len == 1");
        if crate::media::session::extract_direct_video_pump_intent_slots(job, in_slot, out_slot)
            .is_none()
        {
            return Ok(None);
        }
        let Some(ModernInputLease::ReadAt(src)) = context.modern_input(in_io_id).cloned() else {
            return Ok(None);
        };

        // Reserve ONE reusable bounded transfer scratch buffer up front, deduct its capacity
        // from `max_memory_bytes`, and partition the remaining ceiling strictly between
        // MediaSession workspace and `output_buf.capacity()` (zero uncounted per-credit copies!).
        let mut session_services = services.clone();
        let explicit_out = services.limits.and_then(|l| l.max_output_bytes);
        let mem_cap = services.limits.and_then(|l| l.max_memory_bytes);
        let scratch_cap: usize = match mem_cap {
            Some(m) => usize::try_from(m / 8).unwrap_or(usize::MAX).min(64 * 1024).max(1),
            None => 64 * 1024,
        };
        let mut scratch: Vec<u8> = Vec::new();
        scratch
            .try_reserve_exact(scratch_cap)
            .map_err(|_| nerror!(ErrorKind::AllocationFailed, "transfer scratch reserve failed"))?;
        scratch.resize(scratch_cap, 0);
        let scratch_mem = scratch.capacity() as u64;

        let max_out_cap: Option<u64> = if let Some(m) = mem_cap {
            if scratch_mem >= m {
                return Err(nerror!(
                    ErrorKind::SizeLimitExceeded,
                    "max_memory_bytes {} B too small for bounded streaming transfer buffer",
                    m
                ));
            }
            let rem_after_scratch = m.saturating_sub(scratch_mem);
            let output_mem_share = match explicit_out {
                Some(o) => o.min(rem_after_scratch / 2),
                None => rem_after_scratch / 2,
            };
            if let Some(mut lim) = session_services.limits {
                lim = lim.with_max_memory(rem_after_scratch.saturating_sub(output_mem_share));
                session_services.limits = Some(lim);
            }
            Some(output_mem_share)
        } else {
            explicit_out
        };

        let ctx_stop = context.cancellation_stop();
        stop.check_flow()?;
        let known_len = src.length()?;
        // Recheck the SAME absolute deadline after `src.length()` and pass that exact
        // `Instant` into `MediaSession` (NEVER restart `Instant::now() + timeout_ms`!).
        stop.check_flow()?;
        let bindings = [
            BindingSpec::read_at_source(in_io_id, known_len.is_some()),
            BindingSpec::patch_sink(out_io_id, Visibility::Staged),
        ];
        let job_stop = JobStop::new();
        let mut session = MediaSession::create_with_stop_and_deadline_at(
            engine,
            session_services,
            OperationKind::Execute,
            json,
            &bindings,
            job_stop.clone(),
            timeout_ms.map(Duration::from_millis),
            stop.deadline_at(),
        )
        .map_err(|e| nerror!(ErrorKind::InvalidArgument, "{:?}", e))?;
        session.set_transfer_max(u32::try_from(scratch_cap).unwrap_or(u32::MAX));
        stop.check_flow()?;

        let mut output_buf: Vec<u8> = Vec::new();
        loop {
            if ctx_stop.should_stop() {
                let _ = job_stop.trip(StopCause::CallerCancel);
            }
            let ev = session
                .next(WorkQuota::DEFAULT)
                .map_err(|e| nerror!(ErrorKind::InternalError, "{:?}", e))?;
            match ev {
                IoEvent::Yield => continue,
                IoEvent::Complete => {
                    stop.check_flow()?;
                    let result_bytes = session.result_json().unwrap_or(b"{}");
                    let val: Value = serde_json::from_slice(result_bytes).map_err(|e| {
                        nerror!(ErrorKind::InternalError, "invalid OutputV2 JSON: {}", e)
                    })?;
                    context.commit_modern_output(out_io_id, output_buf)?;
                    return Ok(Some(JsonResponse::ok(val)));
                }
                IoEvent::Failed { category, .. } => {
                    let msg = session
                        .result_json()
                        .and_then(|b| serde_json::from_slice::<Value>(b).ok())
                        .and_then(|v| v.get("message").and_then(Value::as_str).map(String::from))
                        .unwrap_or_else(|| "streaming ReadAt execution failed".to_string());
                    return Err(nerror!(ErrorKind::Category(category), "{}", msg));
                }
                IoEvent::NeedInput { ticket, request, .. } => match request {
                    IoInput::Len => match src.length()? {
                        Some(l) => session
                            .reply(ticket, Reply::Length(l))
                            .map_err(|e| nerror!(ErrorKind::InternalError, "{:?}", e))?,
                        None => session
                            .reply(ticket, Reply::UnknownLength)
                            .map_err(|e| nerror!(ErrorKind::InternalError, "{:?}", e))?,
                    },
                    IoInput::AtLeast { next_offset, max_transfer, .. } => {
                        let want = (max_transfer as usize).min(scratch.len());
                        let n = src.read_at(next_offset, &mut scratch[..want])?;
                        if n == 0 {
                            session
                                .reply(ticket, Reply::End { total_len: known_len })
                                .map_err(|e| nerror!(ErrorKind::InternalError, "{:?}", e))?;
                        } else {
                            session
                                .reply(ticket, Reply::Bytes(&scratch[..n]))
                                .map_err(|e| nerror!(ErrorKind::InternalError, "{:?}", e))?;
                        }
                    }
                    IoInput::Range { offset, len } => {
                        let want = (len as usize).min(scratch.len());
                        let n = src.read_at(offset, &mut scratch[..want])?;
                        if n == 0 {
                            session
                                .reply(ticket, Reply::End { total_len: known_len })
                                .map_err(|e| nerror!(ErrorKind::InternalError, "{:?}", e))?;
                        } else {
                            session
                                .reply(ticket, Reply::Bytes(&scratch[..n]))
                                .map_err(|e| nerror!(ErrorKind::InternalError, "{:?}", e))?;
                        }
                    }
                },
                IoEvent::Output { ticket, len, .. } => {
                    let len_u = len as usize;
                    if len_u > scratch.len() {
                        return Err(nerror!(
                            ErrorKind::InternalError,
                            "output offer {} B exceeds transfer scratch {} B",
                            len_u,
                            scratch.len()
                        ));
                    }
                    if let Some(cap) = max_out_cap {
                        let next_len = (output_buf.len() as u64).saturating_add(u64::from(len));
                        if next_len > cap {
                            return Err(nerror!(
                                ErrorKind::SizeLimitExceeded,
                                "sync ReadAt output {} B exceeds output/memory share {} B",
                                next_len,
                                cap
                            ));
                        }
                    }
                    let n = session
                        .copy_pending(ticket, &mut scratch[..len_u])
                        .map_err(|e| nerror!(ErrorKind::InternalError, "{:?}", e))?;
                    let needed_len = output_buf.len().saturating_add(n);
                    if needed_len > output_buf.capacity() {
                        let target_cap = if let Some(cap) = max_out_cap {
                            let cap_u = usize::try_from(cap).unwrap_or(usize::MAX);
                            if needed_len > cap_u {
                                return Err(nerror!(
                                    ErrorKind::SizeLimitExceeded,
                                    "sync ReadAt output {} B exceeds output/memory share {} B",
                                    needed_len,
                                    cap
                                ));
                            }
                            output_buf.capacity().saturating_mul(2).max(needed_len).min(cap_u)
                        } else {
                            output_buf.capacity().saturating_mul(2).max(needed_len)
                        };
                        output_buf.try_reserve_exact(target_cap - output_buf.len()).map_err(
                            |_| {
                                nerror!(ErrorKind::AllocationFailed, "output buffer reserve failed")
                            },
                        )?;
                        if let Some(cap) = max_out_cap
                            && output_buf.capacity() as u64 > cap
                        {
                            return Err(nerror!(
                                ErrorKind::SizeLimitExceeded,
                                "sync ReadAt output capacity {} B exceeds output/memory share {} B",
                                output_buf.capacity(),
                                cap
                            ));
                        }
                    }
                    output_buf.extend_from_slice(&scratch[..n]);
                    session
                        .reply(ticket, Reply::Ack { bytes: n as u64 })
                        .map_err(|e| nerror!(ErrorKind::InternalError, "{:?}", e))?;
                }
                IoEvent::Patch { ticket, offset, len, .. } => {
                    let len_u = len as usize;
                    if len_u > scratch.len() {
                        return Err(nerror!(
                            ErrorKind::InternalError,
                            "patch offer {} B exceeds transfer scratch {} B",
                            len_u,
                            scratch.len()
                        ));
                    }
                    let n = session
                        .copy_pending(ticket, &mut scratch[..len_u])
                        .map_err(|e| nerror!(ErrorKind::InternalError, "{:?}", e))?;
                    let off = usize::try_from(offset).map_err(|_| {
                        nerror!(ErrorKind::InvalidArgument, "patch offset overflow")
                    })?;
                    if off + n > output_buf.len() {
                        return Err(nerror!(
                            ErrorKind::InvalidArgument,
                            "patch offset out of bounds"
                        ));
                    }
                    output_buf[off..off + n].copy_from_slice(&scratch[..n]);
                    session
                        .reply(ticket, Reply::Ack { bytes: n as u64 })
                        .map_err(|e| nerror!(ErrorKind::InternalError, "{:?}", e))?;
                }
                IoEvent::Flush { ticket, .. } | IoEvent::Commit { ticket } => {
                    session
                        .reply(ticket, Reply::Ack { bytes: 0 })
                        .map_err(|e| nerror!(ErrorKind::InternalError, "{:?}", e))?;
                }
            }
        }
    }

    pub fn invoke_execute_modern(
        context: &mut Context,
        json: &[u8],
        max_json: Option<usize>,
    ) -> Result<JsonResponse> {
        return_if_cancelled!(context);
        let env = parse_json_with_limit::<ExecuteEnvelopeV2>(json, max_json)?;
        let job = parse_pipeline_value(&env.pipeline, WireLimitConfig::default())
            .map_err(map_pipe_error)?;

        let mut seen_out_ids = HashSet::new();
        for (slot, &io_id) in &env.outputs {
            if !seen_out_ids.insert(io_id) {
                return Err(nerror!(
                    ErrorKind::DuplicateIoId,
                    "output slot '{}' reuses io_id {}",
                    slot,
                    io_id
                ));
            }
            context.can_commit_modern_output(io_id)?;
        }

        let engine = shared_modern_engine()?;
        let mut services = host_services_from_context(context);
        let timeout_ms = validate_and_apply_modern_limits(
            &mut services,
            env.limits.as_ref(),
            context.modern_config().timeout_ms,
        )?;
        let stop = DeadlineStop::new(context.cancellation_stop(), timeout_ms)?;
        stop.check_flow()?;

        #[cfg(feature = "streaming")]
        if let Some(resp) = try_execute_direct_read_at(
            context,
            Arc::clone(&engine),
            &services,
            &stop,
            timeout_ms,
            &env,
            json,
            &job,
        )? {
            return Ok(resp);
        }

        let pulse = zc::SharedPulse::new(zc::how_far::StopOnly::new(stop.clone()));
        let max_in = services.limits.and_then(|l| l.max_input_bytes);
        let max_mem = services.limits.and_then(|l| l.max_memory_bytes);

        let result = {
            let (resolved, live_read_mem) =
                resolve_inputs(context, &env.inputs, max_in, max_mem, &stop)?;
            deduct_mem_budget(&mut services, live_read_mem);
            let mut bindings = Bindings::new();
            for (slot, mat) in &resolved {
                bindings = bindings.with_bytes(slot, mat.as_slice()).map_err(map_pipe_error)?;
            }
            stop.check_flow()?;
            let rt = PipelineRuntime::new(engine, services);
            let prepared = rt.prepare(&job, bindings).map_err(map_pipe_error)?;
            stop.check_flow()?;
            prepared.run_with_pulse(pulse).map_err(map_pipe_error)?
        };
        stop.check_flow()?;

        let out_v2 = result.output_v2();
        let mut commits: Vec<(i32, Vec<u8>)> = Vec::with_capacity(result.outputs.len());
        for art in result.outputs {
            let Some(&io_id) = env.outputs.get(art.slot.as_str()) else {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "pipeline emitted output slot '{}' which is missing from envelope 'outputs'",
                    art.slot
                ));
            };
            context.can_commit_modern_output(io_id)?;
            commits.push((io_id, art.bytes));
        }
        stop.check_flow()?;

        for (io_id, bytes) in commits {
            context.commit_modern_output(io_id, bytes)?;
        }

        Ok(JsonResponse::ok(out_v2))
    }

    pub fn invoke_inspect_modern(
        context: &mut Context,
        json: &[u8],
        max_json: Option<usize>,
    ) -> Result<JsonResponse> {
        return_if_cancelled!(context);
        let env = parse_json_with_limit::<InspectEnvelopeV2>(json, max_json)?;
        let Some(&io_id) = env.inputs.get(&env.input) else {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "Inspect 'input' slot '{}' not present in 'inputs'",
                env.input
            ));
        };
        let bounded_index = match env.scan.as_deref() {
            None | Some("headers") => false,
            Some("bounded_index") => true,
            Some(other) => {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "invalid scan mode '{}' (expected 'headers' or 'bounded_index')",
                    other
                ));
            }
        };
        let engine = shared_modern_engine()?;
        let mut services = host_services_from_context(context);
        let timeout_ms = validate_and_apply_modern_limits(
            &mut services,
            env.limits.as_ref(),
            context.modern_config().timeout_ms,
        )?;
        let stop = DeadlineStop::new(context.cancellation_stop(), timeout_ms)?;
        stop.check_flow()?;
        let pulse = zc::SharedPulse::new(zc::how_far::StopOnly::new(stop.clone()));
        let expected_container = env.video.as_ref().and_then(|v| v.container.as_deref());

        #[cfg(feature = "streaming")]
        if let Some(ModernInputLease::ReadAt(src)) = context.modern_input(io_id).cloned() {
            let report = crate::media::report::inspect_read_at_with_stop(
                &engine,
                &services,
                src.as_ref(),
                expected_container,
                bounded_index,
                Some(&stop),
                Some(pulse),
            )?;
            stop.check_flow()?;
            return Ok(JsonResponse::ok(report));
        }

        let max_in = services.limits.and_then(|l| l.max_input_bytes);
        let max_mem = services.limits.and_then(|l| l.max_memory_bytes);
        let single_map = BTreeMap::from([(env.input.clone(), io_id)]);
        let (resolved, live_read_mem) =
            resolve_inputs(context, &single_map, max_in, max_mem, &stop)?;
        deduct_mem_budget(&mut services, live_read_mem);
        let report = inspect_bytes_with_stop(
            &engine,
            &services,
            resolved[0].1.as_slice(),
            expected_container,
            bounded_index,
            Some(pulse),
        )?;
        stop.check_flow()?;
        Ok(JsonResponse::ok(report))
    }

    pub fn invoke_estimate_modern(
        context: &mut Context,
        json: &[u8],
        max_json: Option<usize>,
    ) -> Result<JsonResponse> {
        return_if_cancelled!(context);
        let env = parse_json_with_limit::<ExecuteEnvelopeV2>(json, max_json)?;
        let job = parse_pipeline_value(&env.pipeline, WireLimitConfig::default())
            .map_err(map_pipe_error)?;
        let engine = shared_modern_engine()?;
        let mut services = host_services_from_context(context);
        let timeout_ms = validate_and_apply_modern_limits(
            &mut services,
            env.limits.as_ref(),
            context.modern_config().timeout_ms,
        )?;
        let stop = DeadlineStop::new(context.cancellation_stop(), timeout_ms)?;
        stop.check_flow()?;
        let max_in = services.limits.and_then(|l| l.max_input_bytes);
        let max_mem = services.limits.and_then(|l| l.max_memory_bytes);
        let (resolved, live_read_mem) =
            resolve_inputs(context, &env.inputs, max_in, max_mem, &stop)?;
        deduct_mem_budget(&mut services, live_read_mem);
        let mut bindings = Bindings::new();
        let mut prefixes = Vec::with_capacity(resolved.len());
        for (slot, mat) in &resolved {
            let slice = mat.as_slice();
            prefixes.push((slot.as_str(), slice));
            bindings = bindings.with_bytes(slot, slice).map_err(map_pipe_error)?;
        }
        let rt = PipelineRuntime::new(Arc::clone(&engine), services.clone());
        let est =
            build_estimate_report(&engine, &rt, &job, bindings, &prefixes, &services, Some(&stop))?;
        stop.check_flow()?;
        Ok(JsonResponse::ok(est))
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct CompareEnvelopeV2 {
        reference_io_id: i32,
        candidate_io_id: i32,
        #[serde(default)]
        metrics: Option<Vec<String>>,
        #[serde(default)]
        limits: Option<ModernLimitsV2>,
    }

    fn parse_compare_metric(token: &str) -> Result<zencodecs::engine::Metric> {
        match token {
            "zensim_a" => Ok(zencodecs::engine::Metric::ZensimA),
            "ssimulacra2" => Ok(zencodecs::engine::Metric::Ssimulacra2),
            "butteraugli_max" => Ok(zencodecs::engine::Metric::ButteraugliMax),
            other => Err(nerror!(
                ErrorKind::InvalidArgument,
                "unknown v2/compare metric '{}' (supported: 'zensim_a', 'ssimulacra2', 'butteraugli_max')",
                other
            )),
        }
    }

    fn check_transient_mem(
        live_mem: u64,
        extra: u64,
        total_mem_cap: Option<u64>,
        what: &str,
    ) -> Result<()> {
        let peak = live_mem.saturating_add(extra);
        if let Some(cap) = total_mem_cap
            && peak > cap
        {
            return Err(nerror!(
                ErrorKind::SizeLimitExceeded,
                "v2/compare {} ({} B peak) exceeds max_memory_bytes ({} B)",
                what,
                peak,
                cap
            ));
        }
        Ok(())
    }

    struct RawPngScan {
        iccp_count: usize,
        has_compressed_text: bool,
        has_compressed_itxt: bool,
        ztxt_count: usize,
        text_bytes: u64,
        text_chunks: usize,
        itxt_total_bytes: u64,
        exif_bytes: u64,
        has_actl: bool,
    }

    /// Zero-allocation raw PNG chunk scanner run BEFORE `zenpng::probe()`.
    ///
    /// Catches truncated chunk framing, APNG `acTL`, duplicate/misplaced/malformed `iCCP` chunks,
    /// and text/metadata payload sizes (`tEXt`, `iTXt`, `zTXt`, `eXIf`) so we bound `zenpng`'s
    /// transient probe allocations and retained `creating_tool` metadata before probing AND detect
    /// when `zenpng` silently drops a corrupt `iCCP` zlib stream!
    fn scan_raw_png_chunks(bytes: &[u8], role: &str, stop: &DeadlineStop) -> Result<RawPngScan> {
        let mut scan = RawPngScan {
            iccp_count: 0,
            has_compressed_text: false,
            has_compressed_itxt: false,
            ztxt_count: 0,
            text_bytes: 0,
            text_chunks: 0,
            itxt_total_bytes: 0,
            exif_bytes: 0,
            has_actl: false,
        };
        let mut pos = 8usize;
        let mut saw_iend = false;
        let mut saw_plte = false;
        let mut saw_idat = false;
        let mut chunk_idx = 0usize;
        while pos < bytes.len() {
            if chunk_idx & 63 == 0 {
                stop.check_flow()?;
            }
            chunk_idx = chunk_idx.saturating_add(1);
            if pos + 12 > bytes.len() {
                return Err(nerror!(
                    ErrorKind::ImageDecodingError,
                    "v2/compare {} PNG chunk header truncated at offset {}",
                    role,
                    pos
                ));
            }
            let len_u32 =
                u32::from_be_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]);
            if len_u32 > 0x7FFF_FFFF {
                return Err(nerror!(
                    ErrorKind::ImageDecodingError,
                    "v2/compare {} PNG chunk length {} exceeds spec limit",
                    role,
                    len_u32
                ));
            }
            let chunk_len = len_u32 as usize;
            let chunk_type = &bytes[pos + 4..pos + 8];
            let data_start = pos + 8;
            let Some(data_end) = data_start.checked_add(chunk_len) else {
                return Err(nerror!(
                    ErrorKind::ImageDecodingError,
                    "v2/compare {} PNG chunk length overflow",
                    role
                ));
            };
            let Some(crc_end) = data_end.checked_add(4) else {
                return Err(nerror!(
                    ErrorKind::ImageDecodingError,
                    "v2/compare {} PNG chunk CRC offset overflow",
                    role
                ));
            };
            if crc_end > bytes.len() {
                return Err(nerror!(
                    ErrorKind::ImageDecodingError,
                    "v2/compare {} PNG chunk truncated (need {} B, have {} B)",
                    role,
                    crc_end,
                    bytes.len()
                ));
            }
            let data = &bytes[data_start..data_end];
            match chunk_type {
                b"PLTE" => saw_plte = true,
                b"IDAT" => saw_idat = true,
                b"acTL" => scan.has_actl = true,
                b"eXIf" => {
                    scan.exif_bytes = scan.exif_bytes.saturating_add(chunk_len as u64);
                }
                b"tEXt" => {
                    scan.text_bytes = scan.text_bytes.saturating_add(chunk_len as u64);
                    scan.text_chunks = scan.text_chunks.saturating_add(1);
                }
                b"zTXt" => {
                    scan.has_compressed_text = true;
                    scan.ztxt_count = scan.ztxt_count.saturating_add(1);
                }
                b"iTXt" => {
                    // Always account for raw iTXt payload bytes: `zenpng::detect::probe` ignores
                    // `compression_flag` for Software/Creator iTXt and retains valid UTF-8 text in
                    // `source_encoding_details` (`creating_tool`).
                    scan.itxt_total_bytes = scan.itxt_total_bytes.saturating_add(chunk_len as u64);
                    let compressed = match data.iter().position(|&b| b == 0) {
                        Some(nul) if nul + 1 < data.len() => data[nul + 1] != 0,
                        _ => true,
                    };
                    if compressed {
                        scan.has_compressed_text = true;
                        scan.has_compressed_itxt = true;
                    }
                }
                b"iCCP" => {
                    if saw_plte || saw_idat {
                        return Err(nerror!(
                            ErrorKind::ColorProfileError,
                            "v2/compare {} PNG iCCP chunk appears after PLTE/IDAT",
                            role
                        ));
                    }
                    scan.iccp_count = scan.iccp_count.saturating_add(1);
                    if scan.iccp_count > 1 {
                        return Err(nerror!(
                            ErrorKind::ColorProfileError,
                            "v2/compare {} PNG contains duplicate iCCP chunks",
                            role
                        ));
                    }
                    let Some(nul) = data.iter().position(|&b| b == 0) else {
                        return Err(nerror!(
                            ErrorKind::ColorProfileError,
                            "v2/compare {} PNG iCCP chunk missing profile name terminator",
                            role
                        ));
                    };
                    if nul == 0 || nul > 79 || nul + 2 >= data.len() {
                        return Err(nerror!(
                            ErrorKind::ColorProfileError,
                            "v2/compare {} PNG iCCP chunk has invalid profile name or empty payload",
                            role
                        ));
                    }
                    if data[nul + 1] != 0 {
                        return Err(nerror!(
                            ErrorKind::ColorProfileError,
                            "v2/compare {} PNG iCCP chunk has unsupported compression method {}",
                            role,
                            data[nul + 1]
                        ));
                    }
                }
                b"IEND" => {
                    saw_iend = true;
                    break;
                }
                _ => {}
            }
            pos = crc_end;
        }
        if !saw_iend {
            return Err(nerror!(
                ErrorKind::ImageDecodingError,
                "v2/compare {} PNG stream missing IEND terminator",
                role
            ));
        }
        Ok(scan)
    }

    fn retained_probe_bytes(
        probed: &zencodecs::engine::ProbedInput<'_>,
        extra_details_bytes: u64,
    ) -> u64 {
        let mut bytes: u64 = 4096u64.saturating_add(extra_details_bytes);
        if let Some(info) = probed.inspection().image_info.as_ref() {
            if let Some(icc) = info.source_color.icc_profile.as_deref() {
                bytes = bytes.saturating_add(icc.len() as u64);
            }
            if let Some(exif) = info.embedded_metadata.exif.as_deref() {
                bytes = bytes.saturating_add(exif.len() as u64);
            }
            if let Some(xmp) = info.embedded_metadata.xmp.as_deref() {
                bytes = bytes.saturating_add(xmp.len() as u64);
            }
        }
        bytes
    }

    /// Zero-allocation raw ICC profile preflight that bounds every table `moxcms::ColorProfile::new_from_slice`
    /// can allocate (including `mAB`/`mBA` CLUTs where `moxcms 0.9.2` ignores `ParsingOptions::max_allowed_clut_size`,
    /// overlapping `mluc` localized string records, duplicate tag entries, TRC curves, and multi-process elements).
    ///
    /// Returns `(parse_scratch_bytes, transform_scratch_bytes)`.
    fn preflight_raw_icc(icc: &[u8], role: &str, stop: &DeadlineStop) -> Result<(u64, u64)> {
        const MAX_ICC_BYTES: usize = 4 * 1024 * 1024;
        const MAX_ICC_TAGS: usize = 256;
        const MAX_CLUT_POINTS: u64 = 262_144;
        const MAX_LUT_1D_ENTRIES: u64 = 131_072;
        const MAX_TRC_ENTRIES: u64 = 40_000;
        const MAX_MLUC_BYTES: u64 = 256 * 1024;

        if icc.len() < 132 || icc.len() > MAX_ICC_BYTES {
            return Err(nerror!(
                ErrorKind::ColorProfileError,
                "v2/compare {} ICC profile size {} B out of supported range (132..={})",
                role,
                icc.len(),
                MAX_ICC_BYTES
            ));
        }
        let tag_count = u32::from_be_bytes([icc[128], icc[129], icc[130], icc[131]]) as usize;
        if tag_count > MAX_ICC_TAGS {
            return Err(nerror!(
                ErrorKind::ColorProfileError,
                "v2/compare {} ICC profile has {} tags (max {})",
                role,
                tag_count,
                MAX_ICC_TAGS
            ));
        }
        let table_end = 132usize.checked_add(tag_count.saturating_mul(12)).ok_or_else(|| {
            nerror!(ErrorKind::ColorProfileError, "v2/compare {} ICC tag table overflow", role)
        })?;
        if table_end > icc.len() {
            return Err(nerror!(
                ErrorKind::ColorProfileError,
                "v2/compare {} ICC tag table truncated",
                role
            ));
        }

        let scan_nested_curves =
            |tag: &[u8], start_off: usize, count: usize, parsed: &mut u64| -> Result<()> {
                let mut off = start_off;
                for _ in 0..count {
                    let end12 = off.checked_add(12).ok_or_else(|| {
                        nerror!(
                            ErrorKind::ColorProfileError,
                            "v2/compare {} ICC nested curve offset overflow",
                            role
                        )
                    })?;
                    if end12 > tag.len() {
                        return Err(nerror!(
                            ErrorKind::ColorProfileError,
                            "v2/compare {} ICC nested curve truncated",
                            role
                        ));
                    }
                    let ctype = &tag[off..off + 4];
                    let curve_size = if ctype == b"curv" {
                        let n = u32::from_be_bytes([
                            tag[off + 8],
                            tag[off + 9],
                            tag[off + 10],
                            tag[off + 11],
                        ]) as u64;
                        if n > MAX_TRC_ENTRIES {
                            return Err(nerror!(
                                ErrorKind::ColorProfileError,
                                "v2/compare {} ICC nested TRC has {} entries (max {})",
                                role,
                                n,
                                MAX_TRC_ENTRIES
                            ));
                        }
                        let byte_len = (n as usize).saturating_mul(2);
                        *parsed = parsed.saturating_add(byte_len as u64);
                        12usize.saturating_add(byte_len)
                    } else if ctype == b"para" {
                        let ftype = u16::from_be_bytes([tag[off + 8], tag[off + 9]]) as usize;
                        const PARAM_COUNTS: [usize; 5] = [1, 3, 4, 5, 7];
                        let Some(&pc) = PARAM_COUNTS.get(ftype) else {
                            return Err(nerror!(
                                ErrorKind::ColorProfileError,
                                "v2/compare {} ICC nested parametric curve has invalid type {}",
                                role,
                                ftype
                            ));
                        };
                        *parsed = parsed.saturating_add(32);
                        12 + pc * 4
                    } else {
                        return Err(nerror!(
                            ErrorKind::ColorProfileError,
                            "v2/compare {} ICC nested curve has invalid type {:?}",
                            role,
                            ctype
                        ));
                    };
                    let next = off.checked_add(curve_size).ok_or_else(|| {
                        nerror!(
                            ErrorKind::ColorProfileError,
                            "v2/compare {} ICC nested curve size overflow",
                            role
                        )
                    })?;
                    if next > tag.len() {
                        return Err(nerror!(
                            ErrorKind::ColorProfileError,
                            "v2/compare {} ICC nested curve exceeds tag bounds",
                            role
                        ));
                    }
                    off = (next + 3) & !3;
                }
                Ok(())
            };

        let mut total_parsed_bytes: u64 = 0;
        let mut mluc_total_bytes: u64 = 0;
        let mut max_clut_points: u64 = 0;

        for idx in 0..tag_count {
            if idx & 63 == 0 {
                stop.check_flow()?;
            }
            let entry = &icc[132 + idx * 12..132 + (idx + 1) * 12];
            let sig = &entry[0..4];
            let off = u32::from_be_bytes([entry[4], entry[5], entry[6], entry[7]]) as usize;
            let size = u32::from_be_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize;

            if matches!(
                sig,
                b"D2B0" | b"D2B1" | b"D2B2" | b"D2B3" | b"B2D0" | b"B2D1" | b"B2D2" | b"B2D3"
            ) {
                return Err(nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "v2/compare refuses multi-process element ICC tag {:?} on {}",
                    sig,
                    role
                ));
            }
            let Some(end) = off.checked_add(size) else {
                return Err(nerror!(
                    ErrorKind::ColorProfileError,
                    "v2/compare {} ICC tag {:?} offset overflow",
                    role,
                    sig
                ));
            };
            if end > icc.len() {
                return Err(nerror!(
                    ErrorKind::ColorProfileError,
                    "v2/compare {} ICC tag {:?} out of bounds",
                    role,
                    sig
                ));
            }
            if size < 4 {
                continue;
            }
            let tag = &icc[off..end];
            let tag_type = &tag[0..4];
            if tag_type == b"mpet" {
                return Err(nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "v2/compare refuses mpet ICC tag on {}",
                    role
                ));
            }

            let is_lut_sig =
                matches!(sig, b"A2B0" | b"A2B1" | b"A2B2" | b"B2A0" | b"B2A1" | b"B2A2" | b"gamt");
            if is_lut_sig || matches!(tag_type, b"mft1" | b"mft2" | b"mAB " | b"mBA ") {
                match tag_type {
                    b"mft1" | b"mft2" => {
                        let min_hdr = if tag_type == b"mft2" { 52 } else { 48 };
                        if tag.len() < min_hdr {
                            return Err(nerror!(
                                ErrorKind::ColorProfileError,
                                "v2/compare {} ICC {:?} LUT header truncated",
                                role,
                                tag_type
                            ));
                        }
                        let in_ch = tag[8] as u64;
                        let out_ch = tag[9] as u64;
                        let grid = tag[10] as u64;
                        if !(1..=4).contains(&in_ch) || !(1..=4).contains(&out_ch) || grid < 2 {
                            return Err(nerror!(
                                ErrorKind::ColorProfileError,
                                "v2/compare {} ICC {:?} LUT has unsupported dims ({}->{}, grid={})",
                                role,
                                tag_type,
                                in_ch,
                                out_ch,
                                grid
                            ));
                        }
                        let grid_pow = grid.checked_pow(in_ch as u32).ok_or_else(|| {
                            nerror!(
                                ErrorKind::ColorProfileError,
                                "v2/compare {} ICC LUT grid overflow",
                                role
                            )
                        })?;
                        let clut_points = grid_pow.checked_mul(out_ch).ok_or_else(|| {
                            nerror!(
                                ErrorKind::ColorProfileError,
                                "v2/compare {} ICC LUT point count overflow",
                                role
                            )
                        })?;
                        if clut_points > MAX_CLUT_POINTS {
                            return Err(nerror!(
                                ErrorKind::ColorProfileError,
                                "v2/compare {} ICC {:?} CLUT has {} points (max {})",
                                role,
                                tag_type,
                                clut_points,
                                MAX_CLUT_POINTS
                            ));
                        }
                        let (in_entries, out_entries, entry_bytes): (u64, u64, u64) =
                            if tag_type == b"mft1" {
                                (256, 256, 1)
                            } else {
                                (
                                    u16::from_be_bytes([tag[48], tag[49]]) as u64,
                                    u16::from_be_bytes([tag[50], tag[51]]) as u64,
                                    2,
                                )
                            };
                        let io_entries = in_entries
                            .saturating_mul(in_ch)
                            .saturating_add(out_entries.saturating_mul(out_ch));
                        if io_entries > MAX_LUT_1D_ENTRIES {
                            return Err(nerror!(
                                ErrorKind::ColorProfileError,
                                "v2/compare {} ICC {:?} 1D LUT tables have {} entries (max {})",
                                role,
                                tag_type,
                                io_entries,
                                MAX_LUT_1D_ENTRIES
                            ));
                        }
                        let lut_bytes =
                            clut_points.saturating_add(io_entries).saturating_mul(entry_bytes);
                        total_parsed_bytes = total_parsed_bytes.saturating_add(lut_bytes);
                        max_clut_points = max_clut_points.max(clut_points);
                    }
                    b"mAB " | b"mBA " => {
                        if tag.len() < 48 {
                            return Err(nerror!(
                                ErrorKind::ColorProfileError,
                                "v2/compare {} ICC {:?} LUT header truncated",
                                role,
                                tag_type
                            ));
                        }
                        let in_ch = tag[8] as usize;
                        let out_ch = tag[9] as usize;
                        if !(1..=4).contains(&in_ch) || !(1..=4).contains(&out_ch) {
                            return Err(nerror!(
                                ErrorKind::ColorProfileError,
                                "v2/compare {} ICC {:?} LUT has unsupported channels {}->{}",
                                role,
                                tag_type,
                                in_ch,
                                out_ch
                            ));
                        }
                        let b_curve_off =
                            u32::from_be_bytes([tag[12], tag[13], tag[14], tag[15]]) as usize;
                        let m_curve_off =
                            u32::from_be_bytes([tag[20], tag[21], tag[22], tag[23]]) as usize;
                        let clut_off =
                            u32::from_be_bytes([tag[24], tag[25], tag[26], tag[27]]) as usize;
                        let a_curve_off =
                            u32::from_be_bytes([tag[28], tag[29], tag[30], tag[31]]) as usize;
                        if clut_off != 0 {
                            let clut_hdr_end = clut_off.checked_add(20).ok_or_else(|| {
                                nerror!(
                                    ErrorKind::ColorProfileError,
                                    "v2/compare {} ICC {:?} CLUT offset overflow",
                                    role,
                                    tag_type
                                )
                            })?;
                            if clut_hdr_end > tag.len() {
                                return Err(nerror!(
                                    ErrorKind::ColorProfileError,
                                    "v2/compare {} ICC {:?} CLUT header truncated",
                                    role,
                                    tag_type
                                ));
                            }
                            let mut clut_points: u64 = 1;
                            for &g in &tag[clut_off..clut_off + in_ch] {
                                if g < 2 {
                                    return Err(nerror!(
                                        ErrorKind::ColorProfileError,
                                        "v2/compare {} ICC {:?} CLUT grid dim {} < 2",
                                        role,
                                        tag_type,
                                        g
                                    ));
                                }
                                clut_points =
                                    clut_points.checked_mul(u64::from(g)).ok_or_else(|| {
                                        nerror!(
                                            ErrorKind::ColorProfileError,
                                            "v2/compare {} ICC {:?} CLUT grid overflow",
                                            role,
                                            tag_type
                                        )
                                    })?;
                                if clut_points > MAX_CLUT_POINTS {
                                    return Err(nerror!(
                                        ErrorKind::ColorProfileError,
                                        "v2/compare {} ICC {:?} CLUT exceeds {} max points",
                                        role,
                                        tag_type,
                                        MAX_CLUT_POINTS
                                    ));
                                }
                            }
                            clut_points =
                                clut_points.checked_mul(out_ch as u64).ok_or_else(|| {
                                    nerror!(
                                        ErrorKind::ColorProfileError,
                                        "v2/compare {} ICC {:?} CLUT point overflow",
                                        role,
                                        tag_type
                                    )
                                })?;
                            if clut_points > MAX_CLUT_POINTS {
                                return Err(nerror!(
                                    ErrorKind::ColorProfileError,
                                    "v2/compare {} ICC {:?} CLUT has {} points (max {})",
                                    role,
                                    tag_type,
                                    clut_points,
                                    MAX_CLUT_POINTS
                                ));
                            }
                            let entry_size = tag[clut_off + 16];
                            if entry_size != 1 && entry_size != 2 {
                                return Err(nerror!(
                                    ErrorKind::ColorProfileError,
                                    "v2/compare {} ICC {:?} CLUT invalid entry size {}",
                                    role,
                                    tag_type,
                                    entry_size
                                ));
                            }
                            let clut_bytes = clut_points.saturating_mul(u64::from(entry_size));
                            let clut_end =
                                (clut_hdr_end as u64).checked_add(clut_bytes).ok_or_else(|| {
                                    nerror!(
                                        ErrorKind::ColorProfileError,
                                        "v2/compare {} ICC {:?} CLUT byte end overflow",
                                        role,
                                        tag_type
                                    )
                                })?;
                            if clut_end > tag.len() as u64 {
                                return Err(nerror!(
                                    ErrorKind::ColorProfileError,
                                    "v2/compare {} ICC {:?} CLUT data truncated",
                                    role,
                                    tag_type
                                ));
                            }
                            total_parsed_bytes = total_parsed_bytes.saturating_add(clut_bytes);
                            max_clut_points = max_clut_points.max(clut_points);
                        }
                        let to_pcs = tag_type == b"mAB ";
                        if a_curve_off != 0 {
                            scan_nested_curves(
                                tag,
                                a_curve_off,
                                if to_pcs { in_ch } else { out_ch },
                                &mut total_parsed_bytes,
                            )?;
                        }
                        if m_curve_off != 0 {
                            scan_nested_curves(
                                tag,
                                m_curve_off,
                                if to_pcs { out_ch } else { in_ch },
                                &mut total_parsed_bytes,
                            )?;
                        }
                        if b_curve_off != 0 {
                            scan_nested_curves(
                                tag,
                                b_curve_off,
                                if to_pcs { out_ch } else { in_ch },
                                &mut total_parsed_bytes,
                            )?;
                        }
                    }
                    _ => {}
                }
            } else if matches!(sig, b"rTRC" | b"gTRC" | b"bTRC" | b"kTRC") && tag_type == b"curv" {
                if tag.len() < 12 {
                    return Err(nerror!(
                        ErrorKind::ColorProfileError,
                        "v2/compare {} ICC TRC tag truncated",
                        role
                    ));
                }
                let n = u32::from_be_bytes([tag[8], tag[9], tag[10], tag[11]]) as u64;
                if n > MAX_TRC_ENTRIES {
                    return Err(nerror!(
                        ErrorKind::ColorProfileError,
                        "v2/compare {} ICC TRC has {} entries (max {})",
                        role,
                        n,
                        MAX_TRC_ENTRIES
                    ));
                }
                total_parsed_bytes = total_parsed_bytes.saturating_add(n.saturating_mul(2));
            } else if matches!(sig, b"cprt" | b"desc" | b"vued" | b"dmdd" | b"dmnd" | b"targ") {
                match tag_type {
                    b"mluc" => {
                        if tag.len() < 16 {
                            return Err(nerror!(
                                ErrorKind::ColorProfileError,
                                "v2/compare {} ICC mluc tag truncated",
                                role
                            ));
                        }
                        let rec_count =
                            u32::from_be_bytes([tag[8], tag[9], tag[10], tag[11]]) as usize;
                        let rec_size =
                            u32::from_be_bytes([tag[12], tag[13], tag[14], tag[15]]) as usize;
                        let recs_end = 16usize
                            .checked_add(rec_count.saturating_mul(rec_size))
                            .ok_or_else(|| {
                                nerror!(
                                    ErrorKind::ColorProfileError,
                                    "v2/compare {} ICC mluc record table overflow",
                                    role
                                )
                            })?;
                        if rec_size < 12 || recs_end > tag.len() {
                            return Err(nerror!(
                                ErrorKind::ColorProfileError,
                                "v2/compare {} ICC mluc record table invalid",
                                role
                            ));
                        }
                        mluc_total_bytes =
                            mluc_total_bytes.saturating_add((rec_count as u64).saturating_mul(80));
                        for (r_idx, rec) in tag[16..recs_end].chunks_exact(rec_size).enumerate() {
                            if r_idx & 63 == 0 {
                                stop.check_flow()?;
                            }
                            let rec_len =
                                u32::from_be_bytes([rec[4], rec[5], rec[6], rec[7]]) as u64;
                            mluc_total_bytes =
                                mluc_total_bytes.saturating_add((rec_len / 2).saturating_mul(3));
                            if mluc_total_bytes > MAX_MLUC_BYTES {
                                return Err(nerror!(
                                    ErrorKind::ColorProfileError,
                                    "v2/compare {} ICC localized text exceeds {} B cap",
                                    role,
                                    MAX_MLUC_BYTES
                                ));
                            }
                        }
                        total_parsed_bytes = total_parsed_bytes.saturating_add(mluc_total_bytes);
                    }
                    b"text" | b"desc" => {
                        let est = (size as u64).saturating_mul(3);
                        total_parsed_bytes = total_parsed_bytes.saturating_add(est);
                    }
                    _ => {}
                }
            }
        }

        let parse_scratch = total_parsed_bytes.saturating_mul(2).saturating_add(256 * 1024);
        let transform_scratch = if max_clut_points > 0 {
            // `MoxCms` reparses the already-preflighted profile once, converts CLUT to f32,
            // and builds 1D + 3D interpolation tables (bounded by MAX_CLUT_POINTS = 262,144).
            parse_scratch
                .saturating_add(max_clut_points.saturating_mul(16))
                .saturating_add(4 * 1024 * 1024)
        } else {
            // Matrix-shaper: 1x profile parse + u8/u16 linearization & gamma LUTs.
            parse_scratch.saturating_add(2 * 1024 * 1024)
        };
        Ok((parse_scratch, transform_scratch))
    }

    /// Validate an ICC profile as SDR full-range RGB/Gray within the remaining memory budget,
    /// using zero-allocation raw ICC preflight + bounded [`moxcms::ParsingOptions`], and return
    /// a conservative scratch upper bound for building and running a single `MoxCms` transform.
    fn validate_icc_sdr_profile(
        icc: &[u8],
        role: &str,
        live_mem: u64,
        total_mem_cap: Option<u64>,
        stop: &DeadlineStop,
    ) -> Result<u64> {
        if let Err(e) = zenpixels::validate_icc_structure(icc) {
            return Err(nerror!(
                ErrorKind::ColorProfileError,
                "v2/compare {} has structurally invalid ICC profile: {}",
                role,
                e
            ));
        }
        let (parse_scratch, transform_scratch) = preflight_raw_icc(icc, role, stop)?;
        check_transient_mem(live_mem, parse_scratch, total_mem_cap, "ICC profile parse")?;
        let opts = moxcms::ParsingOptions {
            max_profile_size: icc.len().saturating_add(1),
            max_allowed_clut_size: 262_144,
            max_allowed_trc_size: 40_000,
        };
        let profile =
            moxcms::ColorProfile::new_from_slice_with_options(icc, opts).map_err(|e| {
                nerror!(
                    ErrorKind::ColorProfileError,
                    "v2/compare {} has invalid or over-budget ICC profile: {:?}",
                    role,
                    e
                )
            })?;
        match profile.color_space {
            moxcms::DataColorSpace::Rgb | moxcms::DataColorSpace::Gray => {}
            other => {
                return Err(nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "v2/compare {} ICC profile color space {:?} is unsupported for SDR comparison",
                    role,
                    other
                ));
            }
        }
        if let Some(cicp) = &profile.cicp {
            let tf = cicp.transfer_characteristics as u8;
            if tf == 16 || tf == 18 {
                return Err(nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "v2/compare refuses HDR ICC profile (CICP transfer {}) on {}",
                    tf,
                    role
                ));
            }
            if tf == 8 || tf == 2 || tf == 0 {
                return Err(nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "v2/compare refuses ambiguous/linear/unspecified ICC CICP transfer {} on {}",
                    tf,
                    role
                ));
            }
            if !cicp.full_range {
                return Err(nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "v2/compare requires full-range SDR color on {}",
                    role
                ));
            }
        }
        Ok(transform_scratch)
    }

    fn validate_probed_still_sdr(
        probed: &zencodecs::engine::ProbedInput<'_>,
        role: &str,
        live_mem: u64,
        total_mem_cap: Option<u64>,
        stop: &DeadlineStop,
    ) -> Result<(u32, u32)> {
        let insp = probed.inspection();
        let env = &insp.envelope;
        if env.kind != zencodecs::engine::MediaKind::Still {
            return Err(nerror!(
                ErrorKind::Category(ErrorCategory::ActionNotSupported),
                "v2/compare requires single-frame still SDR images ({} is {:?})",
                role,
                env.kind
            ));
        }
        if env.has_gain_map {
            return Err(nerror!(
                ErrorKind::Category(ErrorCategory::ActionNotSupported),
                "v2/compare refuses HDR gain-map image on {}",
                role
            ));
        }
        if let Some(info) = insp.image_info.as_ref() {
            if info.supplements.gain_map || info.supplements.auxiliary {
                return Err(nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "v2/compare refuses auxiliary/gain-map image on {}",
                    role
                ));
            }
            if !matches!(info.sequence, zc::ImageSequence::Single) {
                return Err(nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "v2/compare requires single-image still on {} (found {:?})",
                    role,
                    info.sequence
                ));
            }
            let sc = &info.source_color;
            if sc.content_light_level.is_some()
                || sc.mastering_display.is_some()
                || sc.diffuse_white.is_some()
            {
                return Err(nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "v2/compare refuses HDR mastering/content-light/diffuse-white metadata on {}",
                    role
                ));
            }
            if let Some(cicp) = sc.cicp {
                if cicp.transfer_characteristics == 16 || cicp.transfer_characteristics == 18 {
                    return Err(nerror!(
                        ErrorKind::Category(ErrorCategory::ActionNotSupported),
                        "v2/compare refuses HDR CICP transfer {} on {}",
                        cicp.transfer_characteristics,
                        role
                    ));
                }
                if cicp.transfer_characteristics == 8 {
                    return Err(nerror!(
                        ErrorKind::Category(ErrorCategory::ActionNotSupported),
                        "v2/compare refuses ambiguous linear-light CICP transfer on {}",
                        role
                    ));
                }
                if !cicp.full_range {
                    return Err(nerror!(
                        ErrorKind::Category(ErrorCategory::ActionNotSupported),
                        "v2/compare requires full-range SDR signal on {}",
                        role
                    ));
                }
            }
            if let Some(icc) = sc.icc_profile.as_deref().filter(|b| !b.is_empty()) {
                let _ = validate_icc_sdr_profile(icc, role, live_mem, total_mem_cap, stop)?;
            }
        }
        let (uw, uh) = env.orientation.output_dimensions(env.width, env.height);
        if uw == 0 || uh == 0 {
            return Err(nerror!(
                ErrorKind::ImageDecodingError,
                "v2/compare {} has zero dimensions {}x{}",
                role,
                uw,
                uh
            ));
        }
        Ok((uw, uh))
    }

    fn slice_contains(haystack: &[u8], needle: &[u8]) -> bool {
        !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
    }

    /// Zero-allocation, cooperatively stop-aware single-frame SDR JPEG container verifier.
    ///
    /// Walks top-level JPEG markers and entropy-coded SOS scan bodies in <=64 KiB strides, polling
    /// `check_stop` inside entropy scans, fill runs, XMP segments, and trailing padding. Refuses
    /// Ultra HDR (ISO 21496-1 APP2, XMP `hdrgm:` / GContainer `GainMap`), MPF APP2, multi-image
    /// / nested SOI, truncated scans (missing EOI), and any non-zero trailing bytes after EOI.
    ///
    /// Returns the byte offset of the first `SOS` marker (strictly before any entropy scan data).
    fn scan_raw_jpeg_single_sdr(
        bytes: &[u8],
        role: &str,
        check_stop: &mut impl FnMut() -> Result<()>,
    ) -> Result<usize> {
        const POLL_STRIDE: usize = 64 * 1024;
        const XMP_PREFIX: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
        const EXT_XMP_PREFIX: &[u8] = b"http://ns.adobe.com/xmp/extension/\0";

        if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
            return Err(nerror!(
                ErrorKind::ImageDecodingError,
                "v2/compare {} JPEG missing SOI header",
                role
            ));
        }

        let mut pos = 2usize;
        let mut saw_sof = false;
        let mut saw_eoi = false;
        let mut first_sos_offset: Option<usize> = None;
        let mut marker_idx = 0usize;

        while pos < bytes.len() {
            if marker_idx & 63 == 0 {
                check_stop()?;
            }
            marker_idx = marker_idx.saturating_add(1);

            let marker_start = pos;
            if bytes[pos] != 0xFF {
                return Err(nerror!(
                    ErrorKind::ImageDecodingError,
                    "v2/compare {} JPEG expected marker prefix 0xFF at offset {}",
                    role,
                    pos
                ));
            }
            let mut fill_count = 0usize;
            while pos < bytes.len() && bytes[pos] == 0xFF {
                pos += 1;
                fill_count += 1;
                if fill_count >= POLL_STRIDE {
                    check_stop()?;
                    fill_count = 0;
                }
            }
            if pos >= bytes.len() {
                return Err(nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "v2/compare refuses truncated JPEG on {} (missing EOI)",
                    role
                ));
            }

            let marker = bytes[pos];
            pos += 1;
            match marker {
                0xD9 => {
                    if first_sos_offset.is_none() {
                        first_sos_offset = Some(marker_start);
                    }
                    saw_eoi = true;
                    break;
                }
                0xD8 => {
                    return Err(nerror!(
                        ErrorKind::Category(ErrorCategory::ActionNotSupported),
                        "v2/compare refuses multi-image / nested-SOI JPEG on {}",
                        role
                    ));
                }
                0x00 => {
                    return Err(nerror!(
                        ErrorKind::ImageDecodingError,
                        "v2/compare {} JPEG unexpected stuffed byte 0xFF00 outside scan",
                        role
                    ));
                }
                0x01 | 0xD0..=0xD7 => {
                    // Standalone TEM or RSTn marker (0-byte payload).
                }
                _ => {
                    if matches!(
                        marker,
                        0xC0 | 0xC1
                            | 0xC2
                            | 0xC3
                            | 0xC5
                            | 0xC6
                            | 0xC7
                            | 0xC9
                            | 0xCA
                            | 0xCB
                            | 0xCD
                            | 0xCE
                            | 0xCF
                    ) {
                        saw_sof = true;
                    }
                    if marker == 0xDA && first_sos_offset.is_none() {
                        first_sos_offset = Some(marker_start);
                    }
                    if pos + 2 > bytes.len() {
                        return Err(nerror!(
                            ErrorKind::Category(ErrorCategory::ActionNotSupported),
                            "v2/compare refuses truncated JPEG marker 0xFF{:02X} on {}",
                            marker,
                            role
                        ));
                    }
                    let seg_len = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
                    if seg_len < 2 || pos + seg_len > bytes.len() {
                        return Err(nerror!(
                            ErrorKind::Category(ErrorCategory::ActionNotSupported),
                            "v2/compare refuses truncated/invalid JPEG segment 0xFF{:02X} on {}",
                            marker,
                            role
                        ));
                    }
                    let payload = &bytes[pos + 2..pos + seg_len];
                    if marker == 0xE2 {
                        if payload.starts_with(b"MPF\0") {
                            return Err(nerror!(
                                ErrorKind::Category(ErrorCategory::ActionNotSupported),
                                "v2/compare refuses multi-image / MPO / MPF JPEG on {}",
                                role
                            ));
                        }
                        if payload.starts_with(zc::ISO_21496_1_URN) {
                            return Err(nerror!(
                                ErrorKind::Category(ErrorCategory::ActionNotSupported),
                                "v2/compare refuses Ultra HDR / ISO 21496-1 gain-map JPEG on {}",
                                role
                            ));
                        }
                        if saw_sof && payload.starts_with(b"ICC_PROFILE\0") {
                            return Err(nerror!(
                                ErrorKind::Category(ErrorCategory::ActionNotSupported),
                                "v2/compare refuses JPEG with post-SOF ICC_PROFILE segment on {}",
                                role
                            ));
                        }
                    } else if marker == 0xE1 {
                        if saw_sof && payload.starts_with(b"Exif\0\0") {
                            return Err(nerror!(
                                ErrorKind::Category(ErrorCategory::ActionNotSupported),
                                "v2/compare refuses JPEG with post-SOF EXIF segment on {}",
                                role
                            ));
                        }
                        if payload.starts_with(XMP_PREFIX) || payload.starts_with(EXT_XMP_PREFIX) {
                            check_stop()?;
                            let xmp = if payload.starts_with(XMP_PREFIX) {
                                &payload[XMP_PREFIX.len()..]
                            } else {
                                &payload[EXT_XMP_PREFIX.len()..]
                            };
                            if slice_contains(xmp, b"hdrgm:Version")
                                || slice_contains(xmp, b"hdrgm:GainMapMax")
                                || slice_contains(xmp, b"hdr-gain-map")
                                || slice_contains(xmp, b"Item:Semantic=\"GainMap\"")
                                || slice_contains(xmp, b"Item:Semantic='GainMap'")
                            {
                                return Err(nerror!(
                                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                                    "v2/compare refuses Ultra HDR / XMP gain-map JPEG on {}",
                                    role
                                ));
                            }
                        }
                    }
                    pos += seg_len;

                    // Start of Scan (SOS): cooperatively scan entropy-coded data in <=64 KiB strides!
                    if marker == 0xDA {
                        let mut scanned_since_poll = 0usize;
                        loop {
                            if scanned_since_poll >= POLL_STRIDE {
                                check_stop()?;
                                scanned_since_poll = 0;
                            }
                            if pos >= bytes.len() {
                                return Err(nerror!(
                                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                                    "v2/compare refuses truncated JPEG scan on {} (missing EOI)",
                                    role
                                ));
                            }
                            let step_end = bytes
                                .len()
                                .min(pos.saturating_add(POLL_STRIDE - scanned_since_poll));
                            let slice = &bytes[pos..step_end];
                            if let Some(rel_ff) = slice.iter().position(|&b| b == 0xFF) {
                                let ff_pos = pos + rel_ff;
                                scanned_since_poll += rel_ff;
                                let mut next_pos = ff_pos + 1;
                                while next_pos < bytes.len() && bytes[next_pos] == 0xFF {
                                    next_pos += 1;
                                    scanned_since_poll += 1;
                                    if scanned_since_poll >= POLL_STRIDE {
                                        check_stop()?;
                                        scanned_since_poll = 0;
                                    }
                                }
                                if next_pos >= bytes.len() {
                                    return Err(nerror!(
                                        ErrorKind::Category(ErrorCategory::ActionNotSupported),
                                        "v2/compare refuses truncated JPEG scan on {} (missing EOI)",
                                        role
                                    ));
                                }
                                let next_byte = bytes[next_pos];
                                if next_byte == 0x00 || (0xD0..=0xD7).contains(&next_byte) {
                                    pos = next_pos + 1;
                                    scanned_since_poll += 2;
                                    continue;
                                }
                                pos = next_pos - 1;
                                break;
                            } else {
                                scanned_since_poll += slice.len();
                                pos = step_end;
                            }
                        }
                    }
                }
            }
        }

        if !saw_eoi {
            return Err(nerror!(
                ErrorKind::Category(ErrorCategory::ActionNotSupported),
                "v2/compare refuses truncated JPEG on {} (missing EOI)",
                role
            ));
        }

        // Reject any non-zero trailing bytes after EOI (e.g. concatenated or truncated second JPEG).
        let tail = &bytes[pos..];
        for chunk in tail.chunks(POLL_STRIDE) {
            check_stop()?;
            if chunk.iter().any(|&b| b != 0) {
                return Err(nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "v2/compare refuses JPEG with trailing non-zero bytes after EOI on {} ({} trailing B)",
                    role,
                    tail.len()
                ));
            }
        }
        Ok(first_sos_offset.unwrap_or(pos))
    }

    enum CompareProbedStill<'a> {
        Engine(zencodecs::engine::ProbedInput<'a>),
        #[cfg(feature = "zen-codecs")]
        Jpeg {
            bytes: &'a [u8],
            info: zenjpeg::decoder::JpegInfo,
        },
    }

    fn decoded_retained_icc_bytes(buf: &zenpixels::PixelBuf) -> u64 {
        match &buf.format().color {
            zenpixels::ColorSpace::Icc { profile, .. } => profile.len() as u64,
            _ => 0,
        }
    }

    #[cfg(feature = "zen-codecs")]
    fn map_zenjpeg_error(
        err: zenjpeg::decoder::Error,
        role: &str,
        stage: &str,
    ) -> crate::FlowError {
        use zenjpeg::decoder::ErrorKind as Jk;
        match err.kind() {
            Jk::Cancelled(_) => nerror!(
                ErrorKind::Category(ErrorCategory::OperationCancelled),
                "v2/compare {} JPEG {} cancelled: {}",
                role,
                stage,
                err
            ),
            Jk::ResourceLimitExceeded { .. }
            | Jk::AllocationFailed { .. }
            | Jk::ImageTooLarge { .. } => nerror!(
                ErrorKind::SizeLimitExceeded,
                "v2/compare {} JPEG {} exceeded resource limit: {}",
                role,
                stage,
                err
            ),
            Jk::UnsupportedFeature { .. }
            | Jk::UnsupportedOperation(_)
            | Jk::UnsupportedPixelFormat { .. }
            | Jk::UnsupportedPixelDescriptor { .. } => nerror!(
                ErrorKind::Category(ErrorCategory::ActionNotSupported),
                "v2/compare {} JPEG {} unsupported: {}",
                role,
                stage,
                err
            ),
            _ => nerror!(
                ErrorKind::ImageDecodingError,
                "v2/compare {} JPEG {} failed: {}",
                role,
                stage,
                err
            ),
        }
    }

    /// Header-only JPEG probe for `v2/compare` that operates strictly on `&bytes[..first_sos_offset]`
    /// via `zenjpeg::decoder::Decoder::read_info`, skipping optional source-encoder fingerprinting
    /// (`zenjpeg::detect::probe`) so progressive entropy scans are NEVER walked without cancellation.
    #[cfg(feature = "zen-codecs")]
    #[allow(clippy::too_many_arguments)]
    fn probe_jpeg_header_without_fingerprint<'a>(
        bytes: &'a [u8],
        first_sos_offset: usize,
        services: &zenpipe::canonical::HostServices,
        live_mem: &mut u64,
        total_mem_cap: Option<u64>,
        stop: &DeadlineStop,
        role: &str,
    ) -> Result<(CompareProbedStill<'a>, u32, u32, u64)> {
        let header_slice = &bytes[..first_sos_offset.min(bytes.len())];
        let header_len = header_slice.len() as u64;
        let max_segments = header_len / 4;
        let seg_entry = std::mem::size_of::<zenjpeg::decoder::PreservedSegment>() as u64;
        let jpeg_probe_bound = max_segments
            .saturating_mul(3)
            .saturating_mul(seg_entry)
            .saturating_add(max_segments.saturating_mul(32))
            .saturating_add(header_len.saturating_mul(6))
            .saturating_add(512 * 1024);
        check_transient_mem(*live_mem, jpeg_probe_bound, total_mem_cap, "JPEG header probe")?;
        stop.check_flow()?;

        let mut dec = zenjpeg::decoder::Decoder::new();
        if let Some(lim) = services.limits
            && let Some(mp) = lim.max_pixels
        {
            dec = dec.max_pixels(mp);
        }
        if let Some(cap) = total_mem_cap {
            dec = dec.max_memory(cap.saturating_sub(*live_mem));
        }

        let info =
            dec.read_info(header_slice).map_err(|e| map_zenjpeg_error(e, role, "header probe"))?;
        stop.check_flow()?;

        if matches!(
            info.color_space,
            zenjpeg::decoder::ColorSpace::Cmyk
                | zenjpeg::decoder::ColorSpace::Ycck
                | zenjpeg::decoder::ColorSpace::Unknown
        ) {
            return Err(nerror!(
                ErrorKind::Category(ErrorCategory::ActionNotSupported),
                "v2/compare refuses unsupported JPEG color space {:?} on {}",
                info.color_space,
                role
            ));
        }
        // Restrict direct JPEG decode to 8-bit 1- or 3-component non-lossless JPEGs so
        // `Decoder::decode_into` ALWAYS stays on the direct `preserve = None` zero-fingerprint path.
        if info.precision != 8
            || info.mode == zenjpeg::decoder::JpegMode::Lossless
            || !matches!(info.num_components, 1 | 3)
        {
            return Err(nerror!(
                ErrorKind::Category(ErrorCategory::ActionNotSupported),
                "v2/compare refuses unsupported JPEG frame (precision={}, mode={:?}, components={}) on {}",
                info.precision,
                info.mode,
                info.num_components,
                role
            ));
        }

        let orient =
            info.exif.as_deref().and_then(zc::helpers::parse_exif_orientation).unwrap_or_default();
        let (raw_w, raw_h) = (info.dimensions.width, info.dimensions.height);
        let (uw, uh) = orient.output_dimensions(raw_w, raw_h);
        if uw == 0 || uh == 0 {
            return Err(nerror!(
                ErrorKind::ImageDecodingError,
                "v2/compare {} JPEG has zero dimensions {}x{}",
                role,
                uw,
                uh
            ));
        }
        let channels: u64 = if info.num_components == 1 { 1 } else { 3 };
        let raw_output_bytes = u64::from(uw).saturating_mul(u64::from(uh)).saturating_mul(channels);
        if let Some(lim) = services.limits {
            if let Some(mw) = lim.max_width
                && (raw_w > mw || uw > mw)
            {
                return Err(nerror!(
                    ErrorKind::SizeLimitExceeded,
                    "v2/compare {} JPEG width exceeds max_width ({})",
                    role,
                    mw
                ));
            }
            if let Some(mh) = lim.max_height
                && (raw_h > mh || uh > mh)
            {
                return Err(nerror!(
                    ErrorKind::SizeLimitExceeded,
                    "v2/compare {} JPEG height exceeds max_height ({})",
                    role,
                    mh
                ));
            }
            if let Some(mp) = lim.max_pixels
                && u64::from(uw).saturating_mul(u64::from(uh)) > mp
            {
                return Err(nerror!(
                    ErrorKind::SizeLimitExceeded,
                    "v2/compare {} JPEG pixels {}x{} exceed max_pixels ({})",
                    role,
                    uw,
                    uh,
                    mp
                ));
            }
            if let Some(max_out) = lim.max_output_bytes
                && raw_output_bytes > max_out
            {
                return Err(nerror!(
                    ErrorKind::SizeLimitExceeded,
                    "v2/compare {} JPEG decoded raster ({} B) exceeds max_output_bytes ({} B)",
                    role,
                    raw_output_bytes,
                    max_out
                ));
            }
        }

        let retained = 4096u64
            .saturating_add(info.icc_profile.as_ref().map_or(0, |v| v.len() as u64))
            .saturating_add(info.exif.as_ref().map_or(0, |v| v.len() as u64))
            .saturating_add(info.xmp.as_ref().map_or(0, |s| s.len() as u64));
        check_transient_mem(*live_mem, retained, total_mem_cap, "retained probe metadata")?;
        *live_mem = live_mem.saturating_add(retained);

        if let Some(icc) = info.icc_profile.as_deref().filter(|b| !b.is_empty()) {
            let _ = validate_icc_sdr_profile(icc, role, *live_mem, total_mem_cap, stop)?;
        }

        Ok((CompareProbedStill::Jpeg { bytes, info }, uw, uh, retained))
    }

    /// Pre-admit header probe workspace against the combined job budget, run bounded stop-aware
    /// container discovery for JPEG (Ultra HDR / MPO / truncated trailing image refusal, skipping
    /// uninterruptible source fingerprinting) and raw PNG chunk validation (corrupt/duplicate/
    /// misplaced `iCCP`, text probe bounds, and strict compressed-text refusal), probe headers,
    /// and charge retained probe metadata.
    #[allow(clippy::too_many_arguments)]
    fn admit_and_probe_still<'a>(
        engine: &zencodecs::engine::Engine,
        bytes: &'a [u8],
        services: &zenpipe::canonical::HostServices,
        live_mem: &mut u64,
        total_mem_cap: Option<u64>,
        stop: &DeadlineStop,
        pulse: &zc::SharedPulse,
        role: &str,
    ) -> Result<(CompareProbedStill<'a>, u32, u32, u64)> {
        use zencodecs::engine::{Run, WorkspacePolicy};

        let encoded_len = bytes.len() as u64;
        let mut raw_png_has_iccp = false;
        let mut extra_retained_details_bytes: u64 = 0;

        if bytes.starts_with(&[0xFF, 0xD8]) {
            #[cfg(feature = "zen-codecs")]
            {
                let first_sos_offset =
                    scan_raw_jpeg_single_sdr(bytes, role, &mut || stop.check_flow())?;
                return probe_jpeg_header_without_fingerprint(
                    bytes,
                    first_sos_offset,
                    services,
                    live_mem,
                    total_mem_cap,
                    stop,
                    role,
                );
            }
            #[cfg(not(feature = "zen-codecs"))]
            {
                return Err(nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "v2/compare JPEG verification on {} requires feature 'zen-codecs'",
                    role
                ));
            }
        } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            let scan = scan_raw_png_chunks(bytes, role, stop)?;
            if scan.has_actl {
                return Err(nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "v2/compare refuses animated PNG (acTL) on {}",
                    role
                ));
            }
            if scan.has_compressed_text && services.workspace_policy == WorkspacePolicy::Strict {
                return Err(nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "v2/compare Strict policy refuses PNG with compressed text chunks on {} (unbounded probe inflate)",
                    role
                ));
            }
            raw_png_has_iccp = scan.iccp_count > 0;
            // `zenpng::probe_inner` allocates:
            // - up to 2 MiB for iCCP (1 MiB inflate + 1 MiB clone into PngInfo)
            // - up to 10x Latin-1 tEXt bytes (2x UTF-8 in text_chunks + 2x in creating_tool, both
            //   cloned in build_png_info, plus detect::probe creating_tool)
            // - up to 5x iTXt payload bytes (xmp/creating_tool + clones + detect::probe, including
            //   compression-flagged Software/Creator iTXt whose raw UTF-8 text detect::probe retains)
            // - under BestEffort: zTXt inflate (1 MiB scratch + 4 MiB/chunk up to 256 chunks) and
            //   compressed iTXt XMP (4 MiB inflate + 4 MiB clone)
            let iccp_buf: u64 = if raw_png_has_iccp { 2 * 1024 * 1024 } else { 0 };
            let text_probe_bytes = scan
                .text_bytes
                .saturating_mul(10)
                .saturating_add((scan.text_chunks.min(256) as u64).saturating_mul(128))
                .saturating_add(scan.itxt_total_bytes.saturating_mul(5))
                .saturating_add(scan.exif_bytes.saturating_mul(2));
            let compressed_text_probe_bytes = if scan.has_compressed_text {
                let ztxt_bytes = if scan.ztxt_count > 0 {
                    (1024 * 1024u64).saturating_add(
                        (scan.ztxt_count.min(256) as u64).saturating_mul(4 * 1024 * 1024),
                    )
                } else {
                    0
                };
                let itxt_bytes = if scan.has_compressed_itxt { 8 * 1024 * 1024 } else { 0 };
                ztxt_bytes.saturating_add(itxt_bytes)
            } else {
                0
            };
            let png_probe_bound = iccp_buf
                .saturating_add(text_probe_bytes)
                .saturating_add(compressed_text_probe_bytes)
                .saturating_add(encoded_len.saturating_mul(2))
                .saturating_add(256 * 1024);
            check_transient_mem(*live_mem, png_probe_bound, total_mem_cap, "PNG header probe")?;
            // Charge `PngProbe::creating_tool` retained in `info.source_encoding_details`
            // (including compression-flagged iTXt payloads, which zenpng::detect::probe retains).
            extra_retained_details_bytes =
                scan.text_bytes.saturating_mul(2).saturating_add(scan.itxt_total_bytes);
        } else if bytes.starts_with(b"BM") {
            check_transient_mem(*live_mem, 64 * 1024, total_mem_cap, "BMP header probe")?;
        } else if services.workspace_policy == WorkspacePolicy::Strict {
            return Err(nerror!(
                ErrorKind::Category(ErrorCategory::ActionNotSupported),
                "v2/compare Strict policy refuses header probe on {} without a proven workspace upper bound (use workspace_policy='best_effort')",
                role
            ));
        } else {
            let est_probe = encoded_len.saturating_mul(2).saturating_add(256 * 1024);
            check_transient_mem(*live_mem, est_probe, total_mem_cap, "still header probe")?;
        }

        let mut run =
            Run::new().with_workspace_policy(services.workspace_policy).with_pulse(pulse.clone());
        if let Some(mut lim) = services.limits {
            if let Some(cap) = total_mem_cap {
                lim = lim.with_max_memory(cap.saturating_sub(*live_mem));
            }
            run = run.with_limits(lim);
        }
        if let Some(ref cs) = services.color_services {
            run = run.with_color_services(cs.clone());
        }

        let probed = engine.probe(bytes, &run).map_err(crate::media::map_engine_error)?;

        // Catch malformed PNG iCCP silently discarded by zenpng::parse_iccp!
        if raw_png_has_iccp {
            let retained_icc = probed
                .inspection()
                .image_info
                .as_ref()
                .and_then(|i| i.source_color.icc_profile.as_deref())
                .filter(|b| !b.is_empty());
            if retained_icc.is_none() {
                return Err(nerror!(
                    ErrorKind::ColorProfileError,
                    "v2/compare {} PNG iCCP chunk was corrupt or discarded during probe",
                    role
                ));
            }
        }

        let retained = retained_probe_bytes(&probed, extra_retained_details_bytes);
        check_transient_mem(*live_mem, retained, total_mem_cap, "retained probe metadata")?;
        *live_mem = live_mem.saturating_add(retained);

        let (uw, uh) = validate_probed_still_sdr(&probed, role, *live_mem, total_mem_cap, stop)?;
        Ok((CompareProbedStill::Engine(probed), uw, uh, retained))
    }

    #[allow(clippy::too_many_arguments)]
    fn decode_compare_still(
        engine: &zencodecs::engine::Engine,
        probed: CompareProbedStill<'_>,
        needs: zencodecs::engine::DecodeNeeds,
        run: &zencodecs::engine::Run<'_>,
        services: &zenpipe::canonical::HostServices,
        live_mem: u64,
        total_mem_cap: Option<u64>,
        stop: &DeadlineStop,
        role: &str,
    ) -> Result<zenpixels::PixelBuf> {
        match probed {
            CompareProbedStill::Engine(probed) => decode_probed_still(engine, probed, needs, run)
                .map(zencodecs::DecodeOutput::into_buffer),
            #[cfg(feature = "zen-codecs")]
            CompareProbedStill::Jpeg { bytes, mut info } => {
                use zenjpeg::decoder::{JpegMode, PixelFormat as Jpf};
                use zenpixels::{ColorSpace, Orientation, PixelBuf, PixelFormat, SampleLayout};

                stop.check_flow()?;
                let orient = info
                    .exif
                    .as_deref()
                    .and_then(zc::helpers::parse_exif_orientation)
                    .unwrap_or_default();
                let (w, h) = (info.dimensions.width, info.dimensions.height);
                let (uw, uh) = orient.output_dimensions(w, h);
                let channels: usize = if info.num_components == 1 { 1 } else { 3 };
                let out_bytes =
                    u64::from(uw).saturating_mul(u64::from(uh)).saturating_mul(channels as u64);
                if let Some(max_out) = services.limits.and_then(|l| l.max_output_bytes)
                    && out_bytes > max_out
                {
                    return Err(nerror!(
                        ErrorKind::SizeLimitExceeded,
                        "v2/compare {} JPEG decoded raster ({} B) exceeds max_output_bytes ({} B)",
                        role,
                        out_bytes,
                        max_out
                    ));
                }
                let is_prog =
                    matches!(info.mode, JpegMode::Progressive | JpegMode::ArithmeticProgressive);
                let coeff_bytes = if is_prog {
                    u64::from(w)
                        .saturating_mul(u64::from(h))
                        .saturating_mul(u64::from(info.num_components.max(1)))
                        .saturating_mul(2)
                } else {
                    u64::from(w).saturating_mul(64).saturating_mul(4)
                };
                let orient_bytes = if orient == Orientation::Identity { 0 } else { out_bytes };
                // Account for the overlapping ICC profile allocations during direct JPEG decode:
                // - `JpegParser::with_strictness` calls `extract_icc_profile(bytes)` even with
                //   `preserve_no_metadata()` (allocating a second owned copy + chunk reassembly
                //   alongside `info.icc_profile` already charged in `live_mem`).
                // - `pixel_format_for_decoded_pixels` then allocates the `ColorSpace::Icc`
                //   `Arc<[u8]>` while `source_color` still holds the moved profile.
                let icc_bytes = info
                    .icc_profile
                    .as_deref()
                    .filter(|b| !b.is_empty())
                    .map_or(0u64, |b| b.len() as u64);
                let parser_icc_bytes = icc_bytes.saturating_mul(2);
                let decode_bound = out_bytes
                    .saturating_add(coeff_bytes)
                    .saturating_add(orient_bytes)
                    .saturating_add(parser_icc_bytes)
                    .saturating_add(512 * 1024);
                check_transient_mem(
                    live_mem,
                    decode_bound,
                    total_mem_cap,
                    "JPEG decode workspace",
                )?;

                let raw_len = usize::try_from(out_bytes).map_err(|_| {
                    nerror!(ErrorKind::SizeLimitExceeded, "v2/compare JPEG buffer size overflow")
                })?;
                let mut raw_pixels = Vec::new();
                raw_pixels.try_reserve_exact(raw_len).map_err(|_| {
                    nerror!(
                        ErrorKind::AllocationFailed,
                        "v2/compare {} JPEG output buffer allocation failed",
                        role
                    )
                })?;
                raw_pixels.resize(raw_len, 0u8);

                // Use `decode_into` with `auto_orient(false)` + `preserve_no_metadata()` so
                // `zenjpeg` stays on its direct `preserve = None` path: ZERO late marker copies,
                // ZERO `finalize_extras` / `detect::probe` uninterruptible entropy scans, and
                // cooperative `stop` polling inside `read_header_with_stop`, `parser.decode`,
                // AND pixel reconstruction (`write_pixels_into` / `fast_integer_into`).
                let mut dec =
                    zenjpeg::decoder::Decoder::new().auto_orient(false).preserve_no_metadata();
                if let Some(lim) = services.limits
                    && let Some(mp) = lim.max_pixels
                {
                    dec = dec.max_pixels(mp);
                }
                if let Some(cap) = total_mem_cap {
                    dec = dec.max_memory(cap.saturating_sub(
                        live_mem.saturating_add(out_bytes).saturating_add(parser_icc_bytes),
                    ));
                }
                let jpf = if channels == 1 { Jpf::Gray } else { Jpf::Rgb };
                let written = dec
                    .decode_into(bytes, jpf, &mut raw_pixels, stop.clone())
                    .map_err(|e| map_zenjpeg_error(e, role, "decode"))?;
                stop.check_flow()?;
                if written != raw_len {
                    return Err(nerror!(
                        ErrorKind::ImageDecodingError,
                        "v2/compare {} JPEG decode_into wrote {} B, expected {} B",
                        role,
                        written,
                        raw_len
                    ));
                }

                // Apply EXIF orientation cooperatively (polling `stop` every 64 rows) so we never
                // leave `decode_into`'s zero-fingerprint direct path.
                let oriented_pixels = if orient == Orientation::Identity {
                    raw_pixels
                } else {
                    let mut dst = Vec::new();
                    dst.try_reserve_exact(raw_len).map_err(|_| {
                        nerror!(
                            ErrorKind::AllocationFailed,
                            "v2/compare {} JPEG orientation buffer allocation failed",
                            role
                        )
                    })?;
                    dst.resize(raw_len, 0u8);
                    let w_us = w as usize;
                    let h_us = h as usize;
                    let uw_us = uw as usize;
                    for sy in 0..h_us {
                        if sy & 63 == 0 {
                            stop.check_flow()?;
                        }
                        for sx in 0..w_us {
                            let (dx, dy) = match orient {
                                Orientation::Identity => (sx, sy),
                                Orientation::FlipH => (w_us - 1 - sx, sy),
                                Orientation::Rotate180 => (w_us - 1 - sx, h_us - 1 - sy),
                                Orientation::FlipV => (sx, h_us - 1 - sy),
                                Orientation::Transpose => (sy, sx),
                                Orientation::Rotate90 => (h_us - 1 - sy, sx),
                                Orientation::Transverse => (h_us - 1 - sy, w_us - 1 - sx),
                                Orientation::Rotate270 => (sy, w_us - 1 - sx),
                                _ => {
                                    return Err(nerror!(
                                        ErrorKind::Category(ErrorCategory::ActionNotSupported),
                                        "v2/compare {} JPEG unsupported orientation {:?}",
                                        role,
                                        orient
                                    ));
                                }
                            };
                            let src_off = (sy * w_us + sx) * channels;
                            let dst_off = (dy * uw_us + dx) * channels;
                            dst[dst_off..dst_off + channels]
                                .copy_from_slice(&raw_pixels[src_off..src_off + channels]);
                        }
                    }
                    dst
                };
                stop.check_flow()?;

                // Match `zenjpeg`'s exact `zencodec` descriptor policy (including
                // `RenderingIntent::RelativeColorimetric` on non-XYB ICC profiles and sRGB on
                // decoder-inverted XYB JPEGs) by delegating directly to `zencodec::helpers`.
                // Move `info.icc_profile` into `source_color` (avoiding an extra copy) and admit
                // the overlapping `ColorSpace::Icc` allocation made inside
                // `pixel_format_for_decoded_pixels` before constructing the descriptor.
                check_transient_mem(
                    live_mem,
                    out_bytes.saturating_add(icc_bytes),
                    total_mem_cap,
                    "JPEG color descriptor ICC",
                )?;
                let layout = if channels == 1 { SampleLayout::GRAY8 } else { SampleLayout::RGB8 };
                let mut source_color = zc::SourceColor::default();
                if let Some(icc) = info.icc_profile.take().filter(|b| !b.is_empty()) {
                    source_color = source_color.with_icc_profile(icc);
                }
                let corrected_to: Option<ColorSpace> =
                    info.is_xyb.then_some(PixelFormat::RGB8_SRGB.color);
                let pixel_fmt = zc::helpers::pixel_format_for_decoded_pixels(
                    layout,
                    &source_color,
                    corrected_to.as_ref(),
                )
                .map_err(|e| {
                    nerror!(
                        ErrorKind::ColorProfileError,
                        "v2/compare {} JPEG color descriptor resolution failed: {}",
                        role,
                        e
                    )
                })?;
                drop(source_color);

                let buf =
                    PixelBuf::from_bytes(oriented_pixels, uw, uh, pixel_fmt).map_err(|e| {
                        nerror!(
                            ErrorKind::InternalError,
                            "v2/compare {} JPEG PixelBuf construction failed: {}",
                            role,
                            e
                        )
                    })?;
                Ok(buf)
            }
        }
    }

    fn decode_probed_still(
        engine: &zencodecs::engine::Engine,
        mut probed: zencodecs::engine::ProbedInput<'_>,
        needs: zencodecs::engine::DecodeNeeds,
        run: &zencodecs::engine::Run<'_>,
    ) -> Result<zencodecs::DecodeOutput> {
        use crate::media::map_engine_error;
        let offers = engine.decode_offers(&probed, &needs, run).map_err(map_engine_error)?;
        let mut last_err = None;
        for offer in offers {
            match engine.prepare_decode(offer, probed, needs.clone(), run) {
                Ok(prepared) => {
                    return prepared.run().map(|c| c.output).map_err(map_engine_error);
                }
                Err(zencodecs::engine::PrepareError::Capability { cause, lease }) => {
                    last_err = Some(cause);
                    probed = lease;
                }
                Err(zencodecs::engine::PrepareError::Terminal(err)) => {
                    return Err(map_engine_error(err));
                }
            }
        }
        match last_err {
            Some(err) => Err(map_engine_error(err)),
            None => Err(nerror!(
                ErrorKind::Category(ErrorCategory::ActionNotSupported),
                "no workspace-admissible decoder satisfied v2/compare decode requirements"
            )),
        }
    }

    /// Validate the decoder's authoritative output [`PixelFormat`] as SDR full-range and return
    /// conservative CMS transform scratch bytes (`0` for non-ICC parametric formats).
    fn validate_decoded_compare_format(
        fmt: &zenpixels::PixelFormat,
        role: &str,
        live_mem: u64,
        total_mem_cap: Option<u64>,
        stop: &DeadlineStop,
    ) -> Result<u64> {
        use zenpixels::{AlphaMode, BitPacking, ColorSpace, Primaries, SignalRange, Transfer};
        if fmt.layout.packing != BitPacking::Full {
            return Err(nerror!(
                ErrorKind::Category(ErrorCategory::ActionNotSupported),
                "v2/compare requires full-packed pixel layout on {}",
                role
            ));
        }
        if fmt.color.is_hdr() {
            return Err(nerror!(
                ErrorKind::Category(ErrorCategory::ActionNotSupported),
                "v2/compare refuses HDR pixel format on {}",
                role
            ));
        }
        match &fmt.color {
            ColorSpace::Rgb { primaries, transfer, range, alpha, white } => {
                if *primaries == Primaries::Unknown || *transfer == Transfer::Unknown {
                    return Err(nerror!(
                        ErrorKind::Category(ErrorCategory::ActionNotSupported),
                        "v2/compare refuses unresolved/unknown RGB color interpretation on {}",
                        role
                    ));
                }
                if matches!(transfer, Transfer::Pq | Transfer::Hlg(_) | Transfer::Linear) {
                    return Err(nerror!(
                        ErrorKind::Category(ErrorCategory::ActionNotSupported),
                        "v2/compare refuses HDR or ambiguous linear-light transfer {:?} on {}",
                        transfer,
                        role
                    ));
                }
                if *range != SignalRange::Full {
                    return Err(nerror!(
                        ErrorKind::Category(ErrorCategory::ActionNotSupported),
                        "v2/compare requires full-range SDR pixels on {}",
                        role
                    ));
                }
                if white.is_some() {
                    return Err(nerror!(
                        ErrorKind::Category(ErrorCategory::ActionNotSupported),
                        "v2/compare refuses HDR DiffuseWhite anchor on SDR {}",
                        role
                    ));
                }
                if *alpha == AlphaMode::Premultiplied {
                    return Err(nerror!(
                        ErrorKind::Category(ErrorCategory::ActionNotSupported),
                        "v2/compare refuses premultiplied alpha on {}",
                        role
                    ));
                }
                Ok(0)
            }
            ColorSpace::Gray { transfer, range, alpha, white } => {
                if *transfer == Transfer::Unknown
                    || matches!(transfer, Transfer::Pq | Transfer::Hlg(_) | Transfer::Linear)
                    || *range != SignalRange::Full
                    || white.is_some()
                    || *alpha == AlphaMode::Premultiplied
                {
                    return Err(nerror!(
                        ErrorKind::Category(ErrorCategory::ActionNotSupported),
                        "v2/compare refuses unsupported grayscale SDR interpretation on {}",
                        role
                    ));
                }
                Ok(0)
            }
            ColorSpace::Icc { profile, color_channels, alpha, .. } => {
                if !matches!(*color_channels, 1 | 3) {
                    return Err(nerror!(
                        ErrorKind::Category(ErrorCategory::ActionNotSupported),
                        "v2/compare refuses {}-channel ICC color space on {}",
                        color_channels,
                        role
                    ));
                }
                if *alpha == AlphaMode::Premultiplied {
                    return Err(nerror!(
                        ErrorKind::Category(ErrorCategory::ActionNotSupported),
                        "v2/compare refuses premultiplied alpha on {}",
                        role
                    ));
                }
                validate_icc_sdr_profile(profile, role, live_mem, total_mem_cap, stop)
            }
            other => Err(nerror!(
                ErrorKind::Category(ErrorCategory::ActionNotSupported),
                "v2/compare refuses unsupported color space {:?} on {}",
                other,
                role
            )),
        }
    }

    fn normalize_to_canonical_rgba8_srgb(
        view: zenpixels::PixelView<'_>,
        role: &str,
        live_mem: u64,
        total_mem_cap: Option<u64>,
        stop: &DeadlineStop,
    ) -> Result<zenpixels::PixelBuf> {
        use zenpixels::{AlphaMode, PixelBuf, PixelFormat};
        use zenpixels_convert::{ConvertPolicy, MoxCms, RowPlan};

        if !view.hdr().is_empty() {
            return Err(nerror!(
                ErrorKind::Category(ErrorCategory::ActionNotSupported),
                "v2/compare refuses HDR PixelView metadata on {}",
                role
            ));
        }

        // Use the decoder's authoritative output sample interpretation directly — never blindly
        // rebind stored `SourceColor::icc_profile`, which would double-apply inverse profiles
        // on decoders that already transformed samples (e.g. `zenjpeg` XYB -> sRGB).
        let effective_fmt = view.format();
        let cms_scratch =
            validate_decoded_compare_format(effective_fmt, role, live_mem, total_mem_cap, stop)?;

        let src_is_opaque = matches!(
            effective_fmt.color.alpha(),
            AlphaMode::None | AlphaMode::Opaque | AlphaMode::Undefined
        );
        let target = PixelFormat::RGBA8_SRGB;
        let (w, h) = (view.width(), view.height());
        let stride = (w as usize)
            .checked_mul(4)
            .ok_or_else(|| nerror!(ErrorKind::SizeLimitExceeded, "v2/compare stride overflow"))?;
        let total = stride
            .checked_mul(h as usize)
            .ok_or_else(|| nerror!(ErrorKind::SizeLimitExceeded, "v2/compare buffer overflow"))?;
        let mut dst = Vec::new();
        dst.try_reserve_exact(total).map_err(|_| {
            nerror!(ErrorKind::AllocationFailed, "v2/compare buffer allocation failed")
        })?;
        dst.resize(total, 0u8);

        if *effective_fmt == target {
            for y in 0..h {
                if y & 63 == 0 {
                    stop.check_flow()?;
                }
                let src_row = view.row(y);
                let dst_row = &mut dst[(y as usize) * stride..(y as usize + 1) * stride];
                dst_row.copy_from_slice(&src_row[..stride]);
                if src_is_opaque {
                    for px in dst_row.as_chunks_mut::<4>().0 {
                        px[3] = 255;
                    }
                }
            }
        } else {
            let row_scratch = (w as u64).saturating_mul(16).saturating_add(64 * 1024);
            let plan_scratch = cms_scratch.saturating_add(row_scratch);
            check_transient_mem(live_mem, plan_scratch, total_mem_cap, "color conversion plan")?;

            // Build strictly ONE `RowPlan` (no duplicate `OutputPlan` + `RowPlan` pair).
            let policy = ConvertPolicy::default()
                .with_depth(zenpixels_convert::DepthPolicy::Round)
                .with_gamut(zenpixels_convert::GamutPolicy::Clip)
                .with_cms(Arc::new(MoxCms));
            let row_plan = RowPlan::build(effective_fmt, &target, &policy, None).map_err(|e| {
                nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "v2/compare row conversion plan failed on {}: {}",
                    role,
                    e
                )
            })?;
            for y in 0..h {
                if y & 63 == 0 {
                    stop.check_flow()?;
                }
                let src_row = view.row(y);
                let dst_row = &mut dst[(y as usize) * stride..(y as usize + 1) * stride];
                row_plan.convert_row(src_row, dst_row, w).map_err(|e| {
                    nerror!(
                        ErrorKind::ColorProfileError,
                        "v2/compare color conversion failed on {}: {}",
                        role,
                        e
                    )
                })?;
                if src_is_opaque {
                    for px in dst_row.as_chunks_mut::<4>().0 {
                        px[3] = 255;
                    }
                }
            }
        }
        stop.check_flow()?;

        PixelBuf::from_bytes(dst, w, h, target).map_err(|e| {
            nerror!(
                ErrorKind::InternalError,
                "v2/compare normalized PixelBuf validation failed: {}",
                e
            )
        })
    }

    pub fn invoke_compare_modern(
        context: &mut Context,
        json: &[u8],
        max_json: Option<usize>,
    ) -> Result<JsonResponse> {
        use crate::media::map_engine_error;
        use crate::media::report::{
            metric_direction_and_perfect, metric_id_dto, CompareMetricResultV2, CompareReportV2,
        };
        use zencodecs::engine::{DecodeNeeds, DecodeOrientation, Run};

        return_if_cancelled!(context);
        let env = parse_json_with_limit::<CompareEnvelopeV2>(json, max_json)?;

        let metric_tokens = match env.metrics {
            None => vec!["zensim_a".to_string()],
            Some(list) => {
                if list.is_empty() {
                    return Err(nerror!(
                        ErrorKind::InvalidArgument,
                        "v2/compare 'metrics' must not be empty"
                    ));
                }
                list
            }
        };

        let engine = shared_modern_engine()?;
        let mut seen_metrics = HashSet::new();
        let mut requested = Vec::with_capacity(metric_tokens.len());
        for tok in &metric_tokens {
            let m = parse_compare_metric(tok)?;
            if !seen_metrics.insert(m) {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "v2/compare 'metrics' contains duplicate metric '{}'",
                    tok
                ));
            }
            let Some(reg) = engine.metric_scorer(m) else {
                return Err(nerror!(
                    ErrorKind::Category(ErrorCategory::ActionNotSupported),
                    "v2/compare metric '{}' is not enabled/registered in this build",
                    tok
                ));
            };
            requested.push((m, reg));
        }

        let mut services = host_services_from_context(context);
        let timeout_ms = validate_and_apply_modern_limits(
            &mut services,
            env.limits.as_ref(),
            context.modern_config().timeout_ms,
        )?;
        let stop = DeadlineStop::new(context.cancellation_stop(), timeout_ms)?;
        stop.check_flow()?;
        let pulse = zc::SharedPulse::new(zc::how_far::StopOnly::new(stop.clone()));

        let max_in = services.limits.and_then(|l| l.max_input_bytes);
        let total_mem_cap = services.limits.and_then(|l| l.max_memory_bytes);
        let mut live_mem: u64 = 0;

        let charge_mem = |live: &mut u64, bytes: u64, what: &str| -> Result<()> {
            let next = live.saturating_add(bytes);
            if let Some(cap) = total_mem_cap
                && next > cap
            {
                return Err(nerror!(
                    ErrorKind::SizeLimitExceeded,
                    "v2/compare {} ({} B cumulative) exceeds max_memory_bytes ({} B)",
                    what,
                    next,
                    cap
                ));
            }
            *live = next;
            Ok(())
        };

        let build_step_run = |base: &zenpipe::canonical::HostServices,
                              live: u64,
                              pulse: &zc::SharedPulse|
         -> Run<'_> {
            let mut run =
                Run::new().with_workspace_policy(base.workspace_policy).with_pulse(pulse.clone());
            if let Some(mut lim) = base.limits {
                if let Some(cap) = total_mem_cap {
                    lim = lim.with_max_memory(cap.saturating_sub(live));
                }
                run = run.with_limits(lim);
            }
            if let Some(ref cs) = base.color_services {
                run = run.with_color_services(cs.clone());
            }
            run
        };

        // 1. Materialize both inputs within combined memory budget and single deadline.
        let (ref_norm, cand_norm, width, height) = {
            let input_map = if env.reference_io_id == env.candidate_io_id {
                BTreeMap::from([("reference".to_string(), env.reference_io_id)])
            } else {
                BTreeMap::from([
                    ("reference".to_string(), env.reference_io_id),
                    ("candidate".to_string(), env.candidate_io_id),
                ])
            };
            let (resolved, live_read_mem) =
                resolve_inputs(context, &input_map, max_in, total_mem_cap, &stop)?;
            charge_mem(&mut live_mem, live_read_mem, "input materialization")?;

            let ref_bytes = resolved
                .iter()
                .find(|(s, _)| s == "reference")
                .map(|(_, m)| m.as_slice())
                .expect("reference slot present");
            let cand_bytes = if env.reference_io_id == env.candidate_io_id {
                ref_bytes
            } else {
                resolved
                    .iter()
                    .find(|(s, _)| s == "candidate")
                    .map(|(_, m)| m.as_slice())
                    .expect("candidate slot present")
            };

            // 2. Admit & probe BOTH stills upfront sequentially (charging retained reference
            //    probe metadata BEFORE probing candidate, and refusing animations/video, HDR,
            //    Ultra HDR/MPO JPEGs, malformed PNG iCCP, and mismatched upright dimensions
            //    BEFORE decoding full rasters!).
            let (ref_probed, ref_uw, ref_uh, ref_probe_retained) = admit_and_probe_still(
                &engine,
                ref_bytes,
                &services,
                &mut live_mem,
                total_mem_cap,
                &stop,
                &pulse,
                "reference",
            )?;
            stop.check_flow()?;

            let (cand_probed, cand_uw, cand_uh, cand_probe_retained) = admit_and_probe_still(
                &engine,
                cand_bytes,
                &services,
                &mut live_mem,
                total_mem_cap,
                &stop,
                &pulse,
                "candidate",
            )?;
            stop.check_flow()?;

            if (ref_uw, ref_uh) != (cand_uw, cand_uh) {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "v2/compare upright dimensions mismatch: reference {}x{} vs candidate {}x{}",
                    ref_uw,
                    ref_uh,
                    cand_uw,
                    cand_uh
                ));
            }
            let norm_bytes = u64::from(ref_uw).saturating_mul(u64::from(ref_uh)).saturating_mul(4);
            let max_scratch = requested
                .iter()
                .map(|(_, reg)| reg.scorer.estimated_scratch_bytes(ref_uw, ref_uh))
                .max()
                .unwrap_or(0);
            // Pre-admit scoring steady state (2 normalized rasters + peak metric scratch) BEFORE decoding!
            if let Some(cap) = total_mem_cap
                && norm_bytes.saturating_mul(2).saturating_add(max_scratch) > cap
            {
                return Err(nerror!(
                    ErrorKind::SizeLimitExceeded,
                    "v2/compare scoring working set ({} B) exceeds max_memory_bytes ({} B)",
                    norm_bytes.saturating_mul(2).saturating_add(max_scratch),
                    cap
                ));
            }
            let decode_needs = DecodeNeeds::still().with_orientation(DecodeOrientation::Apply);

            // 3. Decode & normalize reference using its exact probed lease, then release raw buffer
            // and its owned ICC profile only after normalization drops `decoded`.
            let ref_norm = {
                // Pre-check normalized raster fits alongside live inputs & retained probes.
                if let Some(cap) = total_mem_cap
                    && live_mem.saturating_add(norm_bytes) > cap
                {
                    return Err(nerror!(
                        ErrorKind::SizeLimitExceeded,
                        "v2/compare reference working set exceeds max_memory_bytes ({} B)",
                        cap
                    ));
                }
                let run = build_step_run(&services, live_mem, &pulse);
                let decoded = decode_compare_still(
                    &engine,
                    ref_probed,
                    decode_needs.clone(),
                    &run,
                    &services,
                    live_mem,
                    total_mem_cap,
                    &stop,
                    "reference",
                )?;
                live_mem = live_mem.saturating_sub(ref_probe_retained);
                stop.check_flow()?;
                let decoded_icc_bytes = decoded_retained_icc_bytes(&decoded);
                let px = decoded.as_view();
                let raw_bytes = u64::from(px.width())
                    .saturating_mul(u64::from(px.height()))
                    .saturating_mul(px.format().layout.bytes_per_pixel() as u64);
                let decoded_held_bytes = raw_bytes.saturating_add(decoded_icc_bytes);
                charge_mem(&mut live_mem, decoded_held_bytes, "decoded reference raster and ICC")?;
                charge_mem(&mut live_mem, norm_bytes, "normalized reference raster")?;
                let norm = normalize_to_canonical_rgba8_srgb(
                    px,
                    "reference",
                    live_mem,
                    total_mem_cap,
                    &stop,
                )?;
                drop(decoded);
                live_mem = live_mem.saturating_sub(decoded_held_bytes);
                norm
            };

            // 4. Decode & normalize candidate under remaining combined memory budget.
            let cand_norm = {
                if let Some(cap) = total_mem_cap
                    && live_mem.saturating_add(norm_bytes) > cap
                {
                    return Err(nerror!(
                        ErrorKind::SizeLimitExceeded,
                        "v2/compare candidate working set exceeds max_memory_bytes ({} B)",
                        cap
                    ));
                }
                let run = build_step_run(&services, live_mem, &pulse);
                let decoded = decode_compare_still(
                    &engine,
                    cand_probed,
                    decode_needs,
                    &run,
                    &services,
                    live_mem,
                    total_mem_cap,
                    &stop,
                    "candidate",
                )?;
                live_mem = live_mem.saturating_sub(cand_probe_retained);
                stop.check_flow()?;
                let decoded_icc_bytes = decoded_retained_icc_bytes(&decoded);
                let px = decoded.as_view();
                if px.width() != ref_norm.width() || px.height() != ref_norm.height() {
                    return Err(nerror!(
                        ErrorKind::InvalidArgument,
                        "v2/compare decoded upright dimensions mismatch: reference {}x{} vs candidate {}x{}",
                        ref_norm.width(),
                        ref_norm.height(),
                        px.width(),
                        px.height()
                    ));
                }
                let raw_bytes = u64::from(px.width())
                    .saturating_mul(u64::from(px.height()))
                    .saturating_mul(px.format().layout.bytes_per_pixel() as u64);
                let decoded_held_bytes = raw_bytes.saturating_add(decoded_icc_bytes);
                charge_mem(&mut live_mem, decoded_held_bytes, "decoded candidate raster and ICC")?;
                charge_mem(&mut live_mem, norm_bytes, "normalized candidate raster")?;
                let norm = normalize_to_canonical_rgba8_srgb(
                    px,
                    "candidate",
                    live_mem,
                    total_mem_cap,
                    &stop,
                )?;
                drop(decoded);
                live_mem = live_mem.saturating_sub(decoded_held_bytes);
                norm
            };

            // Release materialized input buffers before metric scratch admission & scoring.
            drop(resolved);
            live_mem = live_mem.saturating_sub(live_read_mem);
            let (w, h) = (ref_norm.width(), ref_norm.height());
            (ref_norm, cand_norm, w, h)
        };

        // 5. Admit peak scorer scratch alongside both live normalized rasters, then score in order.
        let max_scratch = requested
            .iter()
            .map(|(_, reg)| reg.scorer.estimated_scratch_bytes(width, height))
            .max()
            .unwrap_or(0);
        charge_mem(&mut live_mem, max_scratch, "IQA metric scratch")?;

        let mut score_limits = services.limits.unwrap_or_default();
        if let Some(cap) = total_mem_cap {
            score_limits = score_limits.with_max_memory(cap.saturating_sub(live_mem - max_scratch));
        }
        let score_storage = zc::storage::StorageBudget::new(
            score_limits
                .max_memory_bytes
                .and_then(|b| usize::try_from(b).ok())
                .unwrap_or(usize::MAX),
            usize::MAX,
        );
        let score_resources =
            zc::resources::JobResources::new(score_limits, score_storage, pulse.clone());

        let mut results = Vec::with_capacity(requested.len());
        for (metric, reg) in requested {
            stop.check_flow()?;
            let score = reg
                .scorer
                .score_f64(ref_norm.as_view(), cand_norm.as_view(), &score_resources)
                .map_err(map_engine_error)?;
            stop.check_flow()?;
            if !score.is_finite() {
                return Err(nerror!(
                    ErrorKind::InternalError,
                    "v2/compare metric '{}' produced non-finite score",
                    metric.as_token()
                ));
            }
            let (direction, perfect) = metric_direction_and_perfect(metric);
            results.push(CompareMetricResultV2 {
                metric: metric.as_token().to_string(),
                score,
                direction: direction.to_string(),
                perfect,
                revision: reg.scorer.revision().to_string(),
                metric_id: reg.scorer.metadata().as_ref().map(metric_id_dto),
            });
        }
        stop.check_flow()?;

        Ok(JsonResponse::ok(CompareReportV2 {
            version: 2,
            width,
            height,
            comparison_format: "rgba8_srgb".to_string(),
            results,
        }))
    }
    #[cfg(test)]
    mod compare_preflight_tests {
        use super::{
            scan_raw_jpeg_single_sdr, scan_raw_png_chunks, CancellationToken, DeadlineStop,
        };
        use crate::{nerror, ErrorCategory, ErrorKind};

        #[test]
        fn jpeg_preflight_polls_stop_mid_entropy_scan() {
            // Minimal valid SOI + SOS (1 component) + 1 MiB marker-free entropy body + EOI.
            let mut jpeg = Vec::with_capacity(1024 * 1024 + 32);
            jpeg.extend_from_slice(&[0xFF, 0xD8]); // SOI
            jpeg.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]); // SOS
            jpeg.resize(jpeg.len() + 1024 * 1024, 0x55); // 1 MiB entropy scan with zero 0xFF bytes
            jpeg.extend_from_slice(&[0xFF, 0xD9]); // EOI

            let mut polls = 0usize;
            let err = scan_raw_jpeg_single_sdr(&jpeg, "candidate", &mut || {
                polls += 1;
                if polls >= 4 {
                    return Err(nerror!(
                        ErrorKind::Category(ErrorCategory::OperationCancelled),
                        "cancelled mid-SOS scan at poll {}",
                        polls
                    ));
                }
                Ok(())
            })
            .expect_err("must cancel inside 1 MiB entropy scan before reaching EOI");
            assert_eq!(err.category(), ErrorCategory::OperationCancelled);
            assert_eq!(
                polls, 4,
                "JPEG preflight must abort immediately on the 4th 64 KiB poll inside SOS"
            );
        }

        #[test]
        fn png_preflight_counts_compression_flagged_itxt_for_retained_details() {
            let mut png = Vec::new();
            png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
            let push_chunk = |out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]| {
                out.extend_from_slice(&(data.len() as u32).to_be_bytes());
                out.extend_from_slice(tag);
                out.extend_from_slice(data);
                out.extend_from_slice(&0u32.to_be_bytes());
            };
            let mut itxt = b"Software\0\x01\x00\0\0".to_vec();
            itxt.extend(std::iter::repeat_n(b'A', 131_072));
            push_chunk(&mut png, b"IHDR", &[0u8; 13]);
            push_chunk(&mut png, b"iTXt", &itxt);
            push_chunk(&mut png, b"IEND", &[]);

            let stop = DeadlineStop::new(CancellationToken::new(), None).unwrap();
            let scan = scan_raw_png_chunks(&png, "reference", &stop).unwrap();
            assert!(scan.has_compressed_text);
            assert!(scan.has_compressed_itxt);
            assert_eq!(
                scan.itxt_total_bytes,
                itxt.len() as u64,
                "compression-flagged Software iTXt payload must be counted for retained creating_tool charge"
            );
        }
        #[cfg(feature = "zen-codecs")]
        #[test]
        fn progressive_jpeg_probe_skips_entropy_and_honors_post_preflight_cancel() {
            // Minimal progressive SOF2 (8-bit, 16x16, 1 component) + SOS + 1 MiB progressive
            // entropy body + EOI. If the header probe called `zenjpeg::detect::probe`, it would
            // walk the entire 1 MiB progressive entropy scan without polling cancellation!
            let mut prog = Vec::with_capacity(1024 * 1024 + 64);
            prog.extend_from_slice(&[0xFF, 0xD8]); // SOI
            prog.extend_from_slice(&[
                0xFF, 0xC2, 0x00, 0x0B, // SOF2 (progressive), len=11
                8,    // precision=8
                0x00, 16, // height=16
                0x00, 16, // width=16
                1,  // 1 component
                1, 0x11, 0, // id=1, 1x1, qt=0
            ]);
            let expected_sos_offset = prog.len();
            prog.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00]); // SOS
            prog.resize(prog.len() + 1024 * 1024, 0x55); // 1 MiB progressive entropy body
            prog.extend_from_slice(&[0xFF, 0xD9]); // EOI

            // 1. Preflight walks cooperatively and returns the exact offset before the 1 MiB scan.
            let first_sos = scan_raw_jpeg_single_sdr(&prog, "candidate", &mut || Ok(())).unwrap();
            assert_eq!(first_sos, expected_sos_offset);

            // 2. Deadline/cancellation tripping AFTER preflight, right as header probe starts,
            //    aborts immediately with OperationCancelled without touching progressive entropy!
            let stop = DeadlineStop::new(CancellationToken::new(), Some(0)).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2));
            let services = zenpipe::canonical::HostServices::new();
            let mut live_mem = 0u64;
            let cancel_err = super::probe_jpeg_header_without_fingerprint(
                &prog,
                first_sos,
                &services,
                &mut live_mem,
                None,
                &stop,
                "candidate",
            )
            .err()
            .expect("post-preflight cancel must abort progressive JPEG probe immediately");
            assert_eq!(cancel_err.category(), ErrorCategory::OperationCancelled);

            // 3. Uncancelled probe succeeds purely from `&prog[..first_sos]` (zero scan bytes!)
            //    and recovers exact 16x16 progressive dimensions without invoking detect::probe.
            let live_stop = DeadlineStop::new(CancellationToken::new(), None).unwrap();
            let mut live_mem2 = 0u64;
            let (_, uw, uh, _) = super::probe_jpeg_header_without_fingerprint(
                &prog,
                first_sos,
                &services,
                &mut live_mem2,
                None,
                &live_stop,
                "candidate",
            )
            .expect("progressive JPEG header probe must succeed strictly from pre-SOS prefix");
            assert_eq!((uw, uh), (16, 16));
        }

        #[cfg(feature = "zen-codecs")]
        #[test]
        fn progressive_jpeg_decode_into_skips_fingerprint_and_honors_mid_decode_cancel() {
            use std::sync::atomic::{AtomicUsize, Ordering};

            // Encode a genuine multi-scan progressive 256x256 JPEG.
            let mut rgb = Vec::with_capacity(256 * 256 * 3);
            for y in 0..256u32 {
                for x in 0..256u32 {
                    rgb.push(((x * 3 + y) & 0xFF) as u8);
                    rgb.push(((y * 5 + x) & 0xFF) as u8);
                    rgb.push((((x ^ y) * 7) & 0xFF) as u8);
                }
            }
            const SENTINEL: u8 = 0xA5;

            let prog_bytes = zenjpeg::encoder::EncoderConfig::ycbcr(
                85.0,
                zenjpeg::encoder::ChromaSubsampling::Quarter,
            )
            .progressive(true)
            .encode_bytes(&rgb, 256, 256, zenjpeg::encoder::PixelLayout::Rgb8Srgb)
            .unwrap();

            // Counting Stop that fires at `limit`-th poll and counts every check.
            struct CancelAfterN {
                polls: AtomicUsize,
                limit: usize,
            }
            use enough::{Stop, StopReason};
            impl Stop for CancelAfterN {
                fn check(&self) -> std::result::Result<(), StopReason> {
                    if self.polls.fetch_add(1, Ordering::SeqCst) + 1 >= self.limit {
                        Err(StopReason::Cancelled)
                    } else {
                        Ok(())
                    }
                }
            }

            let first_sos =
                scan_raw_jpeg_single_sdr(&prog_bytes, "candidate", &mut || Ok(())).unwrap();
            let mut expected = vec![0u8; 256 * 256 * 3];

            // 1. Measure header-only polls on `&prog_bytes[..first_sos]` and full `decode_into`
            //    polls on the complete progressive 256x256 4:2:0 frame (16 MCU rows).
            let header_stop = CancelAfterN { polls: AtomicUsize::new(0), limit: usize::MAX };
            let _ = zenjpeg::decoder::Decoder::new()
                .auto_orient(false)
                .preserve_no_metadata()
                .num_threads(1)
                .decode_into(
                    &prog_bytes[..first_sos],
                    zenjpeg::decoder::PixelFormat::Rgb,
                    &mut expected,
                    &header_stop,
                );
            let header_polls = header_stop.polls.load(Ordering::SeqCst);

            let total_stop = CancelAfterN { polls: AtomicUsize::new(0), limit: usize::MAX };
            zenjpeg::decoder::Decoder::new()
                .auto_orient(false)
                .preserve_no_metadata()
                .num_threads(1)
                .decode_into(
                    &prog_bytes,
                    zenjpeg::decoder::PixelFormat::Rgb,
                    &mut expected,
                    &total_stop,
                )
                .unwrap();
            let total_polls = total_stop.polls.load(Ordering::SeqCst);
            let mcu_rows = 256usize / 16;
            assert!(
                total_polls > header_polls * 2 + mcu_rows + 4,
                "expected header + progressive entropy + MCU reconstruction polls (header={header_polls}, total={total_polls})"
            );

            // 2. Phase-specific witness A (progressive entropy decoding): cancel strictly after
            //    both header passes (`> 2 * header_polls`) and before MCU reconstruction (`dst`
            //    remains 100% untouched SENTINEL).
            let mid_entropy = header_polls * 2 + (total_polls - header_polls * 2 - mcu_rows) / 2;
            let mut dst_entropy = vec![SENTINEL; expected.len()];
            let entropy_stop = CancelAfterN { polls: AtomicUsize::new(0), limit: mid_entropy };
            let err_entropy = zenjpeg::decoder::Decoder::new()
                .auto_orient(false)
                .preserve_no_metadata()
                .num_threads(1)
                .decode_into(
                    &prog_bytes,
                    zenjpeg::decoder::PixelFormat::Rgb,
                    &mut dst_entropy,
                    &entropy_stop,
                )
                .expect_err("progressive decode_into must cancel inside entropy decoding");
            assert!(
                matches!(err_entropy.kind(), zenjpeg::decoder::ErrorKind::Cancelled(_)),
                "expected Cancelled during progressive entropy decode, got {err_entropy:?}"
            );
            assert_eq!(entropy_stop.polls.load(Ordering::SeqCst), mid_entropy);
            assert!(
                dst_entropy.iter().all(|&b| b == SENTINEL),
                "entropy cancellation must occur before pixel reconstruction writes to dst"
            );

            // 3. Phase-specific witness B (integer pixel reconstruction): cancel 3 MCU rows before
            //    completion into a fresh SENTINEL buffer and prove partial rendered output (first
            //    MCU strip == expected, final MCU strip still SENTINEL).
            let mid_recon = total_polls - 3;
            let mut dst_recon = vec![SENTINEL; expected.len()];
            let recon_stop = CancelAfterN { polls: AtomicUsize::new(0), limit: mid_recon };
            let err_recon = zenjpeg::decoder::Decoder::new()
                .auto_orient(false)
                .preserve_no_metadata()
                .num_threads(1)
                .decode_into(
                    &prog_bytes,
                    zenjpeg::decoder::PixelFormat::Rgb,
                    &mut dst_recon,
                    &recon_stop,
                )
                .expect_err("progressive decode_into must cancel inside pixel reconstruction");
            assert!(
                matches!(err_recon.kind(), zenjpeg::decoder::ErrorKind::Cancelled(_)),
                "expected Cancelled during pixel reconstruction, got {err_recon:?}"
            );
            assert_eq!(recon_stop.polls.load(Ordering::SeqCst), mid_recon);
            let strip_bytes = 16 * 256 * 3;
            assert_eq!(
                &dst_recon[..strip_bytes],
                &expected[..strip_bytes],
                "first MCU strip must be rendered before mid-reconstruction cancellation"
            );
            assert!(
                dst_recon[dst_recon.len() - strip_bytes..].iter().all(|&b| b == SENTINEL),
                "final MCU strip must stay untouched after mid-reconstruction cancellation"
            );

            // 4. Phase-specific witness C (f32 progressive grayscale reconstruction + chunked dst
            //    copy): 256x256 = 65,536 px = 4 batches of 16,384 px. Cancel mid-copy into a fresh
            //    SENTINEL buffer and prove batch 0 copied == expected while final batch stays SENTINEL.
            let gray: Vec<u8> = (0usize..(256 * 256))
                .map(|i| ((i.wrapping_mul(37) ^ (i >> 3)) & 0xFF) as u8)
                .collect();
            let prog_gray_bytes = zenjpeg::encoder::EncoderConfig::grayscale(85.0)
                .progressive(true)
                .encode_bytes(&gray, 256, 256, zenjpeg::encoder::PixelLayout::Gray8Srgb)
                .unwrap();
            let mut expected_gray = vec![0u8; 256 * 256];
            let gray_total_stop = CancelAfterN { polls: AtomicUsize::new(0), limit: usize::MAX };
            zenjpeg::decoder::Decoder::new()
                .auto_orient(false)
                .preserve_no_metadata()
                .num_threads(1)
                .decode_into(
                    &prog_gray_bytes,
                    zenjpeg::decoder::PixelFormat::Gray,
                    &mut expected_gray,
                    &gray_total_stop,
                )
                .unwrap();
            let gray_total_polls = gray_total_stop.polls.load(Ordering::SeqCst);

            let mid_gray_copy = gray_total_polls - 2;
            let mut dst_gray = vec![SENTINEL; expected_gray.len()];
            let gray_copy_stop = CancelAfterN { polls: AtomicUsize::new(0), limit: mid_gray_copy };
            let err_gray = zenjpeg::decoder::Decoder::new()
                .auto_orient(false)
                .preserve_no_metadata()
                .num_threads(1)
                .decode_into(
                    &prog_gray_bytes,
                    zenjpeg::decoder::PixelFormat::Gray,
                    &mut dst_gray,
                    &gray_copy_stop,
                )
                .expect_err("progressive grayscale decode_into must cancel mid-f32-relayout");
            assert!(
                matches!(err_gray.kind(), zenjpeg::decoder::ErrorKind::Cancelled(_)),
                "expected Cancelled during f32 grayscale reconstruction, got {err_gray:?}"
            );
            assert_eq!(gray_copy_stop.polls.load(Ordering::SeqCst), mid_gray_copy);
            assert_eq!(
                &dst_gray[..16_384],
                &expected_gray[..16_384],
                "first 16,384-pixel batch must be written to dst before mid-copy cancellation"
            );
            assert!(
                dst_gray[dst_gray.len() - 16_384..].iter().all(|&b| b == SENTINEL),
                "final 16,384-pixel batch must stay untouched after mid-copy cancellation"
            );

            // 5. Also verify `decode_compare_still` aborts immediately on post-probe deadline expiry.
            let services = zenpipe::canonical::HostServices::new();
            let ok_stop = DeadlineStop::new(CancellationToken::new(), None).unwrap();
            let mut live_mem = 0u64;
            let (probed, uw, uh, _) = super::probe_jpeg_header_without_fingerprint(
                &prog_bytes,
                first_sos,
                &services,
                &mut live_mem,
                None,
                &ok_stop,
                "candidate",
            )
            .unwrap();
            assert_eq!((uw, uh), (256, 256));

            let expired_stop = DeadlineStop::new(CancellationToken::new(), Some(0)).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(2));
            let engine = crate::media::shared_modern_engine().unwrap();
            let pulse = zc::SharedPulse::new(zc::how_far::StopOnly::new(expired_stop.clone()));
            let run = zencodecs::engine::Run::new().with_pulse(pulse);
            let err = super::decode_compare_still(
                &engine,
                probed,
                zencodecs::engine::DecodeNeeds::still(),
                &run,
                &services,
                live_mem,
                None,
                &expired_stop,
                "candidate",
            )
            .err()
            .expect("decode_compare_still must honor expired stop immediately");
            assert_eq!(err.category(), ErrorCategory::OperationCancelled);
        }

        #[cfg(feature = "zen-codecs")]
        #[test]
        fn jpeg_icc_overlapping_allocations_and_post_decode_retention_are_budgeted() {
            // Build a valid sRGB ICC profile padded with a benign private tag (`padd`) to ~256 KiB
            // so overlapping parser + descriptor ICC copies are measurably significant.
            let mut base_prof = moxcms::ColorProfile::new_srgb();
            base_prof.cicp = None;
            let mut icc = base_prof.encode().unwrap();
            let pad_payload_len: usize = 256 * 1024;
            let orig_tag_count = u32::from_be_bytes([icc[128], icc[129], icc[130], icc[131]]);
            let new_tag_count = orig_tag_count + 1;
            // Shift every existing tag offset forward by 12 bytes to make room for one table entry.
            let old_table_end = 132 + (orig_tag_count as usize) * 12;
            let mut patched = Vec::with_capacity(icc.len() + 12 + pad_payload_len + 4);
            patched.extend_from_slice(&icc[..128]);
            patched.extend_from_slice(&new_tag_count.to_be_bytes());
            for i in 0..(orig_tag_count as usize) {
                let base = 132 + i * 12;
                let sig = &icc[base..base + 4];
                let off = u32::from_be_bytes([
                    icc[base + 4],
                    icc[base + 5],
                    icc[base + 6],
                    icc[base + 7],
                ]) + 12;
                let sz = &icc[base + 8..base + 12];
                patched.extend_from_slice(sig);
                patched.extend_from_slice(&off.to_be_bytes());
                patched.extend_from_slice(sz);
            }
            let body_start = patched.len() + 12;
            let body_len = icc.len() - old_table_end;
            let pad_off = (body_start + body_len + 3) & !3;
            patched.extend_from_slice(b"padd");
            patched.extend_from_slice(&(pad_off as u32).to_be_bytes());
            patched.extend_from_slice(&(pad_payload_len as u32).to_be_bytes());
            patched.extend_from_slice(&icc[old_table_end..]);
            patched.resize(pad_off, 0);
            patched.extend_from_slice(b"data\0\0\0\0");
            patched.resize(pad_off + pad_payload_len, 0x41);
            let total_icc_len = patched.len() as u32;
            patched[0..4].copy_from_slice(&total_icc_len.to_be_bytes());
            icc = patched;

            // Embed the multi-chunk ICC profile into a tiny 16x16 baseline JPEG via APP2 segments.
            let rgb = vec![120u8; 16 * 16 * 3];
            let mut cfg = zenjpeg::encoder::EncoderConfig::ycbcr(
                90.0,
                zenjpeg::encoder::ChromaSubsampling::None,
            )
            .progressive(false);
            // Split ICC into <= 65,519-byte APP2 chunks with ICC_PROFILE\0 header.
            let max_chunk = 60_000usize;
            let chunk_count = icc.len().div_ceil(max_chunk) as u8;
            for (idx, chunk) in icc.chunks(max_chunk).enumerate() {
                let mut seg = Vec::with_capacity(14 + chunk.len());
                seg.extend_from_slice(b"ICC_PROFILE\0");
                seg.push((idx + 1) as u8);
                seg.push(chunk_count);
                seg.extend_from_slice(chunk);
                cfg = cfg.add_segment(0xE2, seg);
            }
            let jpg_bytes =
                cfg.encode_bytes(&rgb, 16, 16, zenjpeg::encoder::PixelLayout::Rgb8Srgb).unwrap();

            let first_sos =
                scan_raw_jpeg_single_sdr(&jpg_bytes, "candidate", &mut || Ok(())).unwrap();
            let services = zenpipe::canonical::HostServices::new();
            let ok_stop = DeadlineStop::new(CancellationToken::new(), None).unwrap();
            let mut live_mem = jpg_bytes.len() as u64;
            let (probed, uw, uh, retained) = super::probe_jpeg_header_without_fingerprint(
                &jpg_bytes,
                first_sos,
                &services,
                &mut live_mem,
                None,
                &ok_stop,
                "candidate",
            )
            .unwrap();
            assert_eq!((uw, uh), (16, 16));
            assert!(retained >= icc.len() as u64);

            // Cap that fits live_mem + raster + strip scratch + 512 KiB, but NOT the parser's
            // overlapping 2x ICC allocation (`parser_icc_bytes = 2 * icc.len()`).
            let tight_cap =
                live_mem + (16 * 16 * 3) + (16 * 64 * 4) + 512 * 1024 + (icc.len() as u64 / 2);
            let engine = crate::media::shared_modern_engine().unwrap();
            let pulse = zc::SharedPulse::new(zc::how_far::StopOnly::new(ok_stop.clone()));
            let run = zencodecs::engine::Run::new().with_pulse(pulse.clone());
            let err = super::decode_compare_still(
                &engine,
                probed,
                zencodecs::engine::DecodeNeeds::still(),
                &run,
                &services,
                live_mem,
                Some(tight_cap),
                &ok_stop,
                "candidate",
            )
            .err()
            .expect("decode_compare_still must refuse when parser overlapping ICC exceeds cap");
            assert!(
                matches!(err.kind, ErrorKind::SizeLimitExceeded),
                "expected SizeLimitExceeded, got {err:?}"
            );

            // With sufficient cap, decode succeeds and `decoded_retained_icc_bytes` reports the
            // full ICC profile kept alive on the `PixelBuf` through normalization.
            let mut live_mem2 = jpg_bytes.len() as u64;
            let (probed2, _, _, _) = super::probe_jpeg_header_without_fingerprint(
                &jpg_bytes,
                first_sos,
                &services,
                &mut live_mem2,
                None,
                &ok_stop,
                "candidate",
            )
            .unwrap();
            let roomy_cap = tight_cap + (icc.len() as u64) * 3;
            let decoded = super::decode_compare_still(
                &engine,
                probed2,
                zencodecs::engine::DecodeNeeds::still(),
                &run,
                &services,
                live_mem2,
                Some(roomy_cap),
                &ok_stop,
                "candidate",
            )
            .unwrap();
            assert_eq!(
                super::decoded_retained_icc_bytes(&decoded),
                icc.len() as u64,
                "decoded PixelBuf must keep full ICC size charged until normalization drops it"
            );
        }
    }
}

#[cfg(feature = "zen-media")]
use media_impl::{
    invoke_compare_modern, invoke_estimate_modern, invoke_execute_modern, invoke_inspect_modern,
};
