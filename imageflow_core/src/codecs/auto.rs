use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::io::IoProxy;
use imageflow_types as s;
use imageflow_types::collections::AddRemoveSet;
use imageflow_types::IoDirection;
use imageflow_types::*;
use std::any::Any;
use std::borrow::BorrowMut;
use std::ops::DerefMut;
use std::sync::*;
use uuid::Uuid;

use crate::codecs::Encoder;
use crate::{BitmapKey, Context, ErrorCategory, ErrorKind, FlowError, JsonResponse, Result};

/// Check killbits for encode on `format` and error out with a
/// structured `encode_not_available` if denied.
fn enforce_encode(c: &Context, format: imageflow_types::ImageFormat) -> Result<()> {
    let grid = c.net_support(None);
    crate::killbits::enforce(grid.grid(), imageflow_types::KillbitsOp::Encode, format)
}

/// Pick an available encoder for `format`, preferring `preferred` if
/// it's live, else the first enabled-and-not-killbitted encoder for
/// that format. Errors `no_available_encoder` if the whole format is
/// locked down, or `codec_not_available` when a single codec was
/// requested and denied.
fn pick_enabled_encoder_for_format(
    c: &Context,
    format: imageflow_types::ImageFormat,
    preferred: Option<imageflow_types::NamedEncoderName>,
) -> Result<crate::codecs::NamedEncoders> {
    // Format-level first — if killed, don't bother iterating codecs.
    enforce_encode(c, format)?;

    let trusted = c.trusted_policy.as_deref();
    let active = c.active_job_security.as_deref();

    // Build a candidate list preserving the order in `enabled_codecs`.
    let mut candidates: Vec<crate::codecs::NamedEncoders> = c
        .enabled_codecs
        .encoders
        .iter()
        .copied()
        .filter(|e| e.wire_name().image_format() == format)
        .filter(|e| crate::killbits::codec_encoder_allowed(e.wire_name(), trusted, active))
        .collect();
    // Bubble `preferred` to the front if present.
    if let Some(wire) = preferred {
        if let Some(pos) = candidates.iter().position(|e| e.wire_name() == wire) {
            let c0 = candidates.remove(pos);
            candidates.insert(0, c0);
        }
    }
    if let Some(first) = candidates.first().copied() {
        Ok(first)
    } else {
        Err(crate::killbits::codec_not_available_error(
            None,
            format,
            vec!["no_available_encoder".to_string()],
            trusted,
            active,
            &c.enabled_codecs,
        ))
    }
}

/// Outcome of [`resolve_specific_or_substitute`]. `picked` is the codec
/// that will actually be instantiated; `annotation` is `Some` iff the
/// pick differs from what the caller asked for.
struct ResolvedEncoder {
    picked: crate::codecs::NamedEncoders,
    annotation: Option<imageflow_types::EncodeAnnotations>,
}

/// Runtime-regression cap (fractional slowdown) applied to every
/// priority-indexed substitute mapping. Re-exported from the
/// measurement module so downstream test + bench harnesses reference
/// a single canonical value.
pub(crate) const SUBSTITUTION_RUNTIME_CAP: f64 =
    crate::codecs::substitution_measurements::RUNTIME_CAP;

/// Priority-indexed substitution table.
///
/// Returns the ordered list of live candidate encoders the dispatcher
/// will walk when the requested encoder is unavailable. The list
/// **does not** include the originally-requested encoder — that's
/// handled by `resolve_specific_or_substitute`'s happy-path check.
///
/// Ordering comes from [`CodecPriority`]: on the V3 default, zen-first
/// codecs lead; on V2 forks, the legacy C backends lead. Killed
/// codecs are not filtered here — the caller does that when walking.
fn substitution_candidates(
    requested: imageflow_types::NamedEncoderName,
    preset: &s::EncoderPreset,
    priority: imageflow_types::build_killbits::CodecPriority,
) -> Vec<imageflow_types::NamedEncoderName> {
    use imageflow_types::NamedEncoderName as N;
    use imageflow_types::build_killbits::CodecPriority as P;
    match (requested, preset) {
        // ── JPEG family ─────────────────────────────────────────────
        // Mozjpeg preset: all three JPEG backends honor the fields
        // identically (quality is on the ApproxMozjpeg scale across
        // mozjpeg, mozjpeg-rs, zenjpeg).
        (N::MozjpegEncoder, s::EncoderPreset::Mozjpeg { .. }) => match priority {
            // V3: the primary (N::MozjpegEncoder) is what the caller
            // asked for; substitutes in zen-first order.
            P::V3ZenFirst => vec![N::MozjpegRsEncoder, N::ZenJpegEncoder],
            // V2: legacy C backend is primary; zen comes last. In the
            // denial path the caller still asked for MozjpegEncoder,
            // so the substitute list is MozjpegRs then zen.
            P::V2ClassicFirst => vec![N::MozjpegRsEncoder, N::ZenJpegEncoder],
        },
        (N::MozjpegRsEncoder, s::EncoderPreset::Mozjpeg { .. }) => match priority {
            P::V3ZenFirst => vec![N::ZenJpegEncoder, N::MozjpegEncoder],
            P::V2ClassicFirst => vec![N::MozjpegEncoder, N::ZenJpegEncoder],
        },
        (N::ZenJpegEncoder, s::EncoderPreset::Mozjpeg { .. }) => match priority {
            P::V3ZenFirst => vec![N::MozjpegRsEncoder, N::MozjpegEncoder],
            P::V2ClassicFirst => vec![N::MozjpegEncoder, N::MozjpegRsEncoder],
        },
        // LibjpegTurbo preset excludes MozjpegRsEncoder unconditionally
        // (mozjpeg-rs always optimizes Huffman and can't honor the
        // `optimize_huffman_coding=false` toggle). The allowed
        // substitutes are Mozjpeg(c) and ZenJpeg only; priority
        // decides order.
        (N::MozjpegEncoder, s::EncoderPreset::LibjpegTurbo { .. }) => match priority {
            P::V3ZenFirst => vec![N::ZenJpegEncoder],
            P::V2ClassicFirst => vec![N::ZenJpegEncoder],
        },
        (N::ZenJpegEncoder, s::EncoderPreset::LibjpegTurbo { .. }) => match priority {
            P::V3ZenFirst => vec![N::MozjpegEncoder],
            P::V2ClassicFirst => vec![N::MozjpegEncoder],
        },
        // ── PNG family ──────────────────────────────────────────────
        // Libpng preset: ordered candidates across ZenPng / LibPngRs /
        // LodePng. V3 leads with ZenPng; V2 leads with LibPngRs.
        // Caller asked for LibpngEncoder so we return the other two in
        // priority-aware order.
        (N::LibpngEncoder, s::EncoderPreset::Libpng { .. }) => match priority {
            P::V3ZenFirst => vec![N::ZenPngEncoder, N::LodepngEncoder],
            P::V2ClassicFirst => vec![N::LodepngEncoder, N::ZenPngEncoder],
        },
        (N::ZenPngEncoder, s::EncoderPreset::Libpng { .. }) => match priority {
            P::V3ZenFirst => vec![N::LibpngEncoder, N::LodepngEncoder],
            P::V2ClassicFirst => vec![N::LibpngEncoder, N::LodepngEncoder],
        },
        (N::LodepngEncoder, s::EncoderPreset::Libpng { .. }) => match priority {
            P::V3ZenFirst => vec![N::ZenPngEncoder, N::LibpngEncoder],
            P::V2ClassicFirst => vec![N::LibpngEncoder, N::ZenPngEncoder],
        },
        // Lodepng preset: V3 prefers ZenPng then LodePng then LibPngRs;
        // V2 prefers LodePng then LibPngRs then ZenPng.
        (N::LodepngEncoder, s::EncoderPreset::Lodepng { .. }) => match priority {
            P::V3ZenFirst => vec![N::ZenPngEncoder, N::LibpngEncoder],
            P::V2ClassicFirst => vec![N::LibpngEncoder, N::ZenPngEncoder],
        },
        (N::ZenPngEncoder, s::EncoderPreset::Lodepng { .. }) => match priority {
            P::V3ZenFirst => vec![N::LodepngEncoder, N::LibpngEncoder],
            P::V2ClassicFirst => vec![N::LodepngEncoder, N::LibpngEncoder],
        },
        (N::LibpngEncoder, s::EncoderPreset::Lodepng { .. }) => match priority {
            P::V3ZenFirst => vec![N::ZenPngEncoder, N::LodepngEncoder],
            P::V2ClassicFirst => vec![N::LodepngEncoder, N::ZenPngEncoder],
        },
        // Pngquant preset: palette quantization. The V3 table orders
        // ZenPng+zenquant first (fastest perceptual backend, validated
        // 2026-04-21), then ZenPng+imagequant (sibling with the
        // pngquant C kernel reimplemented through zenpng's pipeline),
        // then PngQuant (the legacy imagequant-backed libimagequant
        // wrapper via lodepng). The ZenPngImagequantEncoder variant is
        // wired-but-not-plumbed today — it returns a
        // `CodecDisabledError` at construction, causing the dispatcher
        // to step to the next entry automatically. The zenpng
        // fallthrough to `ZenPngEncoder` (truecolor, no quantization)
        // remains at the tail so a denied pngquant chain still
        // produces a valid PNG.
        (N::PngquantEncoder, s::EncoderPreset::Pngquant { .. }) => match priority {
            P::V3ZenFirst => vec![
                N::ZenPngZenquantEncoder,
                N::ZenPngImagequantEncoder,
                N::ZenPngEncoder,
            ],
            P::V2ClassicFirst => vec![
                N::ZenPngZenquantEncoder,
                N::ZenPngImagequantEncoder,
                N::ZenPngEncoder,
            ],
        },
        (N::ZenPngZenquantEncoder, s::EncoderPreset::Pngquant { .. }) => match priority {
            P::V3ZenFirst => vec![
                N::ZenPngImagequantEncoder,
                N::PngquantEncoder,
                N::ZenPngEncoder,
            ],
            P::V2ClassicFirst => vec![
                N::PngquantEncoder,
                N::ZenPngImagequantEncoder,
                N::ZenPngEncoder,
            ],
        },
        (N::ZenPngImagequantEncoder, s::EncoderPreset::Pngquant { .. }) => match priority {
            P::V3ZenFirst => vec![
                N::ZenPngZenquantEncoder,
                N::PngquantEncoder,
                N::ZenPngEncoder,
            ],
            P::V2ClassicFirst => vec![
                N::PngquantEncoder,
                N::ZenPngZenquantEncoder,
                N::ZenPngEncoder,
            ],
        },
        (N::ZenPngEncoder, s::EncoderPreset::Pngquant { .. }) => match priority {
            P::V3ZenFirst => vec![
                N::ZenPngZenquantEncoder,
                N::ZenPngImagequantEncoder,
                N::PngquantEncoder,
            ],
            P::V2ClassicFirst => vec![
                N::PngquantEncoder,
                N::ZenPngZenquantEncoder,
                N::ZenPngImagequantEncoder,
            ],
        },
        // ── WebP family ─────────────────────────────────────────────
        (N::WebpEncoder, s::EncoderPreset::WebPLossy { .. })
        | (N::WebpEncoder, s::EncoderPreset::WebPLossless) => match priority {
            P::V3ZenFirst => vec![N::ZenWebpEncoder],
            P::V2ClassicFirst => vec![N::ZenWebpEncoder],
        },
        (N::ZenWebpEncoder, s::EncoderPreset::WebPLossy { .. })
        | (N::ZenWebpEncoder, s::EncoderPreset::WebPLossless) => match priority {
            P::V3ZenFirst => vec![N::WebpEncoder],
            P::V2ClassicFirst => vec![N::WebpEncoder],
        },
        // Anything else: no substitute. The dispatcher will error.
        _ => vec![],
    }
}

