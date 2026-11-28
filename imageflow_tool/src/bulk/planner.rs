//! Route-aware coding planner, no-upscale geometry breakpoint solver, canonical single-decode
//! multi-output `zenpipe` DAG builder, and `<picture>`/`srcset` HTML snippet renderer.

use super::schema::{
    BulkLimits, FitMode, ManifestAsset, ManifestVariant, OutputFormat, QualityFallbackPolicy,
    QualityTarget, ResponsivePreset, WorkspaceAdmission,
};
use serde_json::{Map, Value, json};

/// Resolved coding configuration for a single output variant, determined UP FRONT before
/// executing the pipeline so runtime errors (`TargetUnmet`, `PolicyRefusal`) are never masked.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedCoding {
    pub coding_json: Value,
    pub requested_label: String,
    pub effective_label: String,
    pub fallback_reason: Option<String>,
}

/// Query live `v2/capabilities` once to check whether `zensim_a` closed-loop verification is
/// compiled into the linked `imageflow_core` build.
pub fn live_supports_zensim_a_verify() -> bool {
    static CACHED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *CACHED.get_or_init(|| {
        let Ok(mut ctx) = imageflow_core::Context::create() else {
            return false;
        };
        let (resp, res) = ctx.message("v2/capabilities", b"{}");
        if res.is_err() {
            return false;
        }
        let Ok(v) = serde_json::from_slice::<Value>(&resp.response_json) else {
            return false;
        };
        v["data"]["verify_metrics"]
            .as_array()
            .is_some_and(|arr| arr.iter().any(|m| m.as_str() == Some("zensim_a")))
    })
}

/// Resolve the exact `zenpipe` `encode.coding` JSON payload for `(format, quality, fallback, workspace)`
/// based on live capabilities + provider truth table:
/// - `PNG`: lossless-only (`"lossless"`).
/// - `JPEG`: closed-loop `Verify(zensim_a)` & `AimFor(zensim_a)` supported under both `Strict` and `BestEffort` (when `zensim_a` is compiled in).
/// - `WebP`: `Verify(zensim_a)` supported under `BestEffort` (refused under `Strict`); `AimFor(zensim_a)` supported via calibrated fidelity path.
/// - `AVIF` / `JXL`: encoder lacks qualified closed-loop verify mapping -> falls back to `calibrated_quality`
///   under `QualityFallbackPolicy::Calibrated` or fails up front under `QualityFallbackPolicy::StrictVerify`.
pub fn resolve_variant_coding(
    format: OutputFormat,
    quality: &QualityTarget,
    fallback: QualityFallbackPolicy,
    workspace: WorkspaceAdmission,
) -> Result<ResolvedCoding, String> {
    resolve_variant_coding_with_caps(
        format,
        quality,
        fallback,
        workspace,
        live_supports_zensim_a_verify(),
    )
}

