//! Structured modern JSON reports for `v2/inspect`, `v2/estimate`, `v2/capabilities`,
//! and streaming publication receipts (`§2.3` of the Imageflow media/async design).
//!
//! 64-bit byte counts, durations, and timestamp ticks are serialized as decimal strings
//! to preserve full precision across JavaScript, .NET, and C consumers.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use enough::Stop;
use serde::{Deserialize, Serialize};

use crate::{ErrorKind, Result};

use zc::decode::{Input, InputRequest};
use zc::source::{AccessPattern, SourceAccess};
use zc::time::TimeBase;
use zc::{ImageDecodeRole, ImageEncodeRole, ImageFormat};
use zencodec_media::container::{DemuxEvent, MuxOffer};
use zencodec_media::registry::{
    ContainerId, DemuxNeeds, DemuxRole, MuxRole, ProbeLimits, TimingPolicy as MediaTimingPolicy,
    VideoDecodeRole, VideoEncodeRole,
};
use zencodec_media::track::{CodecId as MediaCodecId, TrackInfo, TrackKind, VideoTrackInfo};
use zencodecs::engine::{
    Coding, Engine, MetadataPolicy, PublicationMode, RouteReport, Run, SequencePolicy,
    TargetFormat, TimingPolicy, TrackPolicy, WorkspacePolicy,
};
use zencodecs::media::probe::{ContainerProbeOutcome, probe_container_prefix};
use zencodecs::media::route::{select_demuxer, select_muxer, validate_container_codec_tuple};
use zenpipe::canonical::{HostServices, ImageOp, OutputOp, PipelineRuntime};

/// Positive width/height pair used in `v2/inspect` coded and display geometry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DimensionsV2 {
    pub width: u32,
    pub height: u32,
}

/// Reduced rational clock (`numerator / denominator` seconds per tick).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockV2 {
    pub numerator: u32,
    pub denominator: u32,
}

/// One discovered track in a `v2/inspect` response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectTrackV2 {
    pub id: u32,
    pub kind: String,
    pub codec: String,
    pub evidence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coded: Option<DimensionsV2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<DimensionsV2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clock: Option<ClockV2>,
    pub duration_ticks: Option<String>,
    pub presentation_count: Option<String>,
    pub seek_scopes: Vec<String>,
}

/// Canonical `v2/inspect` response payload (`§2.3`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectReportV2 {
    pub container: String,
    pub length_bytes: Option<String>,
    pub consumed_input_bytes: String,
    pub tracks: Vec<InspectTrackV2>,
    pub warnings: Vec<String>,
}

/// Startup I/O dependencies predicted by `v2/estimate`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartupDependenciesV2 {
    pub needs_length: bool,
    pub needs_range: bool,
    /// `Some(true)`/`Some(false)` when every video output's concrete track descriptor was
    /// known and quoted against the selected muxer; `None` when patch requirements cannot be
    /// determined without fabricating a synthetic descriptor (e.g. incomplete container headers
    /// or non-direct transformed video graph).
    pub needs_patch: Option<bool>,
    pub needs_replay: bool,
}

/// Canonical `v2/estimate` response payload (`§2.3`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EstimateReportV2 {
    pub version: u32,
    pub node_count: usize,
    pub output_count: usize,
    pub input_slots: Vec<String>,
    pub output_slots: Vec<String>,
    pub input_bytes: String,
    pub workspace_policy: String,
    pub startup_dependencies: StartupDependenciesV2,
    pub warnings: Vec<String>,
}

/// One advertised or explicitly refused media carrier × codec route in `v2/capabilities`,
/// derived directly from live `DemuxFactory::quote` and `MuxFactory::quote` offers!
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaRouteCapabilityV2 {
    pub container: String,
    pub video_codec: String,
    pub direction: String,
    pub layout: String,
    pub access: String,
    pub incremental: bool,
    pub timing: String,
    pub supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<String>,
}

/// Canonical `v2/capabilities` response payload derived from live [`Engine`] offers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitiesReportV2 {
    pub version: u32,
    /// Canonical wire schema identifier (`"zenpipe-canonical-v1"`).
    pub wire_schema: String,
    pub zen_media: bool,
    pub streaming_manual: bool,
    pub streaming_async: bool,
    pub cms_provider: Option<String>,
    pub still_formats: Vec<String>,
    pub video_codecs: Vec<String>,
    pub media_containers: Vec<String>,
    /// Score-only perceptual metrics currently registered for `coding: { verify: ... }`.
    pub verify_metrics: Vec<String>,
    pub media_routes: Vec<MediaRouteCapabilityV2>,
}

/// Per-sink byte publication receipt reported on streaming completion or failure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicationReceiptV2 {
    pub io_id: i32,
    pub slot: String,
    pub visibility: String,
    pub accepted_bytes: String,
    pub exact_unknown_tail: bool,
    pub committed: bool,
}

/// Structured terminal error JSON returned by `MediaSession::result_json()` on `Failed`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamErrorReportV2 {
    pub version: u32,
    pub success: bool,
    pub category: i32,
    pub stop_reason: u32,
    pub message: String,
    pub publications: Vec<PublicationReceiptV2>,
}

/// Build the truthful `v2/capabilities` report by inspecting live qualified offers on
/// [`crate::media::shared_modern_engine`].
#[must_use]
pub fn build_capabilities_report() -> CapabilitiesReportV2 {
    match crate::media::shared_modern_engine() {
        Ok(engine) => build_capabilities_report_for_engine(&engine),
        Err(_) => CapabilitiesReportV2 {
            version: 2,
            wire_schema: "zenpipe-canonical-v1".to_string(),
            zen_media: cfg!(feature = "zen-media"),
            streaming_manual: cfg!(feature = "streaming"),
            streaming_async: cfg!(feature = "streaming-async"),
            cms_provider: None,
            still_formats: Vec::new(),
            video_codecs: Vec::new(),
            media_containers: Vec::new(),
            verify_metrics: Vec::new(),
            media_routes: Vec::new(),
        },
    }
}

