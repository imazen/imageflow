//! Modern JSON v2 / `v1/execute_zen` endpoint dispatcher (`§2.3` & Decision 1).
//!
//! Implements the canonical `zenpipe-canonical-v1` wire contract (`encode.format`,
//! `encode.video_codec`, `decode.select`, `encode.sequence`, `encode.tracks`,
//! `encode.publication`, `encode.metadata`, `encode.coding`, `OutputV2`) across
//! `v1/execute_zen`, `v2/execute`, `v2/inspect`, `v2/estimate`, `v2/configure_context`,
//! and `v2/capabilities`, while leaving legacy v1 endpoints 100% untouched.

use serde::Deserialize;

use crate::json::{JsonResponse, parse_json_with_limit};
use crate::{Context, ErrorCategory, ErrorKind, Result};

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
    if let Some(mode) = req.input_registration {
        match mode.as_str() {
            "deferred" => context.set_input_registration_deferred(true),
            "eager" => context.set_input_registration_deferred(false),
            other => {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "invalid input_registration '{}' (expected 'eager' or 'deferred')",
                    other
                ));
            }
        }
    }
    if let Some(policy) = req.workspace_policy {
        match policy.as_str() {
            "strict" => context.modern_config_mut().workspace_policy_best_effort = false,
            "best_effort" => context.modern_config_mut().workspace_policy_best_effort = true,
            other => {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "invalid workspace_policy '{}' (expected 'strict' or 'best_effort')",
                    other
                ));
            }
        }
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
        ModernLimitsV2, apply_modern_limits, host_services_from_context, map_pipe_error,
        report::{build_estimate_report, inspect_bytes_with_stop},
        shared_modern_engine,
    };

    /// Cooperative stop combining live `Context` cancellation with resolved `timeout_ms` deadline.
    #[derive(Clone)]
    struct DeadlineStop {
        cancel: CancellationToken,
        deadline: Option<Instant>,
    }

    impl DeadlineStop {
        fn new(cancel: CancellationToken, timeout_ms: Option<u64>) -> Self {
            let deadline = timeout_ms.and_then(|ms| Instant::now().checked_add(Duration::from_millis(ms)));
            Self { cancel, deadline }
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
                return Err(nerror!(ErrorKind::OperationCancelled, "operation timed out (timeout_ms)"));
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
            self.cancel.should_stop()
                || self.deadline.is_some_and(|dl| Instant::now() >= dl)
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
                            output_buf
                                .capacity()
                                .saturating_mul(2)
                                .max(needed_len)
                                .min(cap_u)
                        } else {
                            output_buf.capacity().saturating_mul(2).max(needed_len)
                        };
                        output_buf
                            .try_reserve_exact(target_cap - output_buf.len())
                            .map_err(|_| {
                                nerror!(ErrorKind::AllocationFailed, "output buffer reserve failed")
                            })?;
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
        let timeout_ms =
            apply_modern_limits(&mut services, env.limits.as_ref(), context.modern_config().timeout_ms);
        let stop = DeadlineStop::new(context.cancellation_stop(), timeout_ms);
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

        // Pinned zenpipe `PreparedJob::run()` does not expose mid-DAG cooperative `Pulse` stops.
        // Pre-expired deadlines were already rejected by `stop.check_flow()?` above; explicitly
        // refuse positive `timeout_ms` on buffered multi-node Execute rather than silently running
        // an un-interruptible DAG past its wall-clock deadline!
        if timeout_ms.is_some() {
            stop.check_flow()?;
            return Err(nerror!(
                ErrorKind::Category(ErrorCategory::ActionNotSupported),
                "limits.timeout_ms cannot be enforced mid-execution on buffered multi-node Execute pipelines with the pinned zenpipe runtime; use a direct ReadAt video streaming route, v2/inspect, v2/estimate, or omit timeout_ms"
            ));
        }

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
            prepared.run().map_err(map_pipe_error)?
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
        let timeout_ms =
            apply_modern_limits(&mut services, env.limits.as_ref(), context.modern_config().timeout_ms);
        let stop = DeadlineStop::new(context.cancellation_stop(), timeout_ms);
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
        let timeout_ms =
            apply_modern_limits(&mut services, env.limits.as_ref(), context.modern_config().timeout_ms);
        let stop = DeadlineStop::new(context.cancellation_stop(), timeout_ms);
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
}

#[cfg(feature = "zen-media")]
use media_impl::{invoke_estimate_modern, invoke_execute_modern, invoke_inspect_modern};