/// Deterministic capability-parameterized coding resolver (used by tests & live planner).
pub fn resolve_variant_coding_with_caps(
    format: OutputFormat,
    quality: &QualityTarget,
    fallback: QualityFallbackPolicy,
    workspace: WorkspaceAdmission,
    has_zensim_a_iqa: bool,
) -> Result<ResolvedCoding, String> {
    let requested_label = quality.display_label();

    if format.is_lossless_only() {
        let reason = if matches!(quality, QualityTarget::Lossless) {
            None
        } else {
            Some("lossless_format_uses_lossless".to_string())
        };
        return Ok(ResolvedCoding {
            coding_json: json!("lossless"),
            requested_label,
            effective_label: "lossless".to_string(),
            fallback_reason: reason,
        });
    }

    match *quality {
        QualityTarget::Lossless => Ok(ResolvedCoding {
            coding_json: json!("lossless"),
            requested_label,
            effective_label: "lossless".to_string(),
            fallback_reason: None,
        }),
        QualityTarget::CalibratedQuality { quality: q } => Ok(ResolvedCoding {
            coding_json: json!({ "calibrated_quality": q }),
            requested_label,
            effective_label: format!("calibrated_quality({q:.1})"),
            fallback_reason: None,
        }),
        QualityTarget::QualityProfile { level, dpr } => Ok(ResolvedCoding {
            coding_json: json!({
                "quality_profile": { "level": level.as_wire_str(), "dpr": dpr }
            }),
            requested_label,
            effective_label: format!("profile({}@{dpr:.1}x)", level.as_wire_str()),
            fallback_reason: None,
        }),
        QualityTarget::VerifyZensimA { target } => {
            let can_verify = has_zensim_a_iqa
                && match format {
                    OutputFormat::Jpeg => true,
                    OutputFormat::Webp => workspace == WorkspaceAdmission::BestEffort,
                    OutputFormat::Avif | OutputFormat::Jxl => false,
                    OutputFormat::Png
                    | OutputFormat::Mp4Av1
                    | OutputFormat::WebmVp9
                    | OutputFormat::WebmVp8 => false,
                };
            if can_verify {
                Ok(ResolvedCoding {
                    coding_json: json!({
                        "verify": { "metric": "zensim_a", "value": target }
                    }),
                    requested_label,
                    effective_label: format!("verify(zensim_a>={target:.1})"),
                    fallback_reason: None,
                })
            } else {
                let reason = if !has_zensim_a_iqa {
                    "iqa_zensim_a_unavailable"
                } else {
                    match format {
                        OutputFormat::Webp => "webp_verify_requires_best_effort_workspace",
                        OutputFormat::Avif => "avif_encoder_verify_unavailable",
                        OutputFormat::Jxl => "jxl_encoder_verify_unavailable",
                        _ => "format_verify_unavailable",
                    }
                };
                if fallback == QualityFallbackPolicy::StrictVerify {
                    return Err(format!(
                        "strict_verify refused format '{}' under workspace '{}': {}",
                        format,
                        workspace.as_wire_str(),
                        reason
                    ));
                }
                Ok(ResolvedCoding {
                    coding_json: json!({ "calibrated_quality": target }),
                    requested_label,
                    effective_label: format!("calibrated_quality({target:.1})"),
                    fallback_reason: Some(reason.to_string()),
                })
            }
        }
        QualityTarget::AimForZensimA { target } => {
            // In pinned zenpipe, AimFor computes a calibrated quality seed from the encoder's
            // fidelity model without requiring a registered Verify IQA scorer (JPEG + WebP).
            let can_aim = matches!(format, OutputFormat::Jpeg | OutputFormat::Webp);
            if can_aim {
                Ok(ResolvedCoding {
                    coding_json: json!({
                        "aim_for": { "metric": "zensim_a", "value": target }
                    }),
                    requested_label,
                    effective_label: format!("aim_for(zensim_a~{target:.1}) [estimated]"),
                    fallback_reason: None,
                })
            } else if fallback == QualityFallbackPolicy::StrictVerify {
                Err(format!(
                    "strict_verify refused aim_for(zensim_a) on format '{}'",
                    format
                ))
            } else {
                Ok(ResolvedCoding {
                    coding_json: json!({ "calibrated_quality": target }),
                    requested_label,
                    effective_label: format!("calibrated_quality({target:.1})"),
                    fallback_reason: Some("format_aim_for_unavailable".to_string()),
                })
            }
        }
    }
}

/// One resolved unique target breakpoint `(width, height)` for a source image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlannedBreakpoint {
    pub width: u32,
    pub height: u32,
    pub fit_mode: FitMode,
    pub bypass_resize: bool,
}

/// Resolve authoritative responsive breakpoints for upright source dimensions `(src_w, src_h)`.
///
/// Enforces no-upscale on BOTH width and height when `!preset.allow_upscale`, deduplicates
/// identical `(width, height)` pairs, and guarantees at least one breakpoint is emitted.
pub fn plan_breakpoints(src_w: u32, src_h: u32, preset: &ResponsivePreset) -> Vec<PlannedBreakpoint> {
    let src_w = src_w.max(1);
    let src_h = src_h.max(1);
    let mut sorted_widths = preset.widths_px.clone();
    sorted_widths.sort_unstable();
    sorted_widths.dedup();

    let mut planned: Vec<PlannedBreakpoint> = Vec::with_capacity(sorted_widths.len());
    for req_w in sorted_widths {
        let (tw, th, bypass) = if let Some(ar) = preset.aspect_ratio {
            let ar_w = u64::from(ar.width.max(1));
            let ar_h = u64::from(ar.height.max(1));
            let ideal_h = ((u64::from(req_w) * ar_h + ar_w / 2) / ar_w)
                .clamp(1, u64::from(u32::MAX)) as u32;
            if preset.allow_upscale {
                (req_w, ideal_h, req_w == src_w && ideal_h == src_h)
            } else {
                // Fit desired aspect ratio strictly inside (src_w, src_h) with zero upscaling on either axis.
                let max_w_from_h = (u64::from(src_h) * ar_w / ar_h)
                    .clamp(1, u64::from(u32::MAX)) as u32;
                let clamped_w = req_w.min(src_w).min(max_w_from_h).max(1);
                let clamped_h = ((u64::from(clamped_w) * ar_h + ar_w / 2) / ar_w)
                    .clamp(1, u64::from(src_h)) as u32;
                (clamped_w, clamped_h, clamped_w == src_w && clamped_h == src_h)
            }
        } else {
            let tw = if preset.allow_upscale { req_w } else { req_w.min(src_w) }.max(1);
            let th_u64 = ((u64::from(tw) * u64::from(src_h) + u64::from(src_w) / 2)
                / u64::from(src_w))
            .max(1);
            let th = if preset.allow_upscale {
                th_u64.clamp(1, u64::from(u32::MAX)) as u32
            } else {
                th_u64.clamp(1, u64::from(src_h)) as u32
            };
            (tw, th, tw == src_w && th == src_h)
        };

        let bp = PlannedBreakpoint {
            width: tw,
            height: th,
            fit_mode: preset.fit_mode,
            bypass_resize: bypass,
        };
        if !planned.iter().any(|p| p.width == bp.width && p.height == bp.height) {
            planned.push(bp);
        }
    }

    if planned.is_empty() {
        planned.push(PlannedBreakpoint {
            width: src_w,
            height: src_h,
            fit_mode: preset.fit_mode,
            bypass_resize: true,
        });
    }
    planned
}