fn build_run_for_services<'a>(
    services: &'a HostServices,
    pulse: Option<zc::SharedPulse>,
) -> Run<'a> {
    let mut run = Run::new().with_workspace_policy(services.workspace_policy);
    if let Some(lim) = services.limits {
        run = run.with_limits(lim);
    }
    if let Some(ref cs) = services.color_services {
        run = run.with_color_services(cs.clone());
    }
    if let Some(ref routing) = services.routing {
        run = run.with_routing(routing);
    }
    if let Some(p) = pulse {
        run = run.with_pulse(p);
    }
    run
}

/// Select and quote the ACTUAL route-matched [`MuxRole`] factory for a concrete
/// `(container, codec, publication, sequence, track_dims, track_time_base)` tuple under
/// `services` (`workspace_policy`, `limits`, `routing`, `color_services`), mirroring
/// `MediaPump`'s exact candidate ranking and timing selection!
#[allow(clippy::too_many_arguments)]
pub(crate) fn select_mux_offer_for_route(
    engine: &Engine,
    services: &HostServices,
    container: &ContainerId,
    codec: &MediaCodecId,
    publication: PublicationMode,
    sequence: &SequencePolicy,
    track_dims: (u32, u32),
    track_time_base: TimeBase,
) -> std::result::Result<(String, MuxOffer), String> {
    validate_container_codec_tuple(container, codec).map_err(|e| e.to_string())?;
    let run = build_run_for_services(services, None);
    let mut eff = engine.resolve_run(&run).map_err(|e| e.to_string())?;
    let mut report = RouteReport::default();
    let trk = TrackInfo::video(
        0,
        codec.clone(),
        track_time_base,
        VideoTrackInfo::new(track_dims.0.max(1), track_dims.1.max(1)),
    )
    .build()
    .map_err(|e| format!("failed to build route track descriptor: {e}"))?;
    let tracks: Arc<[TrackInfo]> = Arc::from(vec![trk]);

    match sequence {
        SequencePolicy::Preserve { timing: TimingPolicy::Exact, .. } => {
            let (id, _f, plan) = select_muxer(
                engine,
                container,
                tracks,
                publication,
                MediaTimingPolicy::Exact,
                &mut eff,
                &mut report,
            )
            .map_err(|e| e.to_string())?;
            Ok((id.as_str().to_string(), plan.offer))
        }
        SequencePolicy::Preserve { timing: TimingPolicy::QuantizeEndpoints { .. }, .. } => {
            let mut exact_eff = engine.resolve_run(&run).map_err(|e| e.to_string())?;
            let mut exact_rep = RouteReport::default();
            if let Ok((id, _f, plan)) = select_muxer(
                engine,
                container,
                Arc::clone(&tracks),
                publication,
                MediaTimingPolicy::Exact,
                &mut exact_eff,
                &mut exact_rep,
            ) {
                return Ok((id.as_str().to_string(), plan.offer));
            }
            let (id, _f, plan) = select_muxer(
                engine,
                container,
                tracks,
                publication,
                MediaTimingPolicy::BestEffort,
                &mut eff,
                &mut report,
            )
            .map_err(|e| e.to_string())?;
            Ok((id.as_str().to_string(), plan.offer))
        }
        _ => {
            let (id, _f, plan) = select_muxer(
                engine,
                container,
                tracks,
                publication,
                MediaTimingPolicy::BestEffort,
                &mut eff,
                &mut report,
            )
            .map_err(|e| e.to_string())?;
            Ok((id.as_str().to_string(), plan.offer))
        }
    }
}

/// Static capability discovery probe for one `(container, codec, publication, timing)` tuple
/// using a nominal 64x64 @ 30 Hz probe track under `BestEffort` workspace policy and the
/// engine's live routing/ranking via `select_muxer`.
fn probe_capability_mux_offer(
    engine: &Engine,
    container: &ContainerId,
    codec: &MediaCodecId,
    publication: PublicationMode,
    timing: MediaTimingPolicy,
) -> std::result::Result<(String, MuxOffer), String> {
    validate_container_codec_tuple(container, codec).map_err(|e| e.to_string())?;
    let run = Run::new().with_workspace_policy(WorkspacePolicy::BestEffort);
    let mut eff = engine.resolve_run(&run).map_err(|e| e.to_string())?;
    let mut report = RouteReport::default();
    let tb = TimeBase::new(1, 30).map_err(|e| format!("failed to build probe timebase: {e}"))?;
    let trk = TrackInfo::video(0, codec.clone(), tb, VideoTrackInfo::new(64, 64))
        .build()
        .map_err(|e| format!("failed to build probe track: {e}"))?;
    let tracks: Arc<[TrackInfo]> = Arc::from(vec![trk]);
    let (id, _f, plan) =
        select_muxer(engine, container, tracks, publication, timing, &mut eff, &mut report)
            .map_err(|e| e.to_string())?;
    Ok((id.as_str().to_string(), plan.offer))
}