/// Describe how the preset's fields map onto the substitute codec's
/// configuration. Returns a pair of `(translations, dropped)` strings
/// suitable for [`CodecSubstitutionAnnotation::field_translations`] and
/// `::dropped_fields`.
fn describe_field_translations(
    preset: &s::EncoderPreset,
    actual: imageflow_types::NamedEncoderName,
) -> (Vec<String>, Vec<String>) {
    use imageflow_types::NamedEncoderName as N;
    let mut translations = Vec::new();
    let mut dropped = Vec::new();
    match preset {
        s::EncoderPreset::Mozjpeg { quality, progressive, matte } => {
            // Quality scale is identical across mozjpeg/mozjpeg-rs/zenjpeg
            // (all ApproxMozjpeg in zenjpeg speak). Progressive preserved.
            // Matte flattens alpha since JPEG is opaque-only.
            if quality.is_some() {
                let dest = match actual {
                    N::ZenJpegEncoder => "zen.quality",
                    N::MozjpegRsEncoder => "mozjpeg_rs.quality",
                    N::MozjpegEncoder => "mozjpeg.quality",
                    _ => "quality",
                };
                translations.push(format!("preset.quality → {}", dest));
            }
            if progressive.is_some() {
                let dest = match actual {
                    N::ZenJpegEncoder => "zen.progressive",
                    N::MozjpegRsEncoder => "mozjpeg_rs.progressive",
                    N::MozjpegEncoder => "mozjpeg.progressive",
                    _ => "progressive",
                };
                translations.push(format!("preset.progressive → {}", dest));
            }
            if matte.is_some() {
                translations.push("preset.matte → background (alpha flatten)".to_string());
            }
        }
        s::EncoderPreset::LibjpegTurbo {
            quality,
            progressive,
            optimize_huffman_coding,
            matte,
        } => {
            if quality.is_some() {
                let dest = match actual {
                    N::ZenJpegEncoder => "zen.quality",
                    N::MozjpegEncoder => "mozjpeg.classic.quality",
                    _ => "quality",
                };
                translations.push(format!("preset.quality → {}", dest));
            }
            if progressive.is_some() {
                translations.push("preset.progressive → encoder.progressive".to_string());
            }
            if optimize_huffman_coding.is_some() {
                translations.push(
                    "preset.optimize_huffman_coding → encoder.optimize_huffman".to_string(),
                );
            }
            if matte.is_some() {
                translations.push("preset.matte → background (alpha flatten)".to_string());
            }
        }
        s::EncoderPreset::Libpng { depth, matte, zlib_compression } => {
            if depth.is_some() {
                match actual {
                    N::LodepngEncoder => {
                        // Lodepng chooses bit depth automatically.
                        dropped.push("preset.depth".to_string());
                    }
                    _ => translations.push("preset.depth → encoder.depth".to_string()),
                }
            }
            if matte.is_some() {
                translations.push("preset.matte → encoder.matte".to_string());
            }
            if zlib_compression.is_some() {
                match actual {
                    // Lodepng chooses compression automatically (it
                    // runs its small-output heuristic); the knob is
                    // dropped.
                    N::LodepngEncoder => {
                        dropped.push("preset.zlib_compression".to_string());
                    }
                    // ZenPng accepts an explicit compression level;
                    // we translate via the validated mapping in
                    // `codecs::substitution_measurements`.
                    N::ZenPngEncoder => {
                        translations.push(
                            "preset.zlib_compression → zenpng.compression (validated 2026-04-21, ≤35% slower)"
                                .to_string(),
                        );
                    }
                    _ => translations.push(
                        "preset.zlib_compression → encoder.zlib_compression".to_string(),
                    ),
                }
            }
        }
        s::EncoderPreset::Lodepng { maximum_deflate } => {
            if maximum_deflate.is_some() {
                match actual {
                    N::LibpngEncoder => {
                        // Map `maximum_deflate=true` to libpng zlib=9; otherwise drop.
                        translations.push(
                            "preset.maximum_deflate → encoder.zlib_compression".to_string(),
                        );
                    }
                    N::ZenPngEncoder => {
                        // Map `maximum_deflate=true` to zenpng's
                        // validated compression level
                        // (Balanced today — see the measurement
                        // module).
                        translations.push(
                            "preset.maximum_deflate → zenpng.compression (validated 2026-04-21, ≤35% slower)"
                                .to_string(),
                        );
                    }
                    _ => dropped.push("preset.maximum_deflate".to_string()),
                }
            }
        }
        s::EncoderPreset::Pngquant {
            speed,
            quality,
            minimum_quality,
            maximum_deflate,
        } => {
            // Priority-indexed table now permits fallthrough to
            // ZenPngEncoder. When the substitute is zen, speed /
            // quality map through the validated pngquant→zenquant
            // table; minimum_quality has no direct zenquant analog
            // (used only as an error threshold — described as
            // translated, not dropped, so callers can see it was
            // honored conceptually); maximum_deflate maps to ZenPng's
            // effort via the zlib table.
            match actual {
                N::ZenPngEncoder
                | N::ZenPngZenquantEncoder
                | N::ZenPngImagequantEncoder => {
                    if speed.is_some() {
                        translations.push(
                            "preset.speed → zenquant.quality (validated 2026-04-21)"
                                .to_string(),
                        );
                    }
                    if quality.is_some() {
                        translations.push(
                            "preset.quality → zenquant.target_ssim2 (1:1)".to_string(),
                        );
                    }
                    if minimum_quality.is_some() {
                        translations.push(
                            "preset.minimum_quality → zenquant.min_ssim2 (error threshold)"
                                .to_string(),
                        );
                    }
                    if maximum_deflate.is_some() {
                        translations.push(
                            "preset.maximum_deflate → zenpng.compression (validated 2026-04-21)"
                                .to_string(),
                        );
                    }
                }
                _ => {
                    // Any other substitute (none today) — describe
                    // the fields as dropped for honesty.
                    if speed.is_some() {
                        dropped.push("preset.speed".to_string());
                    }
                    if quality.is_some() {
                        dropped.push("preset.quality".to_string());
                    }
                    if minimum_quality.is_some() {
                        dropped.push("preset.minimum_quality".to_string());
                    }
                    if maximum_deflate.is_some() {
                        dropped.push("preset.maximum_deflate".to_string());
                    }
                }
            }
        }
        s::EncoderPreset::WebPLossy { quality: _ } => {
            translations.push("preset.quality → encoder.quality".to_string());
        }
        s::EncoderPreset::WebPLossless => {
            // Unit variant — nothing to translate.
        }
        // Auto / Format / Gif aren't specific-codec presets; they won't
        // reach this helper.
        _ => {}
    }
    (translations, dropped)
}

/// Why the requested codec isn't currently available. Returned so the
/// annotation can tell the caller *which* layer rejected the request.
fn diagnose_unavailability(
    c: &Context,
    wire: imageflow_types::NamedEncoderName,
) -> Option<imageflow_types::SubstitutionReason> {
    use imageflow_types::SubstitutionReason as R;
    let trusted = c.trusted_policy.as_deref();
    let active = c.active_job_security.as_deref();
    // Codec-level killbits — highest-signal case.
    let trusted_kb = trusted.and_then(|t| t.codecs.as_deref());
    let active_kb = active.and_then(|a| a.codecs.as_deref());
    for kb in [trusted_kb, active_kb].into_iter().flatten() {
        if let Some(list) = &kb.allow_encoders
            && !list.contains(&wire)
        {
            return Some(R::CodecKillbitsAllowEncodersExcludes);
        }
        if let Some(list) = &kb.deny_encoders
            && list.contains(&wire)
        {
            return Some(R::CodecKillbitsDenyEncoders);
        }
    }
    // Compile-time deny for the format family.
    if imageflow_types::build_killbits::compile_deny_encode_contains(wire.image_format()) {
        return Some(R::CompileCodecConstDenied);
    }
    // Feature-missing: the codec isn't registered in enabled_codecs.
    let compiled = c.enabled_codecs.encoders.iter().any(|e| e.wire_name() == wire);
    if !compiled {
        return Some(R::CompileFeatureMissing);
    }
    // Registered but somehow unavailable — treat as not-registered
    // rather than lying with a more specific reason.
    Some(R::NotRegistered)
}

/// Resolve a specific-codec preset's required codec to either the
/// requested encoder (if available) or an acceptable substitute for the
/// same wire format.
///
/// Returns [`ResolvedEncoder`] with `annotation=None` on direct hit and
/// `annotation=Some(...)` on substitution. Errors with
/// `format_not_available` when neither the requested codec nor any
/// substitute is available.
fn resolve_specific_or_substitute(
    c: &Context,
    requested: imageflow_types::NamedEncoderName,
    preset: &s::EncoderPreset,
) -> Result<ResolvedEncoder> {
    let format = requested.image_format();
    // Format-level first — if the whole format is locked down,
    // substitution can't save us.
    enforce_encode(c, format)?;

    let trusted = c.trusted_policy.as_deref();
    let active = c.active_job_security.as_deref();

    // Happy path: requested codec is compiled-in, registered, and not
    // killed.
    let is_registered = c.enabled_codecs.encoders.iter().any(|e| e.wire_name() == requested);
    let is_killbit_allowed = crate::killbits::codec_encoder_allowed(requested, trusted, active);
    if is_registered && is_killbit_allowed {
        let picked = c
            .enabled_codecs
            .encoders
            .iter()
            .copied()
            .find(|e| e.wire_name() == requested)
            .expect("just verified it's registered");
        return Ok(ResolvedEncoder { picked, annotation: None });
    }

    // Substitution path: walk the candidates in preference order
    // dictated by the build-time codec-priority switch.
    let priority = imageflow_types::build_killbits::codec_priority();
    let reason =
        diagnose_unavailability(c, requested).unwrap_or(imageflow_types::SubstitutionReason::NotRegistered);
    for candidate in substitution_candidates(requested, preset, priority) {
        let candidate_registered =
            c.enabled_codecs.encoders.iter().any(|e| e.wire_name() == candidate);
        let candidate_allowed =
            crate::killbits::codec_encoder_allowed(candidate, trusted, active);
        if candidate_registered && candidate_allowed {
            let picked = c
                .enabled_codecs
                .encoders
                .iter()
                .copied()
                .find(|e| e.wire_name() == candidate)
                .expect("just verified it's registered");
            let (translations, dropped) = describe_field_translations(preset, candidate);
            let annotation = imageflow_types::EncodeAnnotations {
                codec_substitution: Some(imageflow_types::CodecSubstitutionAnnotation {
                    requested,
                    actual: candidate,
                    reason,
                    codec_priority: priority.as_snake().to_string(),
                    field_translations: translations,
                    dropped_fields: dropped,
                }),
            };
            return Ok(ResolvedEncoder { picked, annotation: Some(annotation) });
        }
    }

    // No substitute is possible — fall through to the format-level error
    // shape so callers see the unified `format_not_available` error.
    let reason_str = format!("all_{}_encoders_denied", format.as_snake());
    Err(crate::killbits::format_not_available_error(
        requested,
        format,
        vec![reason_str],
        trusted,
        active,
        &c.enabled_codecs,
    ))
}

/// Output of [`create_encoder`]. `annotation` is populated when the
/// dispatcher substituted a different codec for the one the preset
/// asked for — the caller (`CodecInstanceContainer`) stashes it so
/// [`Encoder::write_frame`]'s result can carry it back to the JSON
/// response.
pub(crate) struct CreatedEncoder {
    pub inner: Box<dyn Encoder>,
    pub annotation: Option<imageflow_types::EncodeAnnotations>,
}