/// Metadata for one output slot in a multi-output `v2/execute` DAG.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedOutputSlot {
    pub slot_name: String,
    pub io_id: i32,
    pub breakpoint: PlannedBreakpoint,
    pub format: OutputFormat,
    pub resolved_coding: ResolvedCoding,
}

/// Build a canonical single-decode, single-resize-per-breakpoint, multi-format fan-out
/// `v2/execute` JSON envelope for a still image.
pub fn build_still_fanout_envelope(
    breakpoints: &[PlannedBreakpoint],
    preset: &ResponsivePreset,
    limits: &BulkLimits,
) -> Result<(Value, Vec<PlannedOutputSlot>), String> {
    build_fanout_envelope_inner(breakpoints, preset, limits, false)
}

/// Build a single-decode multi-variant poster extraction envelope from a video or animation
/// (`decode.select = { "presentation_index": { "index": 0 } }`).
pub fn build_video_poster_envelope(
    breakpoints: &[PlannedBreakpoint],
    preset: &ResponsivePreset,
    limits: &BulkLimits,
) -> Result<(Value, Vec<PlannedOutputSlot>), String> {
    build_fanout_envelope_inner(breakpoints, preset, limits, true)
}

fn build_fanout_envelope_inner(
    breakpoints: &[PlannedBreakpoint],
    preset: &ResponsivePreset,
    limits: &BulkLimits,
    select_first_presentation: bool,
) -> Result<(Value, Vec<PlannedOutputSlot>), String> {
    let mut nodes = Map::new();
    let mut edges: Vec<Value> = Vec::new();
    let mut outputs_map = Map::new();
    let mut slots: Vec<PlannedOutputSlot> = Vec::new();

    let decode_payload = if select_first_presentation {
        json!({
            "input": "src",
            "orientation": "apply",
            "select": { "presentation_index": { "index": 0 } }
        })
    } else {
        json!({
            "input": "src",
            "orientation": "apply"
        })
    };
    nodes.insert("dec".to_string(), json!({ "decode": decode_payload }));

    let mut next_io_id: i32 = 1;
    for (bi, bp) in breakpoints.iter().enumerate() {
        let mut upstream_name = "dec".to_string();

        if !bp.bypass_resize {
            let geo_name = format!("geo_{bi}");
            nodes.insert(
                geo_name.clone(),
                json!({
                    "geometry": {
                        "constrain": {
                            "mode": bp.fit_mode.as_wire_str(),
                            "width": bp.width,
                            "height": bp.height
                        }
                    }
                }),
            );
            edges.push(json!({ "from": upstream_name, "to": geo_name }));
            upstream_name = geo_name;

            if let Some(amt) = preset.sharpen_amount
                && amt > 0.0
            {
                let sharp_name = format!("sharp_{bi}");
                nodes.insert(
                    sharp_name.clone(),
                    json!({
                        "filter": {
                            "sharpen": { "amount": amt, "radius": 1.0 }
                        }
                    }),
                );
                edges.push(json!({ "from": upstream_name, "to": sharp_name }));
                upstream_name = sharp_name;
            }
        }

        for &fmt in &preset.formats {
            let resolved = resolve_variant_coding(
                fmt,
                &preset.quality,
                preset.quality_fallback,
                preset.workspace_policy,
            )?;
            let slot_name = format!("v_{bi}_{}", fmt.extension());
            let enc_name = format!("enc_{bi}_{}", fmt.extension());
            let io_id = next_io_id;
            next_io_id += 1;

            outputs_map.insert(slot_name.clone(), json!(io_id));
            nodes.insert(
                enc_name.clone(),
                json!({
                    "encode": {
                        "output": slot_name,
                        "format": fmt.wire_format(),
                        "coding": resolved.coding_json,
                        "effort": preset.effort.as_wire_str(),
                        "metadata": preset.metadata.as_wire_str()
                    }
                }),
            );
            edges.push(json!({ "from": upstream_name, "to": enc_name }));

            slots.push(PlannedOutputSlot {
                slot_name,
                io_id,
                breakpoint: *bp,
                format: fmt,
                resolved_coding: resolved,
            });
        }
    }

    let envelope = json!({
        "inputs": { "src": 0 },
        "outputs": Value::Object(outputs_map),
        "limits": {
            "max_memory_bytes": limits.max_memory_bytes,
            "timeout_ms": limits.timeout_ms,
            "max_dimension": limits.max_dimension
        },
        "pipeline": {
            "graph": {
                "nodes": Value::Object(nodes),
                "edges": Value::Array(edges)
            }
        }
    });

    Ok((envelope, slots))
}