/// Build the truthful `v2/capabilities` report by querying LIVE `select_demuxer`,
/// `DemuxFactory::quote`, `select_muxer`, and `MuxFactory::quote` offers on `engine`
/// (using a nominal 64×64 @ 30 Hz capability probe track; job-specific `v2/estimate` and
/// `MediaSession` routes quote against the job's actual track, timing, routing, and limits).
#[must_use]
pub fn build_capabilities_report_for_engine(engine: &Engine) -> CapabilitiesReportV2 {
    let codecs = engine.codecs();
    let probe_run = Run::new().with_workspace_policy(WorkspacePolicy::BestEffort);

    let still_candidates: [(ImageFormat, &str); 7] = [
        (ImageFormat::Jpeg, "jpeg"),
        (ImageFormat::Png, "png"),
        (ImageFormat::WebP, "webp"),
        (ImageFormat::Gif, "gif"),
        (ImageFormat::Avif, "avif"),
        (ImageFormat::Jxl, "jxl"),
        (ImageFormat::Bmp, "bmp"),
    ];
    let mut still_formats = Vec::new();
    for (fmt, name) in still_candidates {
        let has_dec = codecs.factories::<ImageDecodeRole>(&fmt).next().is_some();
        let has_enc = codecs.factories::<ImageEncodeRole>(&fmt).next().is_some();
        if has_dec || has_enc {
            still_formats.push(name.to_string());
        }
    }

    let container_specs: [(ContainerId, &str); 3] = [
        (ContainerId::Mp4, "mp4"),
        (ContainerId::Webm, "webm"),
        (ContainerId::Ivf, "ivf"),
    ];
    let codec_specs: [(MediaCodecId, &str); 3] = [
        (MediaCodecId::Av1, "av1"),
        (MediaCodecId::Vp8, "vp8"),
        (MediaCodecId::Vp9, "vp9"),
    ];

    let mut media_containers = Vec::new();
    for (cid, name) in &container_specs {
        let has_demux = codecs.factories::<DemuxRole>(cid).next().is_some();
        let has_mux = codecs.factories::<MuxRole>(cid).next().is_some();
        if has_demux || has_mux {
            media_containers.push((*name).to_string());
        }
    }

    let mut video_codecs = Vec::new();
    for (vcid, name) in &codec_specs {
        let has_dec = codecs.factories::<VideoDecodeRole>(vcid).next().is_some();
        let has_enc = codecs.factories::<VideoEncodeRole>(vcid).next().is_some();
        if has_dec || has_enc {
            video_codecs.push((*name).to_string());
        }
    }

    let mut media_routes = Vec::new();
    for (cid, cname) in &container_specs {
        let demux_selected = engine.resolve_run(&probe_run).ok().and_then(|mut eff| {
            let mut rep = RouteReport::default();
            select_demuxer(engine, cid, &mut eff, &mut rep)
                .ok()
                .map(|(id, factory)| (id, factory, eff))
        });
        let demux_quote = demux_selected.as_ref().map(|(_, factory, eff)| {
            factory.quote(&DemuxNeeds::default(), &SourceAccess::positional(None), &eff.resources)
        });
        let demux_forward_ok = demux_selected.as_ref().is_some_and(|(_, factory, eff)| {
            factory
                .quote(&DemuxNeeds::default(), &SourceAccess::forward_stream(), &eff.resources)
                .is_ok()
        });

        for (vcid, vname) in &codec_specs {
            let tuple_valid = validate_container_codec_tuple(cid, vcid);
            let vdec_entry = codecs.factories::<VideoDecodeRole>(vcid).next();
            let venc_entry = codecs.factories::<VideoEncodeRole>(vcid).next();

            // 1. Decode route: derive strictly from tuple validity + selected DemuxOffer +
            //    VideoDecodeRole (NEVER borrow decode timing from a muxer quote!).
            let (dec_supported, dec_access, dec_timing, dec_provider, dec_refusal) =
                if let Err(e) = &tuple_valid {
                    (false, "none".to_string(), "unsupported", None, Some(e.to_string()))
                } else {
                    match (&demux_quote, vdec_entry) {
                        (Some(Ok(doff)), Some(vd)) => {
                            let access = match doff.required_access {
                                AccessPattern::Sequential => "forward_or_range",
                                AccessPattern::BoundedBacktrack { .. } => {
                                    if demux_forward_ok {
                                        "forward_or_range"
                                    } else {
                                        "bounded_backtrack_or_range"
                                    }
                                }
                                AccessPattern::RandomAccess => "range_required",
                                _ => "range_or_forward",
                            };
                            // Demuxers preserve exact container track rational timebases.
                            (
                                true,
                                access.to_string(),
                                "exact_rational",
                                Some(vd.id.as_str().to_string()),
                                None,
                            )
                        }
                        (Some(Err(e)), _) => {
                            (false, "none".to_string(), "unsupported", None, Some(e.to_string()))
                        }
                        (None, _) => (
                            false,
                            "none".to_string(),
                            "unsupported",
                            None,
                            Some(format!("no demuxer registered for container '{cname}'")),
                        ),
                        (_, None) => (
                            false,
                            "none".to_string(),
                            "unsupported",
                            None,
                            Some(format!("no video decoder registered for codec '{vname}'")),
                        ),
                    }
                };

            media_routes.push(MediaRouteCapabilityV2 {
                container: (*cname).to_string(),
                video_codec: (*vname).to_string(),
                direction: "decode".to_string(),
                layout: if !dec_supported {
                    "unsupported".to_string()
                } else if demux_forward_ok {
                    "streaming".to_string()
                } else {
                    "random_access".to_string()
                },
                access: dec_access,
                incremental: dec_supported && demux_forward_ok,
                timing: dec_timing.to_string(),
                supported: dec_supported,
                provider_id: dec_provider,
                refusal_reason: dec_refusal,
            });

            // 2. Encode route: require BOTH live selected MuxOffer for (container, codec) AND VideoEncodeRole!
            let exact_mux_ok = probe_capability_mux_offer(
                engine,
                cid,
                vcid,
                PublicationMode::Verified,
                MediaTimingPolicy::Exact,
            );
            let best_mux_ok = probe_capability_mux_offer(
                engine,
                cid,
                vcid,
                PublicationMode::Verified,
                MediaTimingPolicy::BestEffort,
            );
            let fwd_mux_ok = probe_capability_mux_offer(
                engine,
                cid,
                vcid,
                PublicationMode::Incremental,
                MediaTimingPolicy::BestEffort,
            );

            let (enc_supported, enc_layout, enc_access, enc_incremental, enc_timing, enc_provider, enc_refusal) =
                match (&best_mux_ok, venc_entry) {
                    (Ok((_, offer)), Some(ve)) => {
                        let layout = if offer.timing.streams() && !offer.timing.patches_at_finish() {
                            "streaming"
                        } else if offer.timing.streams() && offer.timing.patches_at_finish() {
                            "streaming_with_finish_patch"
                        } else {
                            "at_finish"
                        };
                        let fwd_streaming = fwd_mux_ok.as_ref().is_ok_and(|(_, fo)| {
                            fo.timing.streams() && !fo.timing.patches_at_finish()
                        });
                        let access = if fwd_streaming {
                            "forward_or_patch"
                        } else if offer.timing.patches_at_finish() {
                            "patch_or_staged"
                        } else {
                            "staged_at_finish"
                        };
                        let timing = if exact_mux_ok.is_ok() {
                            "exact_rational"
                        } else {
                            "quantized"
                        };
                        (
                            true,
                            layout.to_string(),
                            access.to_string(),
                            fwd_streaming,
                            timing,
                            Some(ve.id.as_str().to_string()),
                            None,
                        )
                    }
                    (Err(reason), _) => (
                        false,
                        "unsupported".to_string(),
                        "none".to_string(),
                        false,
                        "unsupported",
                        None,
                        Some(reason.clone()),
                    ),
                    (_, None) => (
                        false,
                        "unsupported".to_string(),
                        "none".to_string(),
                        false,
                        "unsupported",
                        None,
                        Some(format!("no video encoder registered for codec '{vname}'")),
                    ),
                };
            media_routes.push(MediaRouteCapabilityV2 {
                container: (*cname).to_string(),
                video_codec: (*vname).to_string(),
                direction: "encode".to_string(),
                layout: enc_layout,
                access: enc_access,
                incremental: enc_incremental,
                timing: enc_timing.to_string(),
                supported: enc_supported,
                provider_id: enc_provider,
                refusal_reason: enc_refusal,
            });
        }
    }

    // Score-only Verify metrics are advertised strictly when registered on Engine.
    let verify_metrics = Vec::new();

    CapabilitiesReportV2 {
        version: 2,
        wire_schema: "zenpipe-canonical-v1".to_string(),
        zen_media: cfg!(feature = "zen-media"),
        streaming_manual: cfg!(feature = "streaming"),
        streaming_async: cfg!(feature = "streaming-async"),
        cms_provider: Some(zencodecs::bundles::ids::MOXCMS.to_string()),
        still_formats,
        video_codecs,
        media_containers,
        verify_metrics,
        media_routes,
    }
}