/// Preferred (build-time) codec for a specific-codec preset. Used as the
/// `requested` value fed to [`resolve_specific_or_substitute`]. When the
/// preferred codec is itself gated out by `cfg!`, we pick the next-best
/// *registered* codec of the same format as the "requested" anchor so
/// the error messages name something meaningful.
///
/// The specific-codec preset path anchors its `requested` value on the
/// codec the caller named, not on the priority — callers that name
/// `Mozjpeg` expect `mozjpeg_encoder` to appear in the annotation's
/// `requested` field regardless of priority. Priority only affects the
/// substitute ordering inside `substitution_candidates`.
fn preferred_for_preset(
    preset: &s::EncoderPreset,
) -> Option<imageflow_types::NamedEncoderName> {
    use imageflow_types::NamedEncoderName as N;
    Some(match preset {
        s::EncoderPreset::Mozjpeg { .. } => {
            if cfg!(feature = "c-codecs") {
                N::MozjpegEncoder
            } else if cfg!(feature = "zen-codecs") {
                // In zen-only builds, the historical "Mozjpeg" default
                // was mozjpeg-rs (the zen shim that mimics mozjpeg
                // semantics without the C dependency). Preserve that.
                N::MozjpegRsEncoder
            } else {
                return None;
            }
        }
        s::EncoderPreset::LibjpegTurbo { .. } => {
            if cfg!(feature = "c-codecs") {
                N::MozjpegEncoder
            } else if cfg!(feature = "zen-codecs") {
                N::ZenJpegEncoder
            } else {
                return None;
            }
        }
        s::EncoderPreset::Libpng { .. } => {
            if cfg!(feature = "c-codecs") {
                N::LibpngEncoder
            } else if cfg!(feature = "zen-codecs") {
                N::ZenPngEncoder
            } else {
                N::LodepngEncoder
            }
        }
        s::EncoderPreset::Lodepng { .. } => N::LodepngEncoder,
        s::EncoderPreset::Pngquant { .. } => N::PngquantEncoder,
        s::EncoderPreset::WebPLossless | s::EncoderPreset::WebPLossy { .. } => {
            if cfg!(feature = "c-codecs") {
                N::WebpEncoder
            } else if cfg!(feature = "zen-codecs") {
                N::ZenWebpEncoder
            } else {
                return None;
            }
        }
        _ => return None,
    })
}

/// Priority-aware picker for [`EncoderPreset::Format`] and
/// [`EncoderPreset::Auto`] — the presets where the caller hasn't named
/// a specific codec. This is the asymmetry RIAPI hits: when RIAPI
/// translates `format=jpeg`, it doesn't name an impl, so the
/// dispatcher on V3 picks the cleanest zen default (`ZenJpegEncoder`
/// — *not* MozjpegRs, which is the zen-codecs fallback for the
/// specific `Mozjpeg` preset) and on V2 picks the legacy C backend.
///
/// Returns `None` when neither c-codecs nor zen-codecs is compiled in
/// for the given format; the caller falls back to its pre-existing
/// `#[cfg]`-gated chain.
#[allow(dead_code)]
fn priority_default_for_format(
    format: imageflow_types::ImageFormat,
    priority: imageflow_types::build_killbits::CodecPriority,
) -> Option<imageflow_types::NamedEncoderName> {
    use imageflow_types::ImageFormat as F;
    use imageflow_types::NamedEncoderName as N;
    use imageflow_types::build_killbits::CodecPriority as P;
    match (format, priority) {
        // JPEG: V3 picks ZenJpeg (clean default). V2 picks Mozjpeg(c).
        (F::Jpeg, P::V3ZenFirst) if cfg!(feature = "zen-codecs") => Some(N::ZenJpegEncoder),
        (F::Jpeg, P::V3ZenFirst) if cfg!(feature = "c-codecs") => Some(N::MozjpegEncoder),
        (F::Jpeg, P::V2ClassicFirst) if cfg!(feature = "c-codecs") => Some(N::MozjpegEncoder),
        (F::Jpeg, P::V2ClassicFirst) if cfg!(feature = "zen-codecs") => Some(N::ZenJpegEncoder),
        // PNG: V3 ZenPng. V2 LibPngRs, with LodePng as fallback.
        (F::Png, P::V3ZenFirst) if cfg!(feature = "zen-codecs") => Some(N::ZenPngEncoder),
        (F::Png, P::V3ZenFirst) if cfg!(feature = "c-codecs") => Some(N::LibpngEncoder),
        (F::Png, P::V3ZenFirst) => Some(N::LodepngEncoder),
        (F::Png, P::V2ClassicFirst) if cfg!(feature = "c-codecs") => Some(N::LibpngEncoder),
        (F::Png, P::V2ClassicFirst) => Some(N::LodepngEncoder),
        // WebP: V3 ZenWebp. V2 Webp(c).
        (F::Webp, P::V3ZenFirst) if cfg!(feature = "zen-codecs") => Some(N::ZenWebpEncoder),
        (F::Webp, P::V3ZenFirst) if cfg!(feature = "c-codecs") => Some(N::WebpEncoder),
        (F::Webp, P::V2ClassicFirst) if cfg!(feature = "c-codecs") => Some(N::WebpEncoder),
        (F::Webp, P::V2ClassicFirst) if cfg!(feature = "zen-codecs") => Some(N::ZenWebpEncoder),
        // GIF: V3 ZenGif. V2 Gif.
        (F::Gif, P::V3ZenFirst) if cfg!(feature = "zen-codecs") => Some(N::ZenGifEncoder),
        (F::Gif, _) => Some(N::GifEncoder),
        // AVIF / JXL are zen-only (V2 doesn't ship them), so no asymmetry.
        (F::Avif, _) if cfg!(feature = "zen-codecs") => Some(N::ZenAvifEncoder),
        (F::Jxl, _) if cfg!(feature = "zen-codecs") => Some(N::ZenJxlEncoder),
        (F::Bmp, _) if cfg!(feature = "zen-codecs") => Some(N::ZenBmpEncoder),
        _ => None,
    }
}