/// Build a dedicated video/animation transcode envelope (kept strictly separate from still poster
/// extraction so full sequence timing and codec routes are preserved cleanly).
pub fn build_video_transcode_envelope(
    format: OutputFormat,
    max_width: Option<u32>,
    limits: &BulkLimits,
) -> Result<Value, String> {
    let vcodec = format
        .video_codec()
        .ok_or_else(|| format!("format '{format}' is not a video output format"))?;
    let mut steps = vec![json!({
        "decode": { "input": "src", "select": "all" }
    })];
    if let Some(w) = max_width {
        steps.push(json!({
            "geometry": { "constrain": { "mode": "within", "width": w } }
        }));
    }
    steps.push(json!({
        "encode": {
            "output": "vid",
            "format": format.wire_format(),
            "video_codec": vcodec,
            "tracks": "drop_non_video"
        }
    }));
    Ok(json!({
        "inputs": { "src": 0 },
        "outputs": { "vid": 1 },
        "limits": {
            "max_memory_bytes": limits.max_memory_bytes,
            "timeout_ms": limits.timeout_ms,
            "max_dimension": limits.max_dimension
        },
        "pipeline": { "steps": steps }
    }))
}

/// Contextually escape HTML text and attribute values.
pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Render a standards-compliant responsive HTML `<picture>` element from actual emitted variants.
///
/// Modern formats (`avif`, `jxl`, `webp`) are emitted as `<source type="..." srcset="..." sizes="...">`
/// in efficiency order, with the fallback format (`jpeg` or `png`, else last format) on `<img>`.
pub fn render_picture_snippet(
    asset: &ManifestAsset,
    url_prefix: &str,
    sizes_attr: &str,
    alt_text: &str,
    eager: bool,
) -> String {
    if asset.variants.is_empty() {
        return String::new();
    }

    let mut by_fmt: std::collections::BTreeMap<(u8, OutputFormat), Vec<&ManifestVariant>> =
        std::collections::BTreeMap::new();
    for v in &asset.variants {
        by_fmt.entry((v.format.picture_source_priority(), v.format)).or_default().push(v);
    }
    for group in by_fmt.values_mut() {
        group.sort_by_key(|v| v.width);
    }

    // Choose fallback format for <img>: prefer JPEG, then PNG, then WebP, else last group.
    let fallback_key = by_fmt
        .keys()
        .find(|(_, f)| *f == OutputFormat::Jpeg)
        .or_else(|| by_fmt.keys().find(|(_, f)| *f == OutputFormat::Png))
        .or_else(|| by_fmt.keys().last())
        .copied()
        .expect("non-empty variants");

    let join_url = |rel: &str| -> String {
        if url_prefix.is_empty() {
            rel.to_string()
        } else {
            format!("{}/{}", url_prefix.trim_end_matches('/'), rel.trim_start_matches('/'))
        }
    };

    let format_srcset = |list: &[&ManifestVariant]| -> String {
        list.iter()
            .map(|v| format!("{} {}w", escape_html(&join_url(&v.relative_path)), v.width))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let mut html = String::from("<picture>\n");
    for (&key, group) in &by_fmt {
        if key == fallback_key {
            continue;
        }
        let mime = key.1.mime_type();
        let srcset = format_srcset(group);
        html.push_str(&format!(
            "  <source type=\"{mime}\" srcset=\"{srcset}\" sizes=\"{}\">\n",
            escape_html(sizes_attr)
        ));
    }

    let fallback_group = &by_fmt[&fallback_key];
    let default_v = fallback_group.last().expect("non-empty group");
    let fallback_srcset = format_srcset(fallback_group);
    let loading = if eager { "eager" } else { "lazy" };
    html.push_str(&format!(
        "  <img src=\"{}\" srcset=\"{}\" sizes=\"{}\" width=\"{}\" height=\"{}\" alt=\"{}\" loading=\"{}\" decoding=\"async\">\n",
        escape_html(&join_url(&default_v.relative_path)),
        fallback_srcset,
        escape_html(sizes_attr),
        default_v.width,
        default_v.height,
        escape_html(alt_text),
        loading
    ));
    html.push_str("</picture>");
    html
}