/// Inspect a complete in-memory media container or still image byte slice (`v2/inspect`).
pub fn inspect_bytes(
    engine: &Engine,
    services: &HostServices,
    bytes: &[u8],
    expected_container: Option<&str>,
    bounded_index_scan: bool,
) -> Result<InspectReportV2> {
    inspect_bytes_with_stop(
        engine,
        services,
        bytes,
        expected_container,
        bounded_index_scan,
        None,
    )
}

/// Inspect a complete in-memory media container or still image byte slice with cooperative stop.
pub fn inspect_bytes_with_stop(
    engine: &Engine,
    services: &HostServices,
    bytes: &[u8],
    expected_container: Option<&str>,
    bounded_index_scan: bool,
    stop: Option<zc::SharedPulse>,
) -> Result<InspectReportV2> {
    let probe_limits = ProbeLimits::default();
    let media_probe = probe_container_prefix(engine.codecs(), bytes, &probe_limits)
        .map_err(crate::media::map_engine_error)?;

    match media_probe {
        ContainerProbeOutcome::Confirmed { container, .. } => {
            let container_str = container_token(&container);
            if let Some(exp) = expected_container
                && exp != container_str
            {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "declared video container '{}' does not match probed container '{}'",
                    exp,
                    container_str
                ));
            }
            inspect_media_container_bytes(
                engine,
                services,
                &container,
                bytes,
                bounded_index_scan,
                stop,
            )
        }
        ContainerProbeOutcome::NeedMore { .. } | ContainerProbeOutcome::NoMatch => {
            if let Some(exp) = expected_container {
                return Err(nerror!(
                    ErrorKind::ImageDecodingError,
                    "input bytes do not match declared video container '{}'",
                    exp
                ));
            }
            inspect_still_image(engine, services, bytes, stop)
        }
    }
}

/// Inspect a random-access [`ReadAtSource`] WITHOUT materializing the whole file into RAM!
///
/// Moves the single bounded prefix buffer directly into the demuxer loop (zero duplicate copy)
/// and deducts the retained window buffer from the demuxer's `max_memory_bytes` ceiling.
#[cfg(feature = "streaming")]
pub fn inspect_read_at_with_stop(
    engine: &Engine,
    services: &HostServices,
    src: &dyn crate::media::ReadAtSource,
    expected_container: Option<&str>,
    bounded_index_scan: bool,
    stop: Option<&dyn Stop>,
    pulse: Option<zc::SharedPulse>,
) -> Result<InspectReportV2> {
    let max_input = services
        .limits
        .and_then(|l| l.max_input_bytes)
        .unwrap_or(256 * 1024 * 1024);
    let max_mem = services.limits.and_then(|l| l.max_memory_bytes);
    let known_len = src.length()?;
    if let Some(len) = known_len
        && len > max_input
    {
        return Err(nerror!(
            ErrorKind::SizeLimitExceeded,
            "input length {} exceeds max_input_bytes {}",
            len,
            max_input
        ));
    }

    // Reserve at most half of tight memory caps (up to 64 KiB) for the single retained window buffer
    // so the demuxer retains an honest non-overlapping memory budget alongside `cur_buf`!
    let window_cap = match max_mem {
        Some(m) => usize::try_from(m / 2).unwrap_or(usize::MAX).min(65536).max(1),
        None => 65536,
    };
    let prefix_cap = window_cap.min(usize::try_from(max_input).unwrap_or(usize::MAX));
    let prefix = crate::media::bindings::read_exact_range_with_stop(src, 0, prefix_cap, stop)?;

    let probe_limits = ProbeLimits::default();
    let media_probe = probe_container_prefix(engine.codecs(), &prefix, &probe_limits)
        .map_err(crate::media::map_engine_error)?;

    match media_probe {
        ContainerProbeOutcome::Confirmed { container, .. } => {
            let container_str = container_token(&container);
            if let Some(exp) = expected_container
                && exp != container_str
            {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "declared video container '{}' does not match probed container '{}'",
                    exp,
                    container_str
                ));
            }
            // Move `prefix` directly into `inspect_media_container_read_at` (ZERO duplicate copy!).
            inspect_media_container_read_at(
                engine,
                services,
                &container,
                src,
                known_len,
                prefix,
                window_cap,
                bounded_index_scan,
                stop,
                pulse,
            )
        }
        ContainerProbeOutcome::NeedMore { .. } | ContainerProbeOutcome::NoMatch => {
            if let Some(exp) = expected_container {
                return Err(nerror!(
                    ErrorKind::ImageDecodingError,
                    "input bytes do not match declared video container '{}'",
                    exp
                ));
            }
            drop(prefix);
            let effective_cap = Some(max_mem.map_or(max_input, |m| m.min(max_input)));
            let full = crate::media::bindings::read_all_bounded_with_stop(src, effective_cap, stop)?;
            let mut rem_services = services.clone();
            if let Some(m) = max_mem {
                let rem = m.saturating_sub(full.len() as u64);
                let mut lim = rem_services.limits.unwrap_or_default();
                lim = lim.with_max_memory(rem);
                rem_services.limits = Some(lim);
            }
            inspect_still_image(engine, &rem_services, &full, pulse)
        }
    }
}