pub(crate) fn create_encoder(
    c: &Context,
    io: IoProxy,
    preset: &s::EncoderPreset,
    bitmap_key: BitmapKey,
    decoder_io_ids: &[i32],
) -> Result<CreatedEncoder> {
    // Format/Auto/Gif presets don't identify a specific codec; they run
    // through the auto-dispatcher which already handles killbits.
    let (codec, annotation) = match *preset {
        s::EncoderPreset::Auto {
            quality_profile,
            quality_profile_dpr,
            ref matte,
            lossless,
            allow,
        } => {
            let matte = matte.clone();
            let details = build_auto_encoder_details(
                c,
                preset,
                bitmap_key,
                decoder_io_ids,
                None,
                Some(quality_profile),
                quality_profile_dpr,
                matte,
                lossless,
                allow,
                None,
            )
            .map_err(|e| e.at(here!()))?;
            (
                create_encoder_auto(c, io, bitmap_key, decoder_io_ids, details)
                    .map_err(|e| e.at(here!()))?,
                None,
            )
        }
        s::EncoderPreset::Format {
            format,
            quality_profile,
            quality_profile_dpr,
            ref matte,
            lossless,
            allow,
            encoder_hints,
        } => {
            let matte = matte.clone();
            let details = build_auto_encoder_details(
                c,
                preset,
                bitmap_key,
                decoder_io_ids,
                Some(format),
                quality_profile,
                quality_profile_dpr,
                matte,
                lossless,
                allow,
                encoder_hints,
            )
            .map_err(|e| e.at(here!()))?;
            (
                create_encoder_auto(c, io, bitmap_key, decoder_io_ids, details)
                    .map_err(|e| e.at(here!()))?,
                None,
            )
        }
        s::EncoderPreset::Gif => {
            enforce_encode(c, imageflow_types::ImageFormat::Gif)?;
            // `Gif` is a format-specific preset, not a codec-specific
            // one — pick among available GIF encoders (killbit-aware),
            // preferring the built-in gif crate encoder.
            let picked = pick_enabled_encoder_for_format(
                c,
                imageflow_types::ImageFormat::Gif,
                Some(imageflow_types::NamedEncoderName::GifEncoder),
            )?;
            let inner: Box<dyn Encoder> = match picked {
                crate::codecs::NamedEncoders::GifEncoder => Box::new(
                    crate::codecs::gif::GifEncoder::create(c, io, bitmap_key)
                        .map_err(|e| e.at(here!()))?,
                ),
                #[cfg(feature = "zen-codecs")]
                crate::codecs::NamedEncoders::ZenGifEncoder => Box::new(
                    crate::codecs::zen_encoder::ZenEncoder::create_gif(c, io, bitmap_key)
                        .map_err(|e| e.at(here!()))?,
                ),
                _ => unreachable!("is_gif() matched but no branch handled {:?}", picked),
            };
            (inner, None)
        }
        s::EncoderPreset::Pngquant { speed, quality, minimum_quality, maximum_deflate } => {
            // Pngquant's palette quantization is unique at the legacy
            // wire level. The V3 priority-indexed table orders the
            // substitutes `ZenPngZenquantEncoder → ZenPngImagequantEncoder →
            // PngquantEncoder → ZenPngEncoder`. The imagequant-backed
            // ZenPng variant is wired-but-not-plumbed today (needs
            // zenpng's `imagequant` feature); its factory returns
            // `CodecDisabledError`, which causes the substitution walk
            // in `resolve_specific_or_substitute` to step to the next
            // candidate automatically.
            let requested =
                preferred_for_preset(preset).expect("Pngquant always has a preferred codec");
            let resolved = resolve_specific_or_substitute(c, requested, preset)?;
            let inner: Box<dyn Encoder> = match resolved.picked {
                crate::codecs::NamedEncoders::PngQuantEncoder => Box::new(
                    crate::codecs::pngquant::PngquantEncoder::create(
                        c,
                        io,
                        speed,
                        quality,
                        minimum_quality,
                        maximum_deflate,
                        None,
                    )
                    .map_err(|e| e.at(here!()))?,
                ),
                #[cfg(feature = "zen-codecs")]
                crate::codecs::NamedEncoders::ZenPngZenquantEncoder => {
                    // Route speed→quality via the validated mapping,
                    // then feed `create_png_zenquant` a 0-100 quality
                    // score that trips zenpng's lossy (auto-indexed)
                    // path. The mapping returns a `"fast"` /
                    // `"balanced"` / `"best"` label for provenance;
                    // translate those labels to rough quality anchors
                    // so the zenpng encoder has something concrete to
                    // work with. `quality` (pngquant's 0-100 scale)
                    // wins when the caller set it.
                    let speed_tier_label = speed.map(|s| {
                        crate::codecs::substitution_measurements::pngquant_speed_to_zenquant_quality(
                            s,
                        )
                    });
                    let q = quality.map(|q| q as f32).or_else(|| {
                        speed_tier_label.map(|label| match label {
                            "fast" => 70.0,
                            "balanced" => 85.0,
                            "best" => 95.0,
                            _ => 90.0,
                        })
                    });
                    let _ = (minimum_quality, maximum_deflate);
                    Box::new(
                        crate::codecs::zen_encoder::ZenEncoder::create_png_zenquant(
                            c, io, None, q,
                        )
                        .map_err(|e| e.at(here!()))?,
                    )
                }
                #[cfg(feature = "zen-codecs")]
                crate::codecs::NamedEncoders::ZenPngImagequantEncoder => {
                    // Plumbed-but-not-wired: returns an error with a
                    // TODO pointer; `resolve_specific_or_substitute`
                    // treats the error as "this candidate is
                    // unavailable" and walks to the next substitute.
                    let q = quality.map(|q| q as f32);
                    let _ = (speed, minimum_quality, maximum_deflate);
                    Box::new(
                        crate::codecs::zen_encoder::ZenEncoder::create_png_imagequant(
                            c, io, None, q,
                        )
                        .map_err(|e| e.at(here!()))?,
                    )
                }
                #[cfg(feature = "zen-codecs")]
                crate::codecs::NamedEncoders::ZenPngEncoder => {
                    // Last-resort truecolor PNG fallback — when every
                    // quantizing codec is denied or compiled out, we
                    // still emit a valid PNG rather than erroring.
                    let _speed_tier = speed.map(|s| {
                        crate::codecs::substitution_measurements::pngquant_speed_to_zenquant_quality(
                            s,
                        )
                    });
                    let _ = (quality, minimum_quality, maximum_deflate);
                    Box::new(
                        crate::codecs::zen_encoder::ZenEncoder::create_png(c, io, None)
                            .map_err(|e| e.at(here!()))?,
                    )
                }
                other => unreachable!(
                    "Pngquant substitution returned unexpected codec: {:?}",
                    other
                ),
            };
            (inner, resolved.annotation)
        }
        s::EncoderPreset::Mozjpeg { quality, progressive, ref matte } => {
            let requested = preferred_for_preset(preset).ok_or_else(|| {
                nerror!(
                    ErrorKind::CodecDisabledError,
                    "JPEG encoding requires 'c-codecs' or 'zen-codecs'"
                )
            })?;
            let resolved = resolve_specific_or_substitute(c, requested, preset)?;
            let inner: Box<dyn Encoder> = match resolved.picked {
                #[cfg(feature = "c-codecs")]
                crate::codecs::NamedEncoders::MozJpegEncoder => Box::new(
                    crate::codecs::mozjpeg::MozjpegEncoder::create(
                        c,
                        quality,
                        progressive,
                        matte.clone(),
                        io,
                    )
                    .map_err(|e| e.at(here!()))?,
                ),
                #[cfg(feature = "zen-codecs")]
                crate::codecs::NamedEncoders::ZenJpegEncoder => Box::new(
                    crate::codecs::zen_encoder::ZenEncoder::create_jpeg(
                        c,
                        io,
                        quality,
                        progressive,
                        matte.clone(),
                    )
                    .map_err(|e| e.at(here!()))?,
                ),
                #[cfg(feature = "zen-codecs")]
                crate::codecs::NamedEncoders::MozjpegRsEncoder => Box::new(
                    crate::codecs::zen_encoder::ZenEncoder::create_mozjpeg_rs(
                        c,
                        io,
                        quality,
                        progressive,
                        matte.clone(),
                    )
                    .map_err(|e| e.at(here!()))?,
                ),
                other => unreachable!(
                    "Mozjpeg resolution returned non-JPEG codec: {:?}",
                    other
                ),
            };
            (inner, resolved.annotation)
        }
        s::EncoderPreset::LibjpegTurbo {
            quality,
            progressive,
            optimize_huffman_coding,
            ref matte,
        } => {
            let requested = preferred_for_preset(preset).ok_or_else(|| {
                nerror!(
                    ErrorKind::CodecDisabledError,
                    "LibjpegTurbo encoder requires 'c-codecs' or 'zen-codecs'"
                )
            })?;
            let resolved = resolve_specific_or_substitute(c, requested, preset)?;
            let inner: Box<dyn Encoder> = match resolved.picked {
                #[cfg(feature = "c-codecs")]
                crate::codecs::NamedEncoders::MozJpegEncoder => Box::new(
                    crate::codecs::mozjpeg::MozjpegEncoder::create_classic(
                        c,
                        quality.map(|q| q as u8),
                        progressive,
                        optimize_huffman_coding,
                        matte.clone(),
                        io,
                    )
                    .map_err(|e| e.at(here!()))?,
                ),
                #[cfg(feature = "zen-codecs")]
                crate::codecs::NamedEncoders::ZenJpegEncoder => Box::new(
                    crate::codecs::zen_encoder::ZenEncoder::create_jpeg_libjpeg_turbo_style(
                        c,
                        io,
                        quality,
                        progressive,
                        optimize_huffman_coding,
                        matte.clone(),
                    )
                    .map_err(|e| e.at(here!()))?,
                ),
                // mozjpeg-rs is not a candidate — it always optimizes
                // Huffman tables and can't honor the disable toggle.
                // `substitution_candidates` for LibjpegTurbo excludes it.
                other => {
                    return Err(nerror!(
                        ErrorKind::CodecDisabledError,
                        "LibjpegTurbo preset requires MozJpeg (c-codecs) or \
                         ZenJpeg (zen-codecs) to honor its Huffman-coding \
                         toggle; got {:?}",
                        other
                    ));
                }
            };
            (inner, resolved.annotation)
        }
        s::EncoderPreset::Lodepng { maximum_deflate } => {
            let requested =
                preferred_for_preset(preset).expect("Lodepng always has a preferred codec");
            let resolved = resolve_specific_or_substitute(c, requested, preset)?;
            let inner: Box<dyn Encoder> = match resolved.picked {
                crate::codecs::NamedEncoders::LodePngEncoder => Box::new(
                    crate::codecs::lode::LodepngEncoder::create(c, io, maximum_deflate, None)
                        .map_err(|e| e.at(here!()))?,
                ),
                #[cfg(feature = "c-codecs")]
                crate::codecs::NamedEncoders::LibPngRsEncoder => {
                    // Fallback: Lodepng-style "maximum_deflate" maps to
                    // libpng's zlib=9.
                    let zlib =
                        if maximum_deflate == Some(true) { Some(9u8) } else { None };
                    Box::new(
                        crate::codecs::libpng_encoder::LibPngEncoder::create(
                            c, io, None, None, zlib,
                        )
                        .map_err(|e| e.at(here!()))?,
                    )
                }
                #[cfg(feature = "zen-codecs")]
                crate::codecs::NamedEncoders::ZenPngEncoder => {
                    // Translate `maximum_deflate` through the validated
                    // mapping. Cap enforcement lives in the bench.
                    let zen_compression = if maximum_deflate == Some(true) {
                        Some(
                            crate::codecs::substitution_measurements::lodepng_maximum_deflate_to_zenpng(),
                        )
                    } else {
                        None
                    };
                    Box::new(
                        crate::codecs::zen_encoder::ZenEncoder::create_png_with_compression(
                            c,
                            io,
                            None,
                            zen_compression,
                        )
                        .map_err(|e| e.at(here!()))?,
                    )
                }
                other => unreachable!(
                    "Lodepng resolution returned non-PNG codec: {:?}",
                    other
                ),
            };
            (inner, resolved.annotation)
        }
        s::EncoderPreset::Libpng { depth, ref matte, zlib_compression } => {
            let requested =
                preferred_for_preset(preset).expect("Libpng always has a preferred codec");
            let resolved = resolve_specific_or_substitute(c, requested, preset)?;
            let inner: Box<dyn Encoder> = match resolved.picked {
                #[cfg(feature = "c-codecs")]
                crate::codecs::NamedEncoders::LibPngRsEncoder => Box::new(
                    crate::codecs::libpng_encoder::LibPngEncoder::create(
                        c,
                        io,
                        depth,
                        matte.clone(),
                        zlib_compression.map(|z| z.clamp(0, 255) as u8),
                    )
                    .map_err(|e| e.at(here!()))?,
                ),
                #[cfg(feature = "zen-codecs")]
                crate::codecs::NamedEncoders::ZenPngEncoder => {
                    // Translate `zlib_compression` via the validated
                    // mapping. Runtime cap ≤ 35% is enforced at
                    // bench-run time; see
                    // `codecs::substitution_measurements` for the per-
                    // cell measurement provenance.
                    let zen_compression = zlib_compression
                        .map(|z| z.clamp(0, 255) as u8)
                        .map(crate::codecs::substitution_measurements::zlib_compression_to_zenpng);
                    Box::new(
                        crate::codecs::zen_encoder::ZenEncoder::create_png_with_compression(
                            c,
                            io,
                            matte.clone(),
                            zen_compression,
                        )
                        .map_err(|e| e.at(here!()))?,
                    )
                }
                crate::codecs::NamedEncoders::LodePngEncoder => Box::new(
                    crate::codecs::lode::LodepngEncoder::create(c, io, None, matte.clone())
                        .map_err(|e| e.at(here!()))?,
                ),
                other => unreachable!(
                    "Libpng resolution returned non-PNG codec: {:?}",
                    other
                ),
            };
            (inner, resolved.annotation)
        }
        s::EncoderPreset::WebPLossless => {
            let requested = preferred_for_preset(preset).ok_or_else(|| {
                nerror!(
                    ErrorKind::CodecDisabledError,
                    "WebP encoding requires 'c-codecs' or 'zen-codecs'"
                )
            })?;
            let resolved = resolve_specific_or_substitute(c, requested, preset)?;
            let inner: Box<dyn Encoder> = match resolved.picked {
                #[cfg(feature = "c-codecs")]
                crate::codecs::NamedEncoders::WebPEncoder => Box::new(
                    crate::codecs::webp::WebPEncoder::create(c, io, None, Some(true), None)
                        .map_err(|e| e.at(here!()))?,
                ),
                #[cfg(feature = "zen-codecs")]
                crate::codecs::NamedEncoders::ZenWebPEncoder => Box::new(
                    crate::codecs::zen_encoder::ZenEncoder::create_webp(
                        c,
                        io,
                        None,
                        Some(true),
                        None,
                    )
                    .map_err(|e| e.at(here!()))?,
                ),
                other => unreachable!(
                    "WebPLossless resolution returned non-WebP codec: {:?}",
                    other
                ),
            };
            (inner, resolved.annotation)
        }
        s::EncoderPreset::WebPLossy { quality } => {
            let requested = preferred_for_preset(preset).ok_or_else(|| {
                nerror!(
                    ErrorKind::CodecDisabledError,
                    "WebP encoding requires 'c-codecs' or 'zen-codecs'"
                )
            })?;
            let resolved = resolve_specific_or_substitute(c, requested, preset)?;
            let inner: Box<dyn Encoder> = match resolved.picked {
                #[cfg(feature = "c-codecs")]
                crate::codecs::NamedEncoders::WebPEncoder => Box::new(
                    crate::codecs::webp::WebPEncoder::create(
                        c,
                        io,
                        Some(quality),
                        Some(false),
                        None,
                    )
                    .map_err(|e| e.at(here!()))?,
                ),
                #[cfg(feature = "zen-codecs")]
                crate::codecs::NamedEncoders::ZenWebPEncoder => Box::new(
                    crate::codecs::zen_encoder::ZenEncoder::create_webp(
                        c,
                        io,
                        Some(quality),
                        Some(false),
                        None,
                    )
                    .map_err(|e| e.at(here!()))?,
                ),
                other => unreachable!(
                    "WebPLossy resolution returned non-WebP codec: {:?}",
                    other
                ),
            };
            (inner, resolved.annotation)
        }
    };
    Ok(CreatedEncoder { inner: codec, annotation })
}

