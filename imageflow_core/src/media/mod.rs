//! Modern canonical Zen pipeline, video media (`AV1`/`VP8`/`VP9` × `MP4`/`WebM`/`IVF`),
//! sans-I/O streaming session, and opt-in Tokio async driver for `imageflow_core`.
//!
//! 100% safe Rust (`forbid(unsafe_code)` when `c-codecs` is disabled). All C ABI
//! translation lives exclusively in `imageflow_abi::streaming`.

pub mod bindings;
pub mod report;
#[cfg(feature = "streaming")]
pub mod session;
#[cfg(feature = "streaming-async")]
pub mod worker;
#[cfg(feature = "streaming-async")]
pub mod async_driver;

pub use bindings::{
    BindingAccess, BindingSpec, FileReadAtSource, IoEvent, IoInput, IoTicket, JobStop,
    OperationKind, ReadAtSource, Reply, SliceReadAtSource, StopCause, Visibility, WorkQuota,
};
pub use report::{
    CapabilitiesReportV2, ClockV2, DimensionsV2, EstimateReportV2, InspectReportV2,
    InspectTrackV2, MediaRouteCapabilityV2, PublicationReceiptV2, StartupDependenciesV2,
    StreamErrorReportV2, build_capabilities_report,
};
#[cfg(feature = "streaming")]
pub use session::{CopyPendingError, MediaSession, SessionError};
#[cfg(feature = "streaming-async")]
pub use async_driver::{AsyncJobHandle, AsyncJobHooks, AsyncStartError, NotificationActivator};

use std::num::NonZeroU32;
use std::sync::{Arc, OnceLock};

use serde::Deserialize;

use crate::context::ModernContextConfig;
use crate::{Context, ErrorKind, FlowError, Result};

use zc::{CodecSet, ResourceLimits};
use zencodecs::bundles;
use zencodecs::engine::{Engine, WorkspacePolicy};
use zenpipe::canonical::HostServices;

/// Shared strict per-request resource limit overrides accepted identically across
/// `v2/configure_context`, `v1/execute_zen`, `v2/execute`, `v2/inspect`, `v2/estimate`,
/// and `MediaSession::create*` envelopes.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModernLimitsV2 {
    #[serde(default)]
    pub max_memory_bytes: Option<u64>,
    #[serde(default)]
    pub max_output_bytes: Option<u64>,
    #[serde(default)]
    pub max_input_bytes: Option<u64>,
    #[serde(default)]
    pub max_frames: Option<u32>,
    #[serde(default)]
    pub max_dimension: Option<u32>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// Intersect per-request [`ModernLimitsV2`] into `services` (may only tighten ceilings).
/// Returns the tightest resolved wall-clock deadline in milliseconds, if any.
pub fn apply_modern_limits(
    services: &mut HostServices,
    req_lim: Option<&ModernLimitsV2>,
    base_timeout_ms: Option<u64>,
) -> Option<u64> {
    let mut deadline_ms = base_timeout_ms;
    let Some(rl) = req_lim else {
        return deadline_ms;
    };
    let mut lim = services.limits.unwrap_or_default();
    if let Some(m) = rl.max_memory_bytes {
        lim = lim.with_max_memory(lim.max_memory_bytes.map_or(m, |c| c.min(m)));
    }
    if let Some(o) = rl.max_output_bytes {
        lim = lim.with_max_output(lim.max_output_bytes.map_or(o, |c| c.min(o)));
    }
    if let Some(i) = rl.max_input_bytes {
        lim = lim.with_max_input_bytes(lim.max_input_bytes.map_or(i, |c| c.min(i)));
    }
    if let Some(f) = rl.max_frames {
        lim = lim.with_max_frames(lim.max_frames.map_or(f, |c| c.min(f)));
    }
    if let Some(d) = rl.max_dimension {
        lim = lim.with_max_width(lim.max_width.map_or(d, |c| c.min(d)));
        lim = lim.with_max_height(lim.max_height.map_or(d, |c| c.min(d)));
    }
    if let Some(t) = rl.timeout_ms {
        deadline_ms = Some(deadline_ms.map_or(t, |c| c.min(t)));
    }
    services.limits = Some(lim);
    deadline_ms
}

/// Lazily initialized process-shared immutable [`zencodecs::Engine`] carrying all
/// compiled-in still, animation, video codec, container, and `moxcms` registrations.
pub fn shared_modern_engine() -> Result<Arc<Engine>> {
    static ENGINE: OnceLock<std::result::Result<Arc<Engine>, String>> = OnceLock::new();
    let cached = ENGINE.get_or_init(|| {
        let set = bundles::register_all_enabled(CodecSet::new())
            .map_err(|e| format!("failed to register zencodecs codecs: {e}"))?;
        let engine = Engine::new(set)
            .map_err(|e| format!("failed to initialize zencodecs Engine: {e}"))?
            .with_color_services(bundles::moxcms_color_services());
        Ok(Arc::new(engine))
    });
    match cached {
        Ok(eng) => Ok(Arc::clone(eng)),
        Err(msg) => Err(nerror!(ErrorKind::InternalError, "{}", msg)),
    }
}

/// Snapshot immutable [`HostServices`] (policy + ceiling limits) from an Imageflow [`Context`].
///
/// Intersects legacy `ExecutionSecurity` with explicit `ModernContextConfig` limits so
/// modern execution can never exceed host-configured safety ceilings.
pub fn host_services_from_context(ctx: &Context) -> HostServices {
    host_services_from_parts(&ctx.security, &ctx.modern_config())
}