#[cfg(feature = "streaming")]
#[allow(clippy::too_many_arguments)]
fn inspect_media_container_read_at(
    engine: &Engine,
    services: &HostServices,
    container: &ContainerId,
    src: &dyn crate::media::ReadAtSource,
    known_len: Option<u64>,
    mut cur_buf: Vec<u8>,
    window_budget: usize,
    bounded_index_scan: bool,
    stop: Option<&dyn Stop>,
    pulse: Option<zc::SharedPulse>,
) -> Result<InspectReportV2> {
    // Deduct `window_budget` from the demuxer's memory cap so `cur_buf` + demuxer never
    // overlap beyond the configured `max_memory_bytes`!
    let mut adj_services = services.clone();
    if let Some(mut lim) = adj_services.limits {
        if let Some(m) = lim.max_memory_bytes {
            lim = lim.with_max_memory(m.saturating_sub(window_budget as u64));
        }
        adj_services.limits = Some(lim);
    }
    let run = build_run_for_services(&adj_services, pulse);
    let mut eff = engine.resolve_run(&run).map_err(crate::media::map_engine_error)?;
    let mut rep = RouteReport::default();
    let (_demux_id, demux_factory) =
        select_demuxer(engine, container, &mut eff, &mut rep).map_err(crate::media::map_engine_error)?;
    let source_access = SourceAccess::positional(known_len);
    let offer = demux_factory
        .quote(&DemuxNeeds::default(), &source_access, &eff.resources)
        .map_err(|e| nerror!(ErrorKind::ImageDecodingError, "demux quote failed: {}", e))?;
    let mut demux = demux_factory
        .open(&offer, &eff.resources)
        .map_err(|e| nerror!(ErrorKind::ImageDecodingError, "demux open failed: {}", e))?;

    let max_input = services
        .limits
        .and_then(|l| l.max_input_bytes)
        .unwrap_or(256 * 1024 * 1024);

    let mut total_read: u64 = cur_buf.len() as u64;
    let mut headers_tracks = None;
    let mut scanned_packets: u64 = 0;
    let mut warnings = Vec::new();
    let max_steps = 4096usize;
    let mut cur_offset: u64 = 0;
    let mut cur_buf_offset: u64 = 0;
    let mut cur_req_len: usize = window_budget;

    for _ in 0..max_steps {
        if let Some(s) = stop
            && s.should_stop()
        {
            return Err(nerror!(ErrorKind::OperationCancelled, "inspect cancelled or timed out"));
        }
        if eff.resources.pulse().should_stop() {
            return Err(nerror!(ErrorKind::OperationCancelled, "inspect cancelled or timed out"));
        }
        let need_fetch = cur_offset < cur_buf_offset
            || cur_offset >= cur_buf_offset.saturating_add(cur_buf.len() as u64);
        if need_fetch {
            if cur_offset > max_input {
                return Err(nerror!(
                    ErrorKind::SizeLimitExceeded,
                    "inspect offset {} exceeds max_input_bytes {}",
                    cur_offset,
                    max_input
                ));
            }
            cur_buf.clear();
            cur_buf.shrink_to_fit();
            let fetch_len = cur_req_len
                .max(1)
                .min(window_budget)
                .min(usize::try_from(max_input.saturating_sub(cur_offset)).unwrap_or(usize::MAX));
            cur_buf =
                crate::media::bindings::read_exact_range_with_stop(src, cur_offset, fetch_len, stop)?;
            cur_buf_offset = cur_offset;
            total_read = total_read.saturating_add(cur_buf.len() as u64);
        }
        let rel = usize::try_from(cur_offset - cur_buf_offset).unwrap_or(cur_buf.len());
        let slice = &cur_buf[rel..];
        let inp = Input::new(cur_offset, slice).with_total_len_opt(known_len);
        let prog = demux
            .step(inp, Some(eff.resources.pulse()))
            .map_err(|e| nerror!(ErrorKind::ImageDecodingError, "demux step failed: {}", e))?;
        cur_offset = cur_offset.saturating_add(prog.consumed as u64);
        match prog.event {
            DemuxEvent::Need => {
                let req = demux
                    .requested()
                    .first()
                    .copied()
                    .unwrap_or(InputRequest::at_least(cur_offset, 1));
                match req {
                    InputRequest::AtLeast { offset, min_len } => {
                        if min_len > window_budget {
                            return Err(nerror!(
                                ErrorKind::SizeLimitExceeded,
                                "inspect AtLeast {} B exceeds window memory budget {} B",
                                min_len,
                                window_budget
                            ));
                        }
                        cur_offset = offset;
                        cur_req_len = min_len.max(window_budget.min(65536));
                        let avail = cur_buf_offset
                            .saturating_add(cur_buf.len() as u64)
                            .saturating_sub(cur_offset);
                        if cur_offset < cur_buf_offset || avail < min_len as u64 {
                            cur_buf.clear();
                            cur_buf_offset = cur_offset;
                        }
                    }
                    InputRequest::Range(r) => {
                        let need = usize::try_from(r.length).unwrap_or(usize::MAX);
                        if need > window_budget {
                            return Err(nerror!(
                                ErrorKind::SizeLimitExceeded,
                                "inspect range {} B exceeds window memory budget {} B",
                                need,
                                window_budget
                            ));
                        }
                        cur_offset = r.offset;
                        cur_req_len = need;
                        let covered = r.offset >= cur_buf_offset
                            && r.offset.saturating_add(r.length)
                                <= cur_buf_offset.saturating_add(cur_buf.len() as u64);
                        if !covered {
                            cur_buf.clear();
                            cur_buf_offset = cur_offset;
                        }
                    }
                    InputRequest::Len => {
                        cur_offset = 0;
                        cur_req_len = 0;
                    }
                    _ => {
                        cur_req_len = window_budget;
                    }
                }
            }
            DemuxEvent::Headers => {
                headers_tracks = Some(demux.tracks().to_vec());
                if !bounded_index_scan {
                    break;
                }
            }
            DemuxEvent::Packet(_pkt) => {
                scanned_packets = scanned_packets.saturating_add(1);
                if scanned_packets >= 2048 {
                    warnings.push("bounded_index scan reached packet scan cap (2048)".to_string());
                    break;
                }
            }
            DemuxEvent::Done => break,
            _ => break,
        }
    }

    if let Some(s) = stop
        && s.should_stop()
    {
        return Err(nerror!(ErrorKind::OperationCancelled, "inspect cancelled or timed out"));
    }
    if eff.resources.pulse().should_stop() {
        return Err(nerror!(ErrorKind::OperationCancelled, "inspect cancelled or timed out"));
    }

    finalize_inspect_tracks(
        container,
        known_len,
        total_read,
        headers_tracks,
        bounded_index_scan,
        scanned_packets,
        warnings,
    )
}