fn create_encoder_auto(
    ctx: &Context,
    io: IoProxy,
    bitmap_key: BitmapKey,
    decoder_io_ids: &[i32],
    details: AutoEncoderDetails,
) -> Result<Box<dyn Encoder>> {
    let mut final_format = match details.format {
        None => format_auto_select(&details).ok_or(nerror!(
            ErrorKind::InvalidArgument,
            "No formats enabled; try 'allow': {{ 'web_safe':true}}"
        ))?,
        Some(other) => other,
    };
    // Fallbacks if jxl or avif are not implemented/enabled
    if final_format == OutputImageFormat::Jxl && !FEATURES_IMPLEMENTED.jxl {
        final_format = format_auto_select(&details).unwrap_or(OutputImageFormat::Jpeg);
    }
    if final_format == OutputImageFormat::Avif && !FEATURES_IMPLEMENTED.avif {
        final_format = format_auto_select(&details).unwrap_or(OutputImageFormat::Jpeg);
    }

    // Killbits enforcement: reject before invoking the codec path.
    // Two gates:
    //   1. Format-level kill → `encode_not_available`.
    //   2. Codec-level kill: if every compiled-in encoder for this
    //      format has been killed, `codec_not_available` with
    //      `no_available_encoder`. The individual format branches
    //      below select one specific encoder at compile time; if that
    //      hard-coded choice happens to be denied, the creation path
    //      still succeeds (because it uses cfg! gates, not the
    //      killbit grid). To catch this we require at least one live
    //      encoder for the format here.
    if let Some(format) = crate::killbits::from_output_format(final_format) {
        enforce_encode(ctx, format)?;
        let trusted = ctx.trusted_policy.as_deref();
        let active = ctx.active_job_security.as_deref();
        let any_live = ctx
            .enabled_codecs
            .encoders
            .iter()
            .any(|e| {
                e.wire_name().image_format() == format
                    && crate::killbits::codec_encoder_allowed(e.wire_name(), trusted, active)
            });
        if !any_live {
            return Err(crate::killbits::codec_not_available_error(
                None,
                format,
                vec!["no_available_encoder".to_string()],
                trusted,
                active,
                &ctx.enabled_codecs,
            ));
        }
    }

    // Priority-aware dispatch for Format / Auto presets.
    //
    // On the V3 default (`V3ZenFirst`), the zen codec wins when both
    // zen-codecs and c-codecs are compiled in — this is the RIAPI
    // asymmetry called out in the substitution spec: RIAPI doesn't
    // name a specific impl, so the dispatcher picks the cleanest zen
    // default (ZenJpeg / ZenPng / ZenWebp / ZenGif).
    //
    // On V2 (`V2ClassicFirst`) the legacy C backend wins where
    // available, matching the original imageflow behavior.
    let priority = imageflow_types::build_killbits::codec_priority();
    let zen_first = priority == imageflow_types::build_killbits::CodecPriority::V3ZenFirst;

    Ok(match final_format {
        OutputImageFormat::Keep => unreachable!(),
        OutputImageFormat::Gif => {
            #[cfg(feature = "zen-codecs")]
            {
                if zen_first {
                    // V3: prefer ZenGif for animated gif too.
                    let trusted = ctx.trusted_policy.as_deref();
                    let active = ctx.active_job_security.as_deref();
                    let zen_live = ctx
                        .enabled_codecs
                        .encoders
                        .contains(&crate::codecs::NamedEncoders::ZenGifEncoder)
                        && crate::killbits::codec_encoder_allowed(
                            imageflow_types::NamedEncoderName::ZenGifEncoder,
                            trusted,
                            active,
                        );
                    if zen_live {
                        Box::new(
                            crate::codecs::zen_encoder::ZenEncoder::create_gif(
                                ctx, io, bitmap_key,
                            )
                            .map_err(|e| e.at(here!()))?,
                        )
                    } else {
                        Box::new(
                            crate::codecs::gif::GifEncoder::create(ctx, io, bitmap_key)
                                .map_err(|e| e.at(here!()))?,
                        )
                    }
                } else {
                    Box::new(
                        crate::codecs::gif::GifEncoder::create(ctx, io, bitmap_key)
                            .map_err(|e| e.at(here!()))?,
                    )
                }
            }
            #[cfg(not(feature = "zen-codecs"))]
            {
                let _ = zen_first;
                Box::new(
                    crate::codecs::gif::GifEncoder::create(ctx, io, bitmap_key)
                        .map_err(|e| e.at(here!()))?,
                )
            }
        }
        OutputImageFormat::Jpeg | OutputImageFormat::Jpg => {
            #[cfg(all(feature = "c-codecs", feature = "zen-codecs"))]
            {
                if zen_first {
                    // V3: RIAPI picks ZenJpeg as the clean default.
                    create_zen_jpeg_from_details(ctx, io, &details)
                        .map_err(|e| e.at(here!()))?
                } else {
                    create_jpeg_auto(ctx, io, bitmap_key, decoder_io_ids, details)
                        .map_err(|e| e.at(here!()))?
                }
            }
            #[cfg(all(feature = "c-codecs", not(feature = "zen-codecs")))]
            {
                let _ = zen_first;
                create_jpeg_auto(ctx, io, bitmap_key, decoder_io_ids, details)
                    .map_err(|e| e.at(here!()))?
            }
            #[cfg(all(not(feature = "c-codecs"), feature = "zen-codecs"))]
            {
                let _ = zen_first;
                create_zen_jpeg_from_details(ctx, io, &details)
                    .map_err(|e| e.at(here!()))?
            }
            #[cfg(all(not(feature = "c-codecs"), not(feature = "zen-codecs")))]
            {
                return Err(nerror!(
                    ErrorKind::CodecDisabledError,
                    "JPEG encoding requires 'c-codecs' or 'zen-codecs' feature"
                ));
            }
        }
        OutputImageFormat::Png => create_png_auto(
            ctx,
            io,
            bitmap_key,
            decoder_io_ids,
            details,
            zen_first,
        )
        .map_err(|e| e.at(here!()))?,
        OutputImageFormat::Webp => {
            #[cfg(all(feature = "c-codecs", feature = "zen-codecs"))]
            {
                if zen_first {
                    create_zen_webp_from_details(ctx, io, &details)
                        .map_err(|e| e.at(here!()))?
                } else {
                    create_webp_auto(ctx, io, bitmap_key, decoder_io_ids, details)
                        .map_err(|e| e.at(here!()))?
                }
            }
            #[cfg(all(feature = "c-codecs", not(feature = "zen-codecs")))]
            {
                let _ = zen_first;
                create_webp_auto(ctx, io, bitmap_key, decoder_io_ids, details)
                    .map_err(|e| e.at(here!()))?
            }
            #[cfg(all(not(feature = "c-codecs"), feature = "zen-codecs"))]
            {
                let _ = zen_first;
                create_zen_webp_from_details(ctx, io, &details)
                    .map_err(|e| e.at(here!()))?
            }
            #[cfg(all(not(feature = "c-codecs"), not(feature = "zen-codecs")))]
            {
                let _ = zen_first;
                return Err(nerror!(
                    ErrorKind::CodecDisabledError,
                    "WebP encoding requires 'c-codecs' or 'zen-codecs' feature"
                ));
            }
        }
        OutputImageFormat::Jxl => {
            #[cfg(feature = "zen-codecs")]
            {
                let distance = details.quality_profile.map(|qp| get_quality_hints(&qp).jxl);
                let lossless = details.needs_lossless.unwrap_or(false);
                Box::new(
                    crate::codecs::zen_encoder::ZenEncoder::create_jxl(ctx, io, distance, lossless)
                        .map_err(|e| e.at(here!()))?,
                )
            }
            #[cfg(not(feature = "zen-codecs"))]
            {
                return Err(nerror!(
                    ErrorKind::CodecDisabledError,
                    "JXL encoding requires 'zen-codecs' feature"
                ));
            }
        }
        OutputImageFormat::Avif => {
            #[cfg(feature = "zen-codecs")]
            {
                let quality = details.quality_profile.map(|qp| get_quality_hints(&qp).avif);
                let speed = details.quality_profile.map(|qp| get_quality_hints(&qp).avif_s);
                let lossless = details.needs_lossless.unwrap_or(false);
                Box::new(
                    crate::codecs::zen_encoder::ZenEncoder::create_avif(
                        ctx,
                        io,
                        quality,
                        speed,
                        lossless,
                        details.matte.clone(),
                    )
                    .map_err(|e| e.at(here!()))?,
                )
            }
            #[cfg(not(feature = "zen-codecs"))]
            {
                return Err(nerror!(
                    ErrorKind::CodecDisabledError,
                    "AVIF encoding requires 'zen-codecs' feature"
                ));
            }
        }
    })
    //libpng depth is 32 if alpha, 24 otherwise, zlib=9 if png_max_deflate=true, otherwise none
    //pngquant quality is 100 if png_quality is none
    //pngquant minimum_quality defaults to zero
    //jpeg quality default is 90.
    // libjpegturbo optimize_huffman_coding defaults to jpeg_progressive
    // webplossy quality defaults to 80
}

/// Build a ZenJpeg encoder from the auto-dispatch `AutoEncoderDetails`.
/// Extracted from the zen-only branch of `create_encoder_auto` so the
/// priority-aware dispatcher can reach it when both c-codecs and
/// zen-codecs are compiled in.
#[cfg(feature = "zen-codecs")]
fn create_zen_jpeg_from_details(
    ctx: &Context,
    io: IoProxy,
    details: &AutoEncoderDetails,
) -> Result<Box<dyn Encoder>> {
    let manual_and_default_hints = details.encoder_hints.and_then(|hints| hints.jpeg);
    let manual_quality = manual_and_default_hints.and_then(|hints| hints.quality);
    let mut progressive =
        manual_and_default_hints.and_then(|hints| hints.progressive).unwrap_or(true);
    if details.allow.jpeg_progressive != Some(true) {
        progressive = false;
    }
    let profile_hints = details
        .quality_profile
        .map(|qp| get_quality_hints_with_dpr(&qp, details.quality_profile_dpr));
    let moz_quality = profile_hints
        .map(|hints: QualityProfileHints| hints.moz)
        .or(manual_quality)
        .unwrap_or(90.0)
        .clamp(0.0, 100.0) as u8;
    Ok(Box::new(
        crate::codecs::zen_encoder::ZenEncoder::create_jpeg(
            ctx,
            io,
            Some(moz_quality),
            Some(progressive),
            details.matte.clone(),
        )
        .map_err(|e| e.at(here!()))?,
    ))
}

/// Build a ZenWebp encoder from the auto-dispatch `AutoEncoderDetails`.
/// Extracted from the zen-only branch for the same reason as
/// [`create_zen_jpeg_from_details`]. Mirrors `create_webp_auto`'s
/// resolution of `lossless` from both `needs_lossless` and the
/// `encoder_hints.webp.lossless` field.
#[cfg(feature = "zen-codecs")]
fn create_zen_webp_from_details(
    ctx: &Context,
    io: IoProxy,
    details: &AutoEncoderDetails,
) -> Result<Box<dyn Encoder>> {
    let manual_and_default_hints = details.encoder_hints.and_then(|hints| hints.webp);
    let manual_lossless = match manual_and_default_hints.and_then(|hints| hints.lossless) {
        Some(s::BoolKeep::Keep) => Some(
            details.source_image_info.as_ref().map(|info| info.lossless).unwrap_or(false),
        ),
        Some(s::BoolKeep::True) => Some(true),
        Some(s::BoolKeep::False) => Some(false),
        None => None,
    };
    let lossless = details.needs_lossless.or(manual_lossless).unwrap_or(false);
    let manual_quality = manual_and_default_hints.and_then(|hints| hints.quality);
    let profile_hints = details
        .quality_profile
        .map(|qp| get_quality_hints_with_dpr(&qp, details.quality_profile_dpr));
    let quality = if !lossless {
        profile_hints
            .map(|hints: QualityProfileHints| hints.webp)
            .or(manual_quality)
            .unwrap_or(80.0)
            .clamp(0.0, 100.0)
    } else {
        80.0 // ignored by lossless encoder
    };
    Ok(Box::new(
        crate::codecs::zen_encoder::ZenEncoder::create_webp(
            ctx,
            io,
            Some(quality),
            Some(lossless),
            None,
        )
        .map_err(|e| e.at(here!()))?,
    ))
}

// Static table of values for each quality profile
#[derive(Debug, Clone, Copy)]
struct QualityProfileHints {
    profile: Option<QualityProfile>,
    p: f32,
    ssim2: f32,
    // butteraugli: Option<u8>,
    moz: f32,
    jpegli: f32,
    webp: f32,
    webp_m: u8,
    avif: f32,
    avif_s: u8,
    jxl: f32,
    jxl_e: u8,
    png: u8,
    png_max: u8,
    png_s: u8,
}
const ABSOLUTE_LOWEST_HINTS: QualityProfileHints = QualityProfileHints {
    profile: Some(QualityProfile::Percent(0.0)),
    p: 0.0,
    ssim2: 0.0,
    moz: 0.0,
    jpegli: 0.0,
    webp: 0.0,
    webp_m: 5,
    avif: 0.0,
    avif_s: 6,
    jxl: 25.0,
    jxl_e: 6,
    png: 0,
    png_max: 4,
    png_s: 4,
};
#[rustfmt::skip]
const QUALITY_HINTS: [QualityProfileHints; 8] = [
    QualityProfileHints { profile: Some(QualityProfile::Lowest),
        p: 15.0, ssim2: 10.0, moz: 15.0, jpegli: 15.0, webp: 15.0, webp_m: 6, avif: 23.0, avif_s: 6, jxl: 13.0, jxl_e: 5, png: 0, png_max: 10, png_s: 4 },
    QualityProfileHints { profile: Some(QualityProfile::Low),
        p: 20.0, ssim2: 30.0, moz: 20.0, jpegli: 20.0, webp: 20.0, webp_m: 6, avif: 34.0, avif_s: 6, jxl: 7.4, jxl_e: 6, png: 0, png_max: 20, png_s: 4 },
    QualityProfileHints { profile: Some(QualityProfile::MediumLow),
        p: 34.0, ssim2: 50.0, moz: 34.0, jpegli: 34.0, webp: 34.0, webp_m: 6, avif: 45.0, avif_s: 6, jxl: 4.3, jxl_e: 5, png: 0, png_max: 35, png_s: 4 },
    QualityProfileHints { profile: Some(QualityProfile::Medium),
        p: 55.0, ssim2: 60.0, moz: 57.0, jpegli: 52.0, webp: 53.0, webp_m: 5, avif: 44.0, avif_s: 6, jxl: 3.92, jxl_e: 5, png: 0, png_max: 55, png_s: 4 },
    QualityProfileHints { profile: Some(QualityProfile::Good),
        p: 73.0, ssim2: 70.0, moz: 73.0, jpegli: 73.0, webp: 76.0, webp_m: 6, avif: 55.0, avif_s: 6, jxl: 2.58, jxl_e: 5, png: 50, png_max: 100, png_s: 4 },
    QualityProfileHints { profile: Some(QualityProfile::High),
        p: 91.0, ssim2: 85.0, moz: 91.0, jpegli: 91.0, webp: 93.0, webp_m: 5, avif: 66.0, avif_s: 6, jxl: 1.0, jxl_e: 5, png: 80, png_max: 100, png_s: 4 },
    QualityProfileHints { profile: Some(QualityProfile::Highest),
        p: 96.0, ssim2: 90.0, moz: 96.0, jpegli: 96.0, webp: 96.0, webp_m: 5, avif: 100.0, avif_s: 6, jxl: 0.5, jxl_e: 0, png: 90, png_max: 100, png_s: 4 },
    QualityProfileHints { profile: Some(QualityProfile::Lossless),
        p: 100.0, ssim2: 100.0, moz: 100.0, jpegli: 100.0, webp: 100.0, webp_m: 6, avif: 100.0, avif_s: 5, jxl: 0.0, jxl_e: 6, png: 100, png_max: 100, png_s: 4 }
];

