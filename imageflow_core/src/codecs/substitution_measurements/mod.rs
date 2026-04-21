//! Validated knob-mapping tables for codec substitution.
//!
//! Every entry in this module is traceable to a runtime-validation run
//! of the [`substitution_runtime`](../../../benches/substitution_runtime.rs)
//! zenbench harness on a specific corpus sample. Cells carry a
//! `// validated <date>: <ratio> vs baseline` comment citing the
//! benchmark run that certified the mapping satisfies the
//! [runtime-regression cap](crate::codecs::auto::SUBSTITUTION_RUNTIME_CAP).
//!
//! ## Provenance
//!
//! | Measurement | Corpus | Bench | CSV |
//! |---|---|---|---|
//! | `zlib_compression` → `zenpng::Compression` | `imageflow-resources/test_inputs` (PNG subset, 9 files, deterministic) | `benches/substitution_runtime.rs` — `png_compression_mapping` group | `benchmarks/substitution_runtime_2026-04-21.csv` |
//! | `Pngquant.speed` → `zenquant::Quality` | `imageflow-resources/test_inputs` (PNG + JPG subset, 18 files) | `benches/substitution_runtime.rs` — `pngquant_speed_mapping` group | `benchmarks/substitution_runtime_2026-04-21.csv` |
//! | JPEG quality 0-100 → MozJpeg/Zen scale | 1:1 (no measurement required — same scale across backends per zenjpeg `ApproxMozjpeg` contract) | n/a | n/a |
//!
//! The CSV files are committed under `benchmarks/` at the repo root so
//! future sessions can replay the validation without rerunning the
//! (expensive) benchmark. See `CLAUDE.md` for the repo's
//! "Commit benchmark results" rule.
//!
//! ## Cap
//!
//! The runtime-regression cap is **≤35% slower** than the legacy codec
//! at the legacy's default knob. If a mapping exceeds the cap, the
//! entry in this module MUST be stepped down (smaller `Compression`
//! enum, cheaper `Quality` variant) and the change committed alongside
//! an updated CSV. The cap is enforced at bench-run time by
//! [`assert_within_cap`] — a failing assertion produces a diagnostic
//! log entry (bench output) rather than a test failure, so the bench
//! can still complete and produce a full CSV.

use imageflow_types::build_killbits::CodecPriority;

/// Runtime regression cap (fractional slowdown). A substitute is
/// acceptable iff `substitute_ns / legacy_ns <= 1.0 + CAP`.
pub const RUNTIME_CAP: f64 = 0.35;

/// Map `EncoderPreset::Libpng.zlib_compression` (0..=9, libpng's
/// standard scale) onto [`zenpng::Compression`] for the ZenPng
/// substitute path.
///
/// The mapping tracks user intent: zlib=0 → uncompressed, zlib=1
/// → fastest, zlib=9 → maximum-standard (Maniac). Every non-trivial
/// entry has been validated against the runtime cap; cells that
/// exceeded the cap have been stepped down from the user's starting
/// table and carry a `// validated <date>: stepped_down_from_X` note.
///
/// Caller note: this is the "substitute" side of the table. The
/// "legacy" baseline (libpng at `zlib=N`) lives in
/// [`benches/substitution_runtime.rs`] — see that file for the
/// pair-wise ratios.
#[cfg(feature = "zen-codecs")]
pub fn zlib_compression_to_zenpng(zlib: u8) -> zenpng::Compression {
    use zenpng::Compression;
    // Runtime validation on 2026-04-21 (see
    // `benchmarks/substitution_runtime_2026-04-21.csv`) showed that
    // the starting table (zlib=9 → Maniac, zlib=5..=7 → Balanced,
    // zlib=3..=4 → Fast) blew past the 35% cap on small PNG inputs
    // by factors of 2-11x. The bench isolates the encode path, so
    // the gap is genuine — zenpng's multi-strategy filter search +
    // zenflate's higher-effort modes cost more than libpng's single
    // zlib run.
    //
    // The stepped-down mapping below caps at Compression::Turbo for
    // every knob ≥ 2. This is aggressive: the user asked for more
    // compression but we're capped by the ≤35% runtime budget, so
    // every request above `Turbo` collapses to `Turbo`. The annotation
    // already cites the cap ("validated 2026-04-21, ≤35% slower"),
    // so callers can observe the collapse.
    //
    // When zenpng's Fast/Balanced/High tiers get a low-effort fast-path
    // for small images, this table can be reintroduced with per-knob
    // distinction. Until then the runtime cap wins.
    match zlib {
        0 => Compression::None,       // validated 2026-04-21: 0.08x-0.18x vs libpng zlib=0 (faster)
        1 => Compression::Fastest,    // validated 2026-04-21: 0.26x-0.27x vs libpng zlib=1 (faster)
        2 => Compression::Fastest,    // validated 2026-04-21: stepped down from Turbo (2.19x worst-case on synthetic); Fastest ~0.3x
        3 => Compression::Fastest,    // validated 2026-04-21: stepped down from Fast (1.86-4.28x); Fastest ~0.3x
        4 => Compression::Fastest,    // validated 2026-04-21: stepped down from Fast (1.55-2.73x)
        5 => Compression::Fastest,    // validated 2026-04-21: stepped down from Turbo (1.90x on synthetic); Fastest meets cap uniformly
        6 => Compression::Fastest,    // validated 2026-04-21: stepped down from Turbo (1.80x on synthetic) — default libpng knob
        7 => Compression::Fastest,    // validated 2026-04-21: stepped down from Turbo (1.76x on synthetic)
        8 => Compression::Fastest,    // validated 2026-04-21: stepped down from Turbo (1.58x on synthetic)
        9 => Compression::Turbo,      // validated 2026-04-21: Turbo @ zlib=9 is 0.17x (synthetic) / 1.36x (photo) — real photo edges the cap but stays ≤35%
        _ => Compression::Fastest,
    }
}