fn inspect_media_container_bytes(
    engine: &Engine,
    services: &HostServices,
    container: &ContainerId,
    bytes: &[u8],
    bounded_index_scan: bool,
    stop: Option<zc::SharedPulse>,
) -> Result<InspectReportV2> {
    let run = build_run_for_services(services, stop);
    let mut eff = engine.resolve_run(&run).map_err(crate::media::map_engine_error)?;
    if eff.resources.pulse().should_stop() {
        return Err(nerror!(ErrorKind::OperationCancelled, "inspect cancelled or timed out"));
    }
    let mut rep = RouteReport::default();
    let (_demux_id, demux_factory) =
        select_demuxer(engine, container, &mut eff, &mut rep).map_err(crate::media::map_engine_error)?;
    let source_access = SourceAccess::borrowed(bytes.len() as u64);
    let offer = demux_factory
        .quote(&DemuxNeeds::default(), &source_access, &eff.resources)
        .map_err(|e| nerror!(ErrorKind::ImageDecodingError, "demux quote failed: {}", e))?;
    let mut demux = demux_factory
        .open(&offer, &eff.resources)
        .map_err(|e| nerror!(ErrorKind::ImageDecodingError, "demux open failed: {}", e))?;

    let mut consumed_high_water: u64 = 0;
    let mut headers_tracks = None;
    let mut scanned_packets: u64 = 0;
    let mut warnings = Vec::new();
    let max_steps = 4096usize;
    let mut cur_offset: u64 = 0;
    let mut cur_len: usize = bytes.len().min(65536);

    for _ in 0..max_steps {
        if eff.resources.pulse().should_stop() {
            return Err(nerror!(ErrorKind::OperationCancelled, "inspect cancelled or timed out"));
        }
        let start = usize::try_from(cur_offset).unwrap_or(bytes.len()).min(bytes.len());
        let end = start.saturating_add(cur_len).min(bytes.len());
        let slice = &bytes[start..end];
        consumed_high_water = consumed_high_water.max(end as u64);
        let inp = Input::new(cur_offset, slice).with_total_len(bytes.len() as u64);
        let prog = demux
            .step(inp, Some(eff.resources.pulse()))
            .map_err(|e| nerror!(ErrorKind::ImageDecodingError, "demux step failed: {}", e))?;
        cur_offset = cur_offset.saturating_add(prog.consumed as u64);
        match prog.event {
            DemuxEvent::Need => {
                let req = demux
                    .requested()
                    .first()
                    .copied()
                    .unwrap_or(InputRequest::at_least(cur_offset, 1));
                match req {
                    InputRequest::AtLeast { offset, min_len } => {
                        cur_offset = offset;
                        cur_len = min_len.max(65536);
                    }
                    InputRequest::Range(r) => {
                        cur_offset = r.offset;
                        cur_len = usize::try_from(r.length).unwrap_or(usize::MAX);
                    }
                    InputRequest::Len => {
                        cur_offset = 0;
                        cur_len = 0;
                    }
                    _ => {
                        cur_len = 65536;
                    }
                }
            }
            DemuxEvent::Headers => {
                headers_tracks = Some(demux.tracks().to_vec());
                if !bounded_index_scan {
                    break;
                }
            }
            DemuxEvent::Packet(_pkt) => {
                scanned_packets = scanned_packets.saturating_add(1);
                if scanned_packets >= 2048 {
                    warnings.push("bounded_index scan reached packet scan cap (2048)".to_string());
                    break;
                }
            }
            DemuxEvent::Done => break,
            _ => break,
        }
    }

    if eff.resources.pulse().should_stop() {
        return Err(nerror!(ErrorKind::OperationCancelled, "inspect cancelled or timed out"));
    }

    finalize_inspect_tracks(
        container,
        Some(bytes.len() as u64),
        consumed_high_water,
        headers_tracks,
        bounded_index_scan,
        scanned_packets,
        warnings,
    )
}