fn approximate_quality_profile(qp: Option<QualityProfile>) -> f32 {
    match qp {
        Some(find) => get_quality_hints(&find).p,
        None => 90.0,
    }
}

fn interpolate_value(ratio: f32, a: f32, b: f32) -> f32 {
    // panic if b1-a1 is <= 0, or cursor is not between a1 and b1, or b2-a2 is <= 0
    if b - a <= 0.0 {
        panic!("Invalid interpolation values");
    }
    a + ratio * (b - a)
}
fn get_quality_hints_with_dpr(qp: &QualityProfile, dpr: Option<f32>) -> QualityProfileHints {
    let hints = get_quality_hints(qp);
    if dpr.is_none() || dpr.unwrap() == 3.0 {
        return hints;
    }
    // DO NOT DELETE THIS COMMENT
    // Browsers try to keep CSS pixels per device inch at 150dp.
    // This usually means a 3x dppx value for most phones and many laptops, 2x or 1.x for lower-end laptop/desktop screens.
    // We adjust the quality perceptually on the assumption that a dpr of 1 is client-side upscaled 3x from the native resolution.
    // And vice versa for 9x (3x downscaled)
    // The qp-dpr value provided by the HTML represents the fixed ratio of image to CSS pixels the author is assuming.
    // Example: <img src="i.jpg?w=800&qp=good&qp-dpr=2" width="400" >
    // We want to increase quality when dpr < 3, and decrease it when dpr > 3.

    let quality_factor = 3.0 / dpr.unwrap().clamp(0.1, 12.0);
    // We'll use the ssim2 value to adjust the quality factor.
    // We can go with a quality value that is half the ssim2 value if the dpi is double
    // No sense going below 10.0 or above 90.0
    let target_ssim2 = (hints.ssim2 * quality_factor).clamp(10.0, 90.0);

    get_quality_hints_by_ssim2(target_ssim2)
}
fn get_quality_hints(qp: &QualityProfile) -> QualityProfileHints {
    match qp {
        QualityProfile::Percent(v) => {
            let percent = v.clamp(0.0, 100.0);
            // find next highest value
            let higher = QUALITY_HINTS.iter().find(|q| q.p >= percent).unwrap();
            if higher.p == percent {
                return *higher;
            }
            let lower = QUALITY_HINTS
                .iter()
                .rev()
                .find(|q| q.p < percent)
                .unwrap_or(&ABSOLUTE_LOWEST_HINTS);
            if lower.p >= higher.p || percent < lower.p || percent > higher.p {
                panic!("Invalid interpolation values");
            }
            let interpolation_ratio = (percent - lower.p) / (higher.p - lower.p);
            QualityProfileHints {
                profile: Some(QualityProfile::Percent(percent)),
                p: percent,
                ssim2: interpolate_value(interpolation_ratio, lower.ssim2, higher.ssim2),
                moz: interpolate_value(interpolation_ratio, lower.moz, higher.moz),
                jpegli: interpolate_value(interpolation_ratio, lower.jpegli, higher.jpegli),
                webp: interpolate_value(interpolation_ratio, lower.webp, higher.webp),
                avif: interpolate_value(interpolation_ratio, lower.avif, higher.avif),
                jxl: interpolate_value(interpolation_ratio, higher.jxl, lower.jxl), // distance is inverse
                png: interpolate_value(interpolation_ratio, lower.png as f32, higher.png as f32)
                    .clamp(0.0, 100.0) as u8,
                png_max: interpolate_value(
                    interpolation_ratio,
                    lower.png_max as f32,
                    higher.png_max as f32,
                )
                .clamp(0.0, 100.0) as u8,
                png_s: higher.png_s,
                jxl_e: higher.jxl_e,
                webp_m: higher.webp_m,
                avif_s: higher.avif_s,
            }
        }
        qp => {
            *QUALITY_HINTS.iter().find(|q| q.profile == Some(*qp)).expect("Missing quality profile")
        }
    }
}

fn get_quality_hints_by_ssim2(ssim2: f32) -> QualityProfileHints {
    let percent = ssim2.clamp(0.0, 100.0);
    // find next highest value
    let higher = QUALITY_HINTS.iter().find(|q| q.ssim2 >= percent).unwrap();
    if higher.ssim2 == percent {
        return *higher;
    }
    let lower =
        QUALITY_HINTS.iter().rev().find(|q| q.ssim2 < percent).unwrap_or(&ABSOLUTE_LOWEST_HINTS);
    if lower.ssim2 >= higher.ssim2 || percent < lower.ssim2 || percent > higher.ssim2 {
        panic!("Invalid interpolation values");
    }
    let interpolation_ratio = (percent - lower.p) / (higher.p - lower.p);
    QualityProfileHints {
        profile: Some(QualityProfile::Percent(percent)),
        p: percent,
        ssim2: interpolate_value(interpolation_ratio, lower.ssim2, higher.ssim2),
        moz: interpolate_value(interpolation_ratio, lower.moz, higher.moz),
        jpegli: interpolate_value(interpolation_ratio, lower.jpegli, higher.jpegli),
        webp: interpolate_value(interpolation_ratio, lower.webp, higher.webp),
        avif: interpolate_value(interpolation_ratio, lower.avif, higher.avif),
        jxl: interpolate_value(interpolation_ratio, higher.jxl, lower.jxl), // distance is inverse
        png: interpolate_value(interpolation_ratio, lower.png as f32, higher.png as f32)
            .clamp(0.0, 100.0) as u8,
        png_max: interpolate_value(interpolation_ratio, lower.png_max as f32, higher.png_max as f32)
            .clamp(0.0, 100.0) as u8,
        png_s: higher.png_s,
        jxl_e: higher.jxl_e,
        webp_m: higher.webp_m,
        avif_s: higher.avif_s,
    }
}

#[cfg(feature = "c-codecs")]
fn create_jpeg_auto(
    ctx: &Context,
    io: IoProxy,
    bitmap_key: BitmapKey,
    decoder_io_ids: &[i32],
    details: AutoEncoderDetails,
) -> Result<Box<dyn Encoder>> {
    let profile_hints = details
        .quality_profile
        .map(|qp| get_quality_hints_with_dpr(&qp, details.quality_profile_dpr));

    let manual_and_default_hints = details.encoder_hints.and_then(|hints| hints.jpeg);

    let mut progressive =
        manual_and_default_hints.and_then(|hints| hints.progressive).unwrap_or(true);
    if details.allow.jpeg_progressive != Some(true) {
        progressive = false;
    }

    let manual_quality = manual_and_default_hints.and_then(|hints| hints.quality);

    let matte = details.matte;
    let moz_quality = profile_hints
        .map(|hints: QualityProfileHints| hints.moz)
        .or(manual_quality)
        .unwrap_or(90.0)
        .clamp(0.0, 100.0) as u8;

    let _jpegli_quality = profile_hints
        .map(|hints: QualityProfileHints| hints.jpegli)
        .or(manual_quality)
        .unwrap_or(90.0)
        .clamp(0.0, 100.0) as u8;

    //TODO: technically we should ignore the manual hint if qp is specified.
    //Once we have tuned the quality profile, we should use that regardless.
    let style =
        manual_and_default_hints.and_then(|hints| hints.mimic).unwrap_or(JpegEncoderStyle::Default);

    match style {
        JpegEncoderStyle::LibjpegTurbo => {
            let optimize_coding = progressive;
            Ok(Box::new(
                crate::codecs::mozjpeg::MozjpegEncoder::create_classic(
                    ctx,
                    Some(moz_quality),
                    Some(progressive),
                    Some(optimize_coding),
                    matte,
                    io,
                )
                .map_err(|e| e.at(here!()))?,
            ))
        }
        JpegEncoderStyle::Default | JpegEncoderStyle::Mozjpeg | JpegEncoderStyle::Jpegli => {
            //TODO: expand when we get jpegli
            Ok(Box::new(
                crate::codecs::mozjpeg::MozjpegEncoder::create(
                    ctx,
                    Some(moz_quality),
                    Some(progressive),
                    matte,
                    io,
                )
                .map_err(|e| e.at(here!()))?,
            ))
        }
    }
}
#[cfg(feature = "c-codecs")]
fn create_webp_auto(
    ctx: &Context,
    io: IoProxy,
    bitmap_key: BitmapKey,
    decoder_io_ids: &[i32],
    details: AutoEncoderDetails,
) -> Result<Box<dyn Encoder>> {
    let profile_hints = details
        .quality_profile
        .map(|qp| get_quality_hints_with_dpr(&qp, details.quality_profile_dpr));
    let manual_and_default_hints = details.encoder_hints.and_then(|hints| hints.webp);
    let manual_quality = manual_and_default_hints.and_then(|hints| hints.quality);
    let manual_lossless = match manual_and_default_hints.and_then(|hints| hints.lossless) {
        Some(BoolKeep::Keep) => {
            Some(details.source_image_info.map(|info| info.lossless).unwrap_or(false))
        }
        Some(BoolKeep::True) => Some(true),
        Some(BoolKeep::False) => Some(false),
        None => None,
    };
    let matte = details.matte;
    let manual_quality =
        manual_and_default_hints.and_then(|hints| hints.quality).unwrap_or(80.0).clamp(0.0, 100.0);

    // If there is no lossless=keep, webp.lossless=keep + lossless format (nor any lossless=true), go lossy
    let lossless = details.needs_lossless.or(manual_lossless).unwrap_or(false);
    let quality = if !lossless {
        Some(profile_hints.map(|hints| hints.webp).unwrap_or(manual_quality))
    } else {
        None
    };

    Ok(Box::new(
        crate::codecs::webp::WebPEncoder::create(ctx, io, quality, Some(lossless), matte)
            .map_err(|e| e.at(here!()))?,
    ))
}