/// Build [`HostServices`] from explicit security and modern config snapshots (zero borrow of `Context`).
pub fn host_services_from_parts(
    security: &imageflow_types::ExecutionSecurity,
    cfg: &ModernContextConfig,
) -> HostServices {
    let mut lim = ResourceLimits::default();

    // Intersect dimension ceilings.
    let mut max_w: Option<u32> = cfg.max_dimension;
    let mut max_h: Option<u32> = cfg.max_dimension;
    let mut max_px: Option<u64> = security.max_total_file_pixels;
    for frame_lim in [
        security.max_decode_size.as_ref(),
        security.max_frame_size.as_ref(),
        security.max_encode_size.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        max_w = min_opt_u32(max_w, Some(frame_lim.w));
        max_h = min_opt_u32(max_h, Some(frame_lim.h));
        let mp = frame_lim.megapixels;
        if mp.is_finite() && mp > 0.0 {
            let px = (mp as f64 * 1_000_000.0).min(u64::MAX as f64) as u64;
            max_px = min_opt_u64(max_px, Some(px));
        }
    }
    if let Some(w) = max_w {
        lim = lim.with_max_width(w);
    }
    if let Some(h) = max_h {
        lim = lim.with_max_height(h);
    }
    if let Some(px) = max_px {
        lim = lim.with_max_pixels(px);
    }
    let max_in = min_opt_u64(
        security.max_input_file_bytes.map(|v| v as u64),
        cfg.max_input_bytes,
    );
    if let Some(b) = max_in {
        lim = lim.with_max_input_bytes(b);
    }
    if let Some(mem) = cfg.max_memory_bytes {
        lim = lim.with_max_memory(mem);
    }
    if let Some(out) = cfg.max_output_bytes {
        lim = lim.with_max_output(out);
    }
    if let Some(frames) = cfg.max_frames {
        lim = lim.with_max_frames(frames);
    }

    let policy = if cfg.workspace_policy_best_effort {
        WorkspacePolicy::BestEffort
    } else {
        WorkspacePolicy::Strict
    };

    let mut services = HostServices::new()
        .with_workspace_policy(policy)
        .with_limits(lim)
        .with_color_services(bundles::moxcms_color_services());

    if let Some(cap) = cfg.trace_capacity.and_then(NonZeroU32::new) {
        services.trace_capacity = Some(cap);
    }
    services
}

/// Map a `zenpipe::PipeError` into an Imageflow [`FlowError`] with accurate [`ErrorCategory`].
pub(crate) fn map_pipe_error(err: whereat::At<zenpipe::PipeError>) -> FlowError {
    let msg = err.to_string();
    match err.error() {
        zenpipe::PipeError::LimitExceeded(_) => {
            nerror!(ErrorKind::SizeLimitExceeded, "{}", msg)
        }
        zenpipe::PipeError::Stopped(_) => {
            nerror!(ErrorKind::OperationCancelled, "{}", msg)
        }
        zenpipe::PipeError::FormatMismatch { .. } => {
            nerror!(ErrorKind::Category(crate::ErrorCategory::ActionNotSupported), "{}", msg)
        }
        zenpipe::PipeError::DimensionMismatch(_) => {
            nerror!(ErrorKind::InvalidArgument, "{}", msg)
        }
        zenpipe::PipeError::Op(s) => {
            if s.contains("cancelled") || s.contains("Cancelled") {
                nerror!(ErrorKind::OperationCancelled, "{}", msg)
            } else if s.contains("not found") || s.contains("missing") || s.contains("unknown") {
                nerror!(ErrorKind::InvalidArgument, "{}", msg)
            } else if s.contains("unsupported") || s.contains("Unsupported") || s.contains("policy")
            {
                nerror!(ErrorKind::Category(crate::ErrorCategory::ActionNotSupported), "{}", msg)
            } else if s.contains("limit") || s.contains("exceeds") || s.contains("budget") {
                nerror!(ErrorKind::SizeLimitExceeded, "{}", msg)
            } else {
                nerror!(ErrorKind::InvalidOperation, "{}", msg)
            }
        }
        _ => nerror!(ErrorKind::InternalError, "{}", msg),
    }
}

/// Map a `zencodecs::engine::EngineError` into an Imageflow [`FlowError`].
pub(crate) fn map_engine_error(err: whereat::At<zencodecs::engine::EngineError>) -> FlowError {
    use zencodecs::engine::Failure;
    let msg = err.to_string();
    match err.error().failure() {
        Failure::Stopped(_) => nerror!(ErrorKind::OperationCancelled, "{}", msg),
        Failure::ResourceLimit => nerror!(ErrorKind::SizeLimitExceeded, "{}", msg),
        Failure::InvalidRequest => nerror!(ErrorKind::InvalidArgument, "{}", msg),
        Failure::InvalidData | Failure::UnrecognizedFormat => {
            nerror!(ErrorKind::ImageDecodingError, "{}", msg)
        }
        Failure::Io => nerror!(ErrorKind::DecodingIoError, "{}", msg),
        Failure::PolicyRefusal
        | Failure::UnsupportedEffect
        | Failure::NoBackend(_)
        | Failure::NoMediaBackend
        | Failure::DeniedFormat(_)
        | Failure::MissingCms
        | Failure::UnknownInterpretation => {
            nerror!(ErrorKind::Category(crate::ErrorCategory::ActionNotSupported), "{}", msg)
        }
        Failure::BackendFailure | Failure::CorruptOutput => {
            nerror!(ErrorKind::InternalError, "{}", msg)
        }
        _ => nerror!(ErrorKind::InternalError, "{}", msg),
    }
}

fn min_opt_u32(a: Option<u32>, b: Option<u32>) -> Option<u32> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

fn min_opt_u64(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}