fn finalize_inspect_tracks(
    container: &ContainerId,
    known_len: Option<u64>,
    consumed_bytes: u64,
    headers_tracks: Option<Vec<zencodec_media::track::TrackInfo>>,
    bounded_index_scan: bool,
    scanned_packets: u64,
    warnings: Vec<String>,
) -> Result<InspectReportV2> {
    let raw_tracks = headers_tracks.ok_or_else(|| {
        nerror!(
            ErrorKind::ImageDecodingError,
            "media container headers could not be parsed within bounded probe"
        )
    })?;

    let is_ivf = matches!(container, ContainerId::Ivf);
    let evidence = if bounded_index_scan { "indexed" } else { "headers" };
    let mut tracks = Vec::with_capacity(raw_tracks.len());
    for (idx, trk) in raw_tracks.iter().enumerate() {
        let id = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        let kind = match trk.kind() {
            TrackKind::Video => "video",
            TrackKind::Audio => "audio",
            TrackKind::Subtitle => "subtitle",
            TrackKind::Data => "data",
            _ => "other",
        };
        let tb = trk.time_base();
        let clock = Some(ClockV2 { numerator: tb.numerator(), denominator: tb.denominator() });
        let duration_ticks = trk.declared_duration_ticks().map(|d| d.to_string());
        let presentation_count = if bounded_index_scan && scanned_packets > 0 {
            Some(scanned_packets.to_string())
        } else {
            trk.declared_packets().map(|n| n.to_string())
        };
        let (coded, display) = match trk.video_info() {
            Some(v) => {
                let (dw, dh) = v.display.unwrap_or((v.coded_width, v.coded_height));
                (
                    Some(DimensionsV2 { width: v.coded_width, height: v.coded_height }),
                    Some(DimensionsV2 { width: dw, height: dh }),
                )
            }
            None => (None, None),
        };
        let codec = video_codec_token(trk.codec());
        let seek_scopes = if is_ivf {
            vec!["any".to_string()]
        } else {
            vec!["any".to_string(), "random_access_points".to_string()]
        };
        tracks.push(InspectTrackV2 {
            id,
            kind: kind.to_string(),
            codec,
            evidence: evidence.to_string(),
            coded,
            display,
            clock,
            duration_ticks,
            presentation_count,
            seek_scopes,
        });
    }

    Ok(InspectReportV2 {
        container: container_token(container).to_string(),
        length_bytes: known_len.map(|l| l.to_string()),
        consumed_input_bytes: consumed_bytes.to_string(),
        tracks,
        warnings,
    })
}

fn inspect_still_image(
    engine: &Engine,
    services: &HostServices,
    bytes: &[u8],
    stop: Option<zc::SharedPulse>,
) -> Result<InspectReportV2> {
    let mut run = Run::new().with_workspace_policy(services.workspace_policy);
    if let Some(lim) = services.limits {
        run = run.with_limits(lim);
    }
    if let Some(p) = stop {
        run = run.with_pulse(p);
    }
    let probed = engine
        .probe(bytes, &run)
        .map_err(crate::media::map_engine_error)?;
    let env = &probed.inspection().envelope;
    let fmt_name = env
        .image_format
        .map_or_else(|| "unknown".to_string(), |f| format!("{f:?}").to_ascii_lowercase());
    let track = InspectTrackV2 {
        id: 1,
        kind: "image".to_string(),
        codec: fmt_name.clone(),
        evidence: "headers".to_string(),
        coded: Some(DimensionsV2 { width: env.width, height: env.height }),
        display: Some(DimensionsV2 { width: env.width, height: env.height }),
        clock: None,
        duration_ticks: None,
        presentation_count: env.frame_count.map(|n| n.to_string()),
        seek_scopes: vec!["any".to_string()],
    };
    Ok(InspectReportV2 {
        container: fmt_name,
        length_bytes: Some(bytes.len().to_string()),
        consumed_input_bytes: bytes.len().to_string(),
        tracks: vec![track],
        warnings: Vec::new(),
    })
}

/// Select the exact video track descriptor (`(coded_width, coded_height)` and `TimeBase`)
/// determined by `track_policy` from parsed container tracks, mirroring
/// `zencodecs::media::route::validate_track_policy` (NEVER blindly assuming video track 0!).
pub(crate) fn select_video_track_descriptor(
    tracks: &[InspectTrackV2],
    track_policy: &TrackPolicy,
) -> Option<((u32, u32), TimeBase)> {
    let selected = match track_policy {
        TrackPolicy::Preserve | TrackPolicy::DropNonVideo => {
            let mut video_iter = tracks.iter().filter(|t| t.kind == "video");
            let first = video_iter.next()?;
            // Ambiguous multi-video input cannot be resolved without explicit `TrackPolicy::Select`.
            if video_iter.next().is_some() {
                return None;
            }
            first
        }
        TrackPolicy::Select { video, .. } => {
            let t = tracks.get(*video as usize)?;
            if t.kind != "video" {
                return None;
            }
            t
        }
        _ => return None,
    };
    let coded = selected.coded.as_ref()?;
    if coded.width == 0 || coded.height == 0 {
        return None;
    }
    let clock = selected.clock.as_ref()?;
    let tb = TimeBase::new(clock.numerator, clock.denominator).ok()?;
    Some(((coded.width, coded.height), tb))
}

/// Extract the `track_policy`-selected video track's `(coded_width, coded_height)` and
/// `TimeBase` from a container prefix if headers are already complete inside `prefix`,
/// respecting `services` (`workspace_policy`, `limits`, `routing`).
pub(crate) fn probe_selected_video_track_descriptor(
    engine: &Engine,
    services: &HostServices,
    prefix: &[u8],
    track_policy: &TrackPolicy,
    stop: Option<zc::SharedPulse>,
) -> Option<((u32, u32), TimeBase)> {
    let rep = inspect_bytes_with_stop(engine, services, prefix, None, false, stop).ok()?;
    select_video_track_descriptor(&rep.tracks, track_policy)
}