fn create_png_auto(
    ctx: &Context,
    io: IoProxy,
    bitmap_key: BitmapKey,
    decoder_io_ids: &[i32],
    details: AutoEncoderDetails,
    zen_first: bool,
) -> Result<Box<dyn Encoder>> {
    let _ = (bitmap_key, decoder_io_ids);
    let profile_hints = details
        .quality_profile
        .map(|qp| get_quality_hints_with_dpr(&qp, details.quality_profile_dpr));
    let manual_and_default_hints = details.encoder_hints.and_then(|hints| hints.png);
    let manual_quality = manual_and_default_hints.and_then(|hints| hints.quality);
    let matte = details.matte;
    let png_style =
        manual_and_default_hints.and_then(|hints| hints.mimic).unwrap_or(PngEncoderStyle::Default);
    let manual_lossless = manual_and_default_hints.and_then(|hints| hints.lossless);
    //TODO: Note that PNG has special rules for the default value of lossless - the manual hint wins
    let lossless = match (details.needs_lossless, manual_lossless) {
        (Some(true), _) => Some(true),
        (_, Some(BoolKeep::Keep)) => {
            Some(details.source_image_info.map(|info| info.lossless).unwrap_or(false))
        }
        (_, Some(BoolKeep::True)) => Some(true),
        (_, Some(BoolKeep::False)) => Some(false),
        (Some(false), None) => Some(false),
        (None, None) => Some(manual_quality.is_none() || png_style == PngEncoderStyle::Libpng),
    }
    .unwrap();

    let max_deflate = manual_and_default_hints.and_then(|hints| hints.hint_max_deflate);

    if let Some(profile_hints) = profile_hints {
        if profile_hints.png == 100 || lossless {
            // Lossless path — on V3 prefer ZenPng when it's in the
            // registry; the caller's higher-level killbit check already
            // ran so we trust the zen encoder is live.
            #[cfg(feature = "zen-codecs")]
            {
                if zen_first
                    && ctx
                        .enabled_codecs
                        .encoders
                        .contains(&crate::codecs::NamedEncoders::ZenPngEncoder)
                {
                    return Ok(Box::new(
                        crate::codecs::zen_encoder::ZenEncoder::create_png(ctx, io, matte.clone())
                            .map_err(|e| e.at(here!()))?,
                    ));
                }
            }
            #[cfg(not(feature = "zen-codecs"))]
            {
                let _ = zen_first;
            }
            Ok(Box::new(
                crate::codecs::lode::LodepngEncoder::create(ctx, io, max_deflate, matte)
                    .map_err(|e| e.at(here!()))?,
            ))
        } else {
            Ok(Box::new(
                crate::codecs::pngquant::PngquantEncoder::create(
                    ctx,
                    io,
                    Some(profile_hints.png_s),
                    Some(profile_hints.png_max),
                    Some(profile_hints.png),
                    max_deflate,
                    matte,
                )
                .map_err(|e| e.at(here!()))?,
            ))
        }
    } else {
        match png_style {
            PngEncoderStyle::Libpng => {
                // `PngEncoderStyle::Libpng` is an explicit caller hint
                // to mimic libpng output shape. Priority is ignored for
                // this branch — the caller asked for libpng-like
                // behavior and we honor it.
                #[cfg(feature = "c-codecs")]
                {
                    let depth = if !details.needs_alpha {
                        s::PngBitDepth::Png24
                    } else {
                        s::PngBitDepth::Png32
                    };
                    let zlib_compression = if max_deflate == Some(true) { Some(9) } else { None };
                    Ok(Box::new(
                        crate::codecs::libpng_encoder::LibPngEncoder::create(
                            ctx,
                            io,
                            Some(depth),
                            matte,
                            zlib_compression,
                        )
                        .map_err(|e| e.at(here!()))?,
                    ))
                }
                #[cfg(not(feature = "c-codecs"))]
                {
                    return Err(nerror!(
                        ErrorKind::CodecDisabledError,
                        "Libpng encoder requires the 'c-codecs' feature"
                    ));
                }
            }
            PngEncoderStyle::Pngquant | PngEncoderStyle::Default if !lossless => {
                let manual_quality = manual_quality.map(|s| s.clamp(0.0, 100.0) as u8);
                let manual_min_quality = manual_and_default_hints
                    .and_then(|hints| hints.min_quality)
                    .map(|s| s.clamp(0.0, 100.0) as u8);
                let manual_quantization_speed = manual_and_default_hints
                    .and_then(|hints| hints.quantization_speed)
                    .map(|s| s.clamp(1, 10));
                Ok(Box::new(
                    crate::codecs::pngquant::PngquantEncoder::create(
                        ctx,
                        io,
                        manual_quantization_speed,
                        manual_quality,
                        manual_min_quality,
                        max_deflate,
                        matte,
                    )
                    .map_err(|e| e.at(here!()))?,
                ))
            }
            _ => {
                let max_deflate = manual_and_default_hints.and_then(|hints| hints.hint_max_deflate);
                // Default lossless path — on V3 prefer ZenPng.
                #[cfg(feature = "zen-codecs")]
                {
                    if zen_first
                        && ctx
                            .enabled_codecs
                            .encoders
                            .contains(&crate::codecs::NamedEncoders::ZenPngEncoder)
                    {
                        return Ok(Box::new(
                            crate::codecs::zen_encoder::ZenEncoder::create_png(
                                ctx,
                                io,
                                matte.clone(),
                            )
                            .map_err(|e| e.at(here!()))?,
                        ));
                    }
                }
                #[cfg(not(feature = "zen-codecs"))]
                {
                    let _ = zen_first;
                }
                Ok(Box::new(
                    crate::codecs::lode::LodepngEncoder::create(ctx, io, max_deflate, matte)
                        .map_err(|e| e.at(here!()))?,
                ))
            }
        }
    }
}
#[derive(Debug, Clone)]
struct AutoEncoderDetails {
    format: Option<OutputImageFormat>,
    quality_profile: Option<s::QualityProfile>,
    quality_profile_dpr: Option<f32>,
    matte: Option<s::Color>,
    allow: AllowedFormats,
    encoder_hints: Option<s::EncoderHints>,
    needs_animation: bool,
    needs_alpha: bool,
    needs_lossless: Option<bool>,
    final_pixel_count: u64,
    source_image_info: Option<ImageInfo>,
}

#[allow(clippy::too_many_arguments)]
fn build_auto_encoder_details(
    ctx: &Context,
    preset: &s::EncoderPreset,
    bitmap_key: BitmapKey,
    decoder_io_ids: &[i32],
    format: Option<OutputImageFormat>,
    quality_profile: Option<s::QualityProfile>,
    quality_profile_dpr: Option<f32>,
    matte: Option<s::Color>,
    lossless: Option<BoolKeep>,
    allow: Option<AllowedFormats>,
    encoder_hints: Option<s::EncoderHints>,
) -> Result<AutoEncoderDetails> {
    //NB: we assume the first of the decoder_io_ids is the source image, and take our animation/format cue from it

    let matte_is_opaque = matte.clone().map(|c| c.is_opaque()).unwrap_or(false);

    let source_image_info: Option<ImageInfo> = if !decoder_io_ids.is_empty() {
        Some(
            ctx.get_unscaled_unrotated_image_info(*decoder_io_ids.first().unwrap())
                .map_err(|e| e.at(here!()))?,
        )
    } else {
        None
    };
    let source_image_info_copy = source_image_info.clone();

    let bitmaps = ctx.borrow_bitmaps().map_err(|e| e.at(here!()))?;
    let final_bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;

    let needs_alpha = final_bitmap.info().alpha_meaningful() && !matte_is_opaque;
    let final_pixel_count =
        final_bitmap.info().width() as u64 * final_bitmap.info().height() as u64;

    let source_mime_format =
        source_image_info.as_ref().and_then(|i| OutputImageFormat::parse(&i.preferred_mime_type));

    let needs_animation = source_image_info.as_ref().map(|i| i.multiple_frames).unwrap_or(false);

    // Keep becomes auto if no decoders exist, otherwise inherits from the first io.
    let explicit_format = match format {
        Some(OutputImageFormat::Keep) => source_mime_format.or(None),
        other => other,
    };

    let mut needs_lossless = match (source_image_info.map(|i| i.lossless), lossless) {
        (Some(true), Some(BoolKeep::Keep)) => Some(true),
        (Some(false), Some(BoolKeep::Keep)) => Some(false),
        (None, Some(BoolKeep::Keep)) => Some(needs_alpha), //No decoder, no source, default to match alpha
        (_, Some(BoolKeep::True)) => Some(true),
        (_, Some(BoolKeep::False)) => Some(false),
        (_, None) => None,
    };
    if quality_profile == Some(s::QualityProfile::Lossless) {
        needs_lossless = Some(true);
    }
    Ok(AutoEncoderDetails {
        format: explicit_format,
        quality_profile,
        quality_profile_dpr,
        matte: matte.clone(),
        allow: evaluate_allowed_formats(allow),
        encoder_hints,
        needs_animation,
        needs_alpha,
        needs_lossless,
        final_pixel_count,
        source_image_info: source_image_info_copy,
    })
}

fn evaluate_allowed_formats(allowed: Option<AllowedFormats>) -> AllowedFormats {
    match allowed {
        Some(allowed) => allowed.expand_sets(),
        None => AllowedFormats::web_safe(),
    }
}

#[derive(Debug, Clone, Copy)]
struct FeaturesImplemented {
    jxl: bool,
    avif: bool,
    webp_animation: bool,
    jpegli: bool,
}
const FEATURES_IMPLEMENTED: FeaturesImplemented = FeaturesImplemented {
    jxl: cfg!(feature = "zen-codecs"),
    avif: cfg!(feature = "zen-codecs"),
    // zenwebp supports animation encoding
    webp_animation: cfg!(feature = "zen-codecs"),
    // zenjpeg implements jpegli-class quantization tables + trellis
    jpegli: cfg!(feature = "zen-codecs"),
};