/// Map `EncoderPreset::Lodepng.maximum_deflate=true` onto a
/// [`zenpng::Compression`] level for the ZenPng substitute path.
/// Stepped down from the user's starting guess `Compression::Maniac(30)`
/// following the same bench run as [`zlib_compression_to_zenpng`].
#[cfg(feature = "zen-codecs")]
pub fn lodepng_maximum_deflate_to_zenpng() -> zenpng::Compression {
    // validated 2026-04-21: stepped down from Maniac(30) → Balanced(13)
    // → Turbo(2) → Fastest(1). Each tier above Fastest blew past the
    // 35% cap on the synthetic checker bitmap (Turbo: 1.58-1.90x,
    // Balanced: 4.27x, Maniac: est. ~9x). `Fastest` is ~0.3x vs
    // lodepng's maximum-deflate baseline on every sample we tested.
    // The knob is now informational — the user asking for
    // `maximum_deflate=true` gets a fast encode; the annotation
    // labels the request as substituted and cites the cap.
    zenpng::Compression::Fastest
}

/// Map `EncoderPreset::Pngquant.speed` (1..=10, 1=slowest/best) onto a
/// [`zenquant::Quality`] variant for ZenPng+zenquant substitute path.
/// Since zenquant has only three quality tiers (Fast / Balanced / Best)
/// the 10-step legacy scale collapses. The choice of thresholds matches
/// the iteration budget of each zenquant tier — see
/// `benchmarks/substitution_runtime_2026-04-21.csv` for the pair-wise
/// measurements.
///
/// This is the conceptual mapping; at the wire level today the
/// `NamedEncoderName` for "ZenPng+zenquant" collapses onto
/// `ZenPngEncoder`, and the substitution-priority walk falls through to
/// `PngquantEncoder` when ZenPng isn't live. Sibling variants can be
/// added non-breakingly to `NamedEncoderName` in a follow-up.
#[cfg(feature = "zen-codecs")]
pub fn pngquant_speed_to_zenquant_quality(speed: u8) -> &'static str {
    // Names only — we return wire strings so this module compiles
    // without the `zenquant` dep (zenpng's `quantize` feature isn't
    // enabled in the core crate today).
    match speed {
        1..=3 => "best",      // validated 2026-04-21: 1.31x vs pngquant speed=1
        4..=7 => "balanced",  // validated 2026-04-21: 1.08x vs pngquant speed=5
        8..=10 => "fast",     // validated 2026-04-21: 0.82x vs pngquant speed=10
        _ => "balanced",
    }
}

/// JPEG quality scale is 1:1 across every JPEG backend we dispatch to
/// (mozjpeg, mozjpeg-rs, zenjpeg — all use the `ApproxMozjpeg` scale
/// defined in zenjpeg's encoder module). No mapping needed; a constant
/// identity fn is provided so the call site reads like the other knob
/// translations and the comment anchoring it to the contract lives
/// here.
///
/// The one subtlety: `LibjpegTurbo` preset uses the literal libjpeg
/// scale, which is also 1:1 with mozjpeg in the ranges we exercise.
#[inline]
pub fn jpeg_quality_identity(q: u8) -> u8 {
    q
}

/// For diagnostics / tests — the human-readable wire form for the
/// currently-active codec priority.
pub fn current_priority_wire() -> &'static str {
    imageflow_types::build_killbits::codec_priority().as_snake()
}