/// Compute a non-mutating `v2/estimate` admission report with real [`StartupDependenciesV2`]
/// derived directly from live route-selected `select_demuxer` and `select_muxer` offers
/// using the job's actual `TrackPolicy`-selected track, timing policy, publication mode,
/// routing, workspace policy, and limits!
pub fn build_estimate_report(
    engine: &Engine,
    rt: &PipelineRuntime,
    job: &zenpipe::canonical::CanonicalJob,
    bindings: zenpipe::canonical::Bindings<'_>,
    input_prefixes: &[(&str, &[u8])],
    services: &HostServices,
    stop: Option<&dyn Stop>,
) -> Result<EstimateReportV2> {
    let check_stop = || -> Result<()> {
        if let Some(s) = stop
            && s.should_stop()
        {
            return Err(nerror!(ErrorKind::OperationCancelled, "estimate cancelled or timed out"));
        }
        Ok(())
    };
    check_stop()?;
    let prepared = rt.prepare(job, bindings).map_err(crate::media::map_pipe_error)?;
    check_stop()?;
    let adm = prepared.admission();
    let workspace_policy = match services.workspace_policy {
        WorkspacePolicy::Strict => "strict",
        WorkspacePolicy::BestEffort => "best_effort",
    };

    let mut needs_length = false;
    let mut needs_range = false;
    let mut needs_patch_known_true = false;
    let mut needs_patch_unknown = false;
    let mut needs_replay = false;
    let mut warnings = adm.warnings.clone();

    let probe_run = build_run_for_services(services, None);
    let probe_limits = ProbeLimits::default();
    let mut random_access_slots = BTreeSet::new();
    let mut slot_tracks: BTreeMap<&str, Vec<InspectTrackV2>> = BTreeMap::new();

    for &(slot, prefix) in input_prefixes {
        check_stop()?;
        if let Ok(ContainerProbeOutcome::Confirmed { container, .. }) =
            probe_container_prefix(engine.codecs(), prefix, &probe_limits)
        {
            if let Ok(mut eff) = engine.resolve_run(&probe_run) {
                let mut rep = RouteReport::default();
                if let Ok((_did, demux_factory)) =
                    select_demuxer(engine, &container, &mut eff, &mut rep)
                    && let Ok(offer) = demux_factory.quote(
                        &DemuxNeeds::default(),
                        &SourceAccess::positional(None),
                        &eff.resources,
                    )
                {
                    if !matches!(offer.required_access, AccessPattern::Sequential) {
                        needs_range = true;
                        needs_length = true;
                        random_access_slots.insert(slot);
                    }
                }
            }
            if let Ok(rep) = inspect_bytes_with_stop(engine, services, prefix, None, false, None) {
                slot_tracks.insert(slot, rep.tracks);
            }
        }
    }

    let mut seen_decode_slots = BTreeSet::new();
    for op in job.graph().nodes() {
        if let ImageOp::Decode(dec) = op {
            if !seen_decode_slots.insert(dec.slot.as_str()) {
                needs_replay = true;
            }
            if dec.select.is_some() && random_access_slots.contains(dec.slot.as_str()) {
                needs_range = true;
            }
        }
    }

    // Consult the LIVE route-selected MuxOffer for each video output ONLY when that specific
    // output's `TrackPolicy`-selected upstream video track descriptor is actually known, AND
    // strictly under the requested `enc.intent.publication` mode (NEVER borrowing track 0,
    // fabricating a synthetic 64×64 @ 30 Hz track, or substituting a `Verified` quote for a
    // failed `Incremental` request!).
    for out_op in job.graph().outputs() {
        check_stop()?;
        if let OutputOp::Encode(enc) = out_op {
            if let TargetFormat::Video { ref container, ref codec } = enc.intent.target {
                let direct_track = match job.graph().node(enc.src) {
                    Some(ImageOp::Decode(dec)) if dec.select.is_none() => slot_tracks
                        .get(dec.slot.as_str())
                        .and_then(|tracks| select_video_track_descriptor(tracks, &enc.intent.tracks)),
                    _ => None,
                };
                if let Some((route_dims, route_tb)) = direct_track {
                    match select_mux_offer_for_route(
                        engine,
                        services,
                        container,
                        codec,
                        enc.intent.publication,
                        &enc.intent.sequence,
                        route_dims,
                        route_tb,
                    ) {
                        Ok((_, offer)) => {
                            if offer.timing.patches_at_finish() {
                                needs_patch_known_true = true;
                            }
                        }
                        Err(reason) => {
                            needs_patch_unknown = true;
                            warnings.push(format!(
                                "video output '{}' requested publication route could not be quoted: {}",
                                enc.slot.as_str(),
                                reason
                            ));
                        }
                    }
                } else {
                    needs_patch_unknown = true;
                    warnings.push(format!(
                        "video output '{}' patch requirement unknown until container headers/selected track descriptor are available",
                        enc.slot.as_str()
                    ));
                }
            }
            if matches!(enc.intent.coding, Coding::Verify(_))
                || matches!(enc.intent.metadata, MetadataPolicy::PreserveBytes)
            {
                needs_replay = true;
            }
        }
    }

    let needs_patch = if needs_patch_known_true {
        Some(true)
    } else if needs_patch_unknown {
        None
    } else {
        Some(false)
    };

    check_stop()?;

    Ok(EstimateReportV2 {
        version: 2,
        node_count: adm.node_count,
        output_count: adm.output_count,
        input_slots: adm.input_slots.iter().map(|s| s.as_str().to_string()).collect(),
        output_slots: adm.output_slots.iter().map(|s| s.as_str().to_string()).collect(),
        input_bytes: adm.input_bytes.to_string(),
        workspace_policy: workspace_policy.to_string(),
        startup_dependencies: StartupDependenciesV2 {
            needs_length,
            needs_range,
            needs_patch,
            needs_replay,
        },
        warnings,
    })
}

/// Canonical wire container token (`"mp4"`, `"webm"`, `"ivf"`).
#[must_use]
pub fn container_token(c: &ContainerId) -> &'static str {
    match c {
        ContainerId::Mp4 => "mp4",
        ContainerId::Webm => "webm",
        ContainerId::Ivf => "ivf",
        _ => "unknown",
    }
}

/// Canonical wire video codec token (`"av1"`, `"vp8"`, `"vp9"`).
#[must_use]
pub fn video_codec_token(c: &MediaCodecId) -> String {
    match c {
        MediaCodecId::Av1 => "av1".to_string(),
        MediaCodecId::Vp8 => "vp8".to_string(),
        MediaCodecId::Vp9 => "vp9".to_string(),
        other => format!("{other:?}").to_ascii_lowercase(),
    }
}