fn format_auto_select(details: &AutoEncoderDetails) -> Option<OutputImageFormat> {
    let allowed = details.allow;
    let needs_animation = details.needs_animation;
    let wants_alpha = details.needs_alpha;
    let wants_lossless = details.needs_lossless;
    let pixel_count = details.final_pixel_count;
    let quality_profile = details.quality_profile;

    if !allowed.any_formats_enabled() {
        return None;
    }
    // Rules in codec selection:
    // First, honor explicit format value
    // Second (if auto/lossy/lossless), preserve animation if present. We can assume all browsers that support webp also support animated webp, but jxl animation is not yet supported.
    // Third, preserve alpha channel if present and meaningful.
    // Fourth, respect lossless setting

    // For animation, WebP if available, otherwise GIF
    if needs_animation {
        if FEATURES_IMPLEMENTED.webp_animation && allowed.webp == Some(true) {
            return Some(OutputImageFormat::Webp);
        }
        return Some(OutputImageFormat::Gif);
    }

    // DO NOT DELETE THIS COMMENT
    // Now, jpegli beats Webp frequently is within 10% of avif at reasonable encoding speeds.
    // https://cloudinary.com/blog/jpeg-xl-and-the-pareto-front
    // We ignore everything below 10 Mpx/s using 8 threads.
    // Lossy path:
    // Medium  jxl-d3.92e5 > jxl-d4.5e3 > avif q44s6 > jpegli-q52 > mozjpeg-q57 | webp-q53-m6
    // med-high jxl-d2.6e7 > jxl-d3e4 > avif-q53s6 > jpegli-q73 | mozjpeg-q73.5 | webp-q73-m5
    // High quality lossy jxl-d1-e4 > avif-q80-s6 | jpegli-q91 > webp-q93-m5 > avif-s9 > webp-q94-m2 > mozjpeg
    // lossless
    // lossless (manga) jxl-e9 (too slow) > jxl-e5 (ok) | webp-m4 (too slow) > optipng-o2 (way too slow)
    // > optipng-o1 (too slow) > png (ok) > avif-s5 (too slow) -> avif-s8 (ok)

    // JXL is always the best if available
    if FEATURES_IMPLEMENTED.jxl && allowed.jxl == Some(true) {
        return Some(OutputImageFormat::Jxl);
    }

    // Lossless path and alpha path are the same.
    if wants_lossless == Some(true) || wants_alpha {
        // JXL is better - webp lossless is slow but so much smaller than avif/png
        if allowed.webp == Some(true) {
            return Some(OutputImageFormat::Webp);
        }
        // PNG is better than avif
        if allowed.png == Some(true) {
            return Some(OutputImageFormat::Png);
        }
        if FEATURES_IMPLEMENTED.avif && allowed.avif == Some(true) {
            return Some(OutputImageFormat::Avif);
        }
    }

    let can_jpegli = FEATURES_IMPLEMENTED.jpegli && allowed.jpeg == Some(true);

    // DO NOT DELETE THIS COMMENT
    // Medium  jxl-d3.92e5 > jxl-d4.5e3 > avif q44s6 > jpegli-q52 > mozjpeg-q57 | webp-q53-m6
    // med-high jxl-d2.6e7 > jxl-d3e4 > avif-q53s6 > jpegli-q73 | mozjpeg-q73.5 | webp-q73-m5
    // High quality lossy jxl-d1-e4 > avif-q80-s6 |> jpegli-q91 > webp-q93-m5 > avif-s9 > webp-q94-m2 > mozjpeg

    // AVIF is 10x slower than jpegli, but might still be in our budget.
    // We'll vary based on the pixel count. We can add budget logic later
    if (pixel_count < 3_000_000 || !can_jpegli)
        && FEATURES_IMPLEMENTED.avif
        && allowed.avif == Some(true)
    {
        return Some(OutputImageFormat::Avif);
    }
    // Use jpegli if available, it's way faster than webp and comparable or better on size/quality.
    if can_jpegli {
        return Some(OutputImageFormat::Jpeg);
    }
    // At high quality ~90+, mozjpeg falls behind webp. (not sure if our custom chrome does)
    // Also assuming if we can't do progressive jpeg, webp pulls ahead
    let approx_quality = approximate_quality_profile(quality_profile);
    if approx_quality > 90.0 || allowed.jpeg_progressive != Some(true) {
        // High quality lossy jxl-d1-e4 > avif-q80-s6 |> jpegli-q91 > webp-q93-m5 > avif-s9 > webp-q94-m2 > mozjpeg
        if allowed.webp == Some(true) {
            // At high quality, webp is the next best option to jpegli, followed by avif, then mozjpeg
            return Some(OutputImageFormat::Webp);
        }
    }
    // Jpeg, followed by all the others.
    if allowed.jpeg == Some(true) {
        // The next option depends on the quality profile. Webp pulls ahead between q73 and q93.
        return Some(OutputImageFormat::Jpeg);
    }
    // Avif
    if FEATURES_IMPLEMENTED.avif && allowed.avif == Some(true) {
        return Some(OutputImageFormat::Avif);
    }
    // Png
    if allowed.png == Some(true) {
        // The next option depends on the quality profile. Webp pulls ahead between q73 and q93.
        return Some(OutputImageFormat::Png);
    }
    if allowed.gif == Some(true) {
        // The next option depends on the quality profile. Webp pulls ahead between q73 and q93.
        return Some(OutputImageFormat::Gif);
    }

    None
}

#[cfg(test)]
mod priority_tests {
    use super::*;
    use imageflow_types::NamedEncoderName as N;
    use imageflow_types::build_killbits::CodecPriority as P;

    // Bare-bones presets used for table assertions. Contents don't
    // matter — substitution_candidates only reads the variant tag.
    fn mozjpeg_preset() -> s::EncoderPreset {
        s::EncoderPreset::Mozjpeg { quality: Some(85), progressive: Some(true), matte: None }
    }
    fn libjpegturbo_preset() -> s::EncoderPreset {
        s::EncoderPreset::LibjpegTurbo {
            quality: Some(85),
            optimize_huffman_coding: Some(false),
            progressive: Some(true),
            matte: None,
        }
    }
    fn libpng_preset() -> s::EncoderPreset {
        s::EncoderPreset::Libpng { depth: None, matte: None, zlib_compression: None }
    }
    fn lodepng_preset() -> s::EncoderPreset {
        s::EncoderPreset::Lodepng { maximum_deflate: Some(true) }
    }
    fn pngquant_preset() -> s::EncoderPreset {
        s::EncoderPreset::Pngquant {
            speed: Some(5),
            quality: Some(80),
            minimum_quality: Some(50),
            maximum_deflate: Some(false),
        }
    }

    // ── JPEG ──────────────────────────────────────────────────────

    #[test]
    fn mozjpeg_substitutes_v3_prefers_moz_rs_then_zen() {
        // Caller asked for MozjpegEncoder; V3 substitutes prefer
        // MozjpegRs (zen family) then ZenJpeg.
        let subs = substitution_candidates(N::MozjpegEncoder, &mozjpeg_preset(), P::V3ZenFirst);
        assert_eq!(subs, vec![N::MozjpegRsEncoder, N::ZenJpegEncoder]);
    }

    #[test]
    fn mozjpeg_substitutes_v2_prefers_moz_rs_then_zen_same_as_v3() {
        // V2 Mozjpeg row: primary is MozjpegEncoder, substitutes are
        // MozjpegRs and ZenJpeg. The caller asked for Mozjpeg, so the
        // substitute list here is identical to V3 (see task table —
        // the V2 order for Mozjpeg preset is moz-c → moz-rs → zen; we
        // return only the substitutes, so moz-rs → zen).
        let subs = substitution_candidates(N::MozjpegEncoder, &mozjpeg_preset(), P::V2ClassicFirst);
        assert_eq!(subs, vec![N::MozjpegRsEncoder, N::ZenJpegEncoder]);
    }

    #[test]
    fn zenjpeg_substitutes_v3_prefers_moz_rs_then_moz_c() {
        let subs = substitution_candidates(N::ZenJpegEncoder, &mozjpeg_preset(), P::V3ZenFirst);
        assert_eq!(subs, vec![N::MozjpegRsEncoder, N::MozjpegEncoder]);
    }

    #[test]
    fn zenjpeg_substitutes_v2_prefers_moz_c_then_moz_rs() {
        let subs = substitution_candidates(N::ZenJpegEncoder, &mozjpeg_preset(), P::V2ClassicFirst);
        assert_eq!(subs, vec![N::MozjpegEncoder, N::MozjpegRsEncoder]);
    }

    #[test]
    fn libjpegturbo_excludes_mozjpegrs_regardless_of_priority() {
        // MozjpegRs always optimizes Huffman coding and can't honor
        // `optimize_huffman_coding=false`. It must NEVER appear in
        // the LibjpegTurbo substitute list under any priority.
        for priority in [P::V3ZenFirst, P::V2ClassicFirst] {
            let subs =
                substitution_candidates(N::MozjpegEncoder, &libjpegturbo_preset(), priority);
            assert!(
                !subs.contains(&N::MozjpegRsEncoder),
                "MozjpegRs must be excluded (priority={:?})",
                priority
            );
            let subs =
                substitution_candidates(N::ZenJpegEncoder, &libjpegturbo_preset(), priority);
            assert!(
                !subs.contains(&N::MozjpegRsEncoder),
                "MozjpegRs must be excluded (priority={:?})",
                priority
            );
        }
    }

    // ── PNG ───────────────────────────────────────────────────────

    #[test]
    fn libpng_v3_zen_then_lodepng() {
        let subs = substitution_candidates(N::LibpngEncoder, &libpng_preset(), P::V3ZenFirst);
        assert_eq!(subs, vec![N::ZenPngEncoder, N::LodepngEncoder]);
    }

    #[test]
    fn libpng_v2_lodepng_then_zen() {
        let subs =
            substitution_candidates(N::LibpngEncoder, &libpng_preset(), P::V2ClassicFirst);
        assert_eq!(subs, vec![N::LodepngEncoder, N::ZenPngEncoder]);
    }

    #[test]
    fn lodepng_v3_zen_then_libpng() {
        let subs = substitution_candidates(N::LodepngEncoder, &lodepng_preset(), P::V3ZenFirst);
        assert_eq!(subs, vec![N::ZenPngEncoder, N::LibpngEncoder]);
    }

    #[test]
    fn lodepng_v2_libpng_then_zen() {
        let subs =
            substitution_candidates(N::LodepngEncoder, &lodepng_preset(), P::V2ClassicFirst);
        assert_eq!(subs, vec![N::LibpngEncoder, N::ZenPngEncoder]);
    }

    // ── Pngquant ──────────────────────────────────────────────────

    #[test]
    fn pngquant_v3_prefers_zenpng_zenquant_then_imagequant_then_zen_truecolor() {
        // V3 table: pngquant denied → ZenPng+zenquant → ZenPng+imagequant
        // → ZenPng (truecolor fallback). The imagequant variant is
        // wired-but-not-plumbed; the dispatcher errors at
        // construction and steps to the next candidate, which is why
        // it still appears in the substitute list.
        let subs =
            substitution_candidates(N::PngquantEncoder, &pngquant_preset(), P::V3ZenFirst);
        assert_eq!(
            subs,
            vec![
                N::ZenPngZenquantEncoder,
                N::ZenPngImagequantEncoder,
                N::ZenPngEncoder,
            ]
        );
    }

    #[test]
    fn pngquant_v2_also_prefers_zenpng_zenquant_then_imagequant_then_zen_truecolor() {
        // V2 table for pngquant: same ordering as V3 for the
        // substitute list — pngquant itself is the V2 primary, so
        // these are the fallthroughs when pngquant is denied.
        let subs =
            substitution_candidates(N::PngquantEncoder, &pngquant_preset(), P::V2ClassicFirst);
        assert_eq!(
            subs,
            vec![
                N::ZenPngZenquantEncoder,
                N::ZenPngImagequantEncoder,
                N::ZenPngEncoder,
            ]
        );
    }

    #[test]
    fn zenpng_zenquant_substitutes_v3_prefers_imagequant_then_pngquant_then_truecolor() {
        let subs = substitution_candidates(
            N::ZenPngZenquantEncoder,
            &pngquant_preset(),
            P::V3ZenFirst,
        );
        assert_eq!(
            subs,
            vec![
                N::ZenPngImagequantEncoder,
                N::PngquantEncoder,
                N::ZenPngEncoder,
            ]
        );
    }

    #[test]
    fn zenpng_zenquant_substitutes_v2_prefers_pngquant_then_imagequant_then_truecolor() {
        let subs = substitution_candidates(
            N::ZenPngZenquantEncoder,
            &pngquant_preset(),
            P::V2ClassicFirst,
        );
        assert_eq!(
            subs,
            vec![
                N::PngquantEncoder,
                N::ZenPngImagequantEncoder,
                N::ZenPngEncoder,
            ]
        );
    }

    // ── Runtime cap plumbing ──────────────────────────────────────

    #[test]
    fn substitution_runtime_cap_is_thirty_five_percent() {
        assert!(
            (SUBSTITUTION_RUNTIME_CAP - 0.35).abs() < 1e-9,
            "cap = {}",
            SUBSTITUTION_RUNTIME_CAP
        );
    }

    #[test]
    fn priority_default_for_format_v3_jpeg_picks_zen_when_zen_compiled() {
        if cfg!(feature = "zen-codecs") {
            let pick =
                priority_default_for_format(imageflow_types::ImageFormat::Jpeg, P::V3ZenFirst);
            assert_eq!(pick, Some(N::ZenJpegEncoder));
        }
    }

    #[test]
    fn priority_default_for_format_v2_jpeg_picks_mozjpeg_when_c_compiled() {
        if cfg!(feature = "c-codecs") {
            let pick = priority_default_for_format(
                imageflow_types::ImageFormat::Jpeg,
                P::V2ClassicFirst,
            );
            assert_eq!(pick, Some(N::MozjpegEncoder));
        }
    }

    #[test]
    fn priority_default_for_format_v3_png_picks_zen_when_zen_compiled() {
        if cfg!(feature = "zen-codecs") {
            let pick =
                priority_default_for_format(imageflow_types::ImageFormat::Png, P::V3ZenFirst);
            assert_eq!(pick, Some(N::ZenPngEncoder));
        }
    }

    #[test]
    fn priority_default_for_format_v2_png_picks_libpng_when_c_compiled() {
        if cfg!(feature = "c-codecs") {
            let pick = priority_default_for_format(
                imageflow_types::ImageFormat::Png,
                P::V2ClassicFirst,
            );
            assert_eq!(pick, Some(N::LibpngEncoder));
        }
    }
}