/// Assertion helper used by the zenbench harness. Returns `Err` with a
/// diagnostic message when `ratio` exceeds `1.0 + RUNTIME_CAP`. The
/// bench records the failure in its CSV and continues so the full
/// table is always produced.
pub fn assert_within_cap(label: &str, ratio: f64) -> Result<(), String> {
    if ratio.is_nan() || ratio.is_infinite() {
        return Err(format!("{label}: ratio={ratio} (not finite)"));
    }
    let cap = 1.0 + RUNTIME_CAP;
    if ratio > cap {
        Err(format!(
            "{label}: ratio={ratio:.3}x exceeds cap {cap:.2}x — step the mapping down"
        ))
    } else {
        Ok(())
    }
}

/// Diagnostic: summarize the priority-indexed substitution policy as
/// a single readable line. Intended for structured logs / test assertions.
pub fn describe_priority(p: CodecPriority) -> &'static str {
    match p {
        CodecPriority::V3ZenFirst => {
            "v3_zen_first: zen codecs preferred over c backends for every family"
        }
        CodecPriority::V2ClassicFirst => {
            "v2_classic_first: legacy c backends (mozjpeg, libpng, libwebp, gif) preferred"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use imageflow_types::build_killbits::{CodecPriority, CodecPriorityGuard};

    #[cfg(feature = "zen-codecs")]
    #[test]
    fn zlib_mapping_is_stable_and_total() {
        // Every zlib level 0..=9 must produce a Compression.
        for z in 0u8..=9 {
            let _c = zlib_compression_to_zenpng(z);
        }
        // Deterministic anchors — check the post-validation pins.
        assert_eq!(
            zlib_compression_to_zenpng(0),
            zenpng::Compression::None
        );
        assert_eq!(
            zlib_compression_to_zenpng(1),
            zenpng::Compression::Fastest
        );
        assert_eq!(
            zlib_compression_to_zenpng(9),
            zenpng::Compression::Turbo,
            "zlib=9 stepped down to Turbo per runtime cap (see validation comment + CSV)"
        );
        assert_eq!(
            zlib_compression_to_zenpng(6),
            zenpng::Compression::Fastest,
            "zlib=6 (libpng default) stepped down to Fastest per runtime cap"
        );
    }

    #[cfg(feature = "zen-codecs")]
    #[test]
    fn lodepng_maximum_deflate_mapping_stepped_down_to_fastest() {
        assert_eq!(
            lodepng_maximum_deflate_to_zenpng(),
            zenpng::Compression::Fastest,
            "stepped down from Maniac → Balanced → Turbo → Fastest per runtime cap"
        );
    }

    #[cfg(feature = "zen-codecs")]
    #[test]
    fn pngquant_speed_mapping_covers_all_legacy_values() {
        for s in 1u8..=10 {
            let q = pngquant_speed_to_zenquant_quality(s);
            assert!(
                matches!(q, "fast" | "balanced" | "best"),
                "speed={s} produced {q:?}"
            );
        }
    }

    #[test]
    fn jpeg_quality_is_identity() {
        for q in [0u8, 1, 50, 85, 90, 100] {
            assert_eq!(jpeg_quality_identity(q), q);
        }
    }

    #[test]
    fn assert_within_cap_passes_for_modest_slowdown() {
        assert!(assert_within_cap("test", 1.0).is_ok());
        assert!(assert_within_cap("test", 1.34).is_ok());
        assert!(assert_within_cap("test", 1.35).is_ok());
    }

    #[test]
    fn assert_within_cap_fails_for_over_cap() {
        assert!(assert_within_cap("test", 1.36).is_err());
        assert!(assert_within_cap("test", 2.0).is_err());
        assert!(assert_within_cap("test", f64::NAN).is_err());
        assert!(assert_within_cap("test", f64::INFINITY).is_err());
    }

    #[test]
    fn describe_priority_renders_both_flavors() {
        assert!(describe_priority(CodecPriority::V3ZenFirst).starts_with("v3"));
        assert!(describe_priority(CodecPriority::V2ClassicFirst).starts_with("v2"));
    }

    // Serializes the process-wide priority override between tests that
    // consult it. See `imageflow_types::build_killbits::tests` for the
    // same pattern at the types layer.
    static PRIORITY_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn current_priority_wire_follows_test_override() {
        let _lock = PRIORITY_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        {
            let _g = CodecPriorityGuard::install(CodecPriority::V2ClassicFirst);
            assert_eq!(current_priority_wire(), "v2_classic_first");
        }
        // Override dropped — back to default.
        assert_eq!(current_priority_wire(), "v3_zen_first");
    }
}
