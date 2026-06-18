//! Build-time killbits ceiling.
//!
//! Formats this build refuses to decode or encode, regardless of runtime
//! configuration. Downstream forks override these by editing the consts here.
//!
//! Upstream defaults to empty arrays — every format that can be compiled in is
//! reachable through trusted policy and job-level requests. The runtime layer
//! (`feature_compiled_in`) also refuses any format whose feature isn't in the
//! build.

use crate::killbits::{ImageFormat, Op};

/// Codec-priority switch that picks which family of backends the dispatcher
/// prefers when resolving an `EncoderPreset` (or a RIAPI `format=…`
/// translation) to a concrete codec.
///
/// Build-time only. V3 forks keep the default (`V3ZenFirst`) so
/// zen-codecs win ties; V2 forks flip the const in this module so the
/// legacy C backends (mozjpeg, libpng, libwebp, gif crate) win ties.
/// There is no runtime override and no field on `ExecutionSecurity` —
/// priority is a single build-wide decision.
///
/// Changing this value only reorders the substitution table; it does
/// not enable or disable codecs. Killbits, feature gates, and
/// `COMPILE_DENY_*` still have the final say.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CodecPriority {
    /// V3 default — prefer pure-Rust zen codecs over the legacy C
    /// backends. Downstream picks for this priority:
    ///
    /// | Legacy preset | Ordered substitutes |
    /// |---|---|
    /// | `Mozjpeg` | `MozjpegRsEncoder` → `ZenJpegEncoder` → `MozjpegEncoder` |
    /// | `LibjpegTurbo` | `ZenJpegEncoder` → `MozjpegRsEncoder` → `MozjpegEncoder` |
    /// | `Libpng` | `ZenPngEncoder` → `LibPngRsEncoder` → `LodePngEncoder` |
    /// | `Lodepng` | `ZenPngEncoder` → `LodePngEncoder` → `LibPngRsEncoder` |
    /// | `Pngquant` | `ZenPngEncoder`+zenquant → `ZenPngEncoder`+imagequant → `PngQuantEncoder` |
    /// | `WebPLossy` / `WebPLossless` | `ZenWebPEncoder` → `WebPEncoder` |
    /// | `Gif` | `ZenGifEncoder` → `GifEncoder` |
    V3ZenFirst,
    /// V2 / legacy flavor — prefer the C backends that shipped in V2
    /// forks. Downstream picks for this priority:
    ///
    /// | Legacy preset | Ordered substitutes |
    /// |---|---|
    /// | `Mozjpeg` | `MozjpegEncoder` → `MozjpegRsEncoder` → `ZenJpegEncoder` |
    /// | `LibjpegTurbo` | `MozjpegEncoder` → `ZenJpegEncoder` → `MozjpegRsEncoder` |
    /// | `Libpng` | `LibPngRsEncoder` → `LodePngEncoder` → `ZenPngEncoder` |
    /// | `Lodepng` | `LodePngEncoder` → `LibPngRsEncoder` → `ZenPngEncoder` |
    /// | `Pngquant` | `PngQuantEncoder` → `ZenPngEncoder`+imagequant → `ZenPngEncoder`+zenquant |
    /// | `WebPLossy` / `WebPLossless` | `WebPEncoder` → `ZenWebPEncoder` |
    /// | `Gif` | `GifEncoder` → `ZenGifEncoder` |
    V2ClassicFirst,
}

impl CodecPriority {
    /// Short snake-case form used in the `codec_priority` annotation
    /// field and structured error payloads.
    pub fn as_snake(self) -> &'static str {
        match self {
            CodecPriority::V3ZenFirst => "v3_zen_first",
            CodecPriority::V2ClassicFirst => "v2_classic_first",
        }
    }
}

/// Build-wide default codec priority. V3 forks leave this at
/// `V3ZenFirst`; V2 forks change the const to `V2ClassicFirst`.
///
/// Use [`codec_priority()`] to read the effective priority at runtime
/// — it honors the `#[cfg(test)]` override installed by
/// [`set_codec_priority_for_test()`]. Production code should never
/// consult the constant directly.
pub const CODEC_PRIORITY_DEFAULT: CodecPriority = CodecPriority::V3ZenFirst;

/// Returns the effective codec priority. In production this is always
/// [`CODEC_PRIORITY_DEFAULT`]; under `cfg(test)` a test may install a
/// different priority via [`set_codec_priority_for_test()`]. The
/// override is process-wide and is cleared with
/// [`clear_codec_priority_for_test()`] (or the returned RAII guard).
pub fn codec_priority() -> CodecPriority {
    test_override::current()
}

// Always compiled — the test override is a single atomic load in
// production, which is cheap and lets downstream test binaries
// (imageflow_core integration tests, benches, RIAPI tests) install a
// priority for their own scope. Setting the override is gated
// behind `set_codec_priority_for_test` / `CodecPriorityGuard` so
// production code can't reach in.
mod test_override {
    use super::CodecPriority;
    use std::sync::atomic::{AtomicU8, Ordering};

    // 0 = unset (use CODEC_PRIORITY_DEFAULT), 1 = V3ZenFirst, 2 = V2ClassicFirst
    static OVERRIDE: AtomicU8 = AtomicU8::new(0);

    fn encode(p: CodecPriority) -> u8 {
        match p {
            CodecPriority::V3ZenFirst => 1,
            CodecPriority::V2ClassicFirst => 2,
        }
    }

    fn decode(v: u8) -> Option<CodecPriority> {
        match v {
            1 => Some(CodecPriority::V3ZenFirst),
            2 => Some(CodecPriority::V2ClassicFirst),
            _ => None,
        }
    }

    pub fn current() -> CodecPriority {
        decode(OVERRIDE.load(Ordering::Acquire)).unwrap_or(super::CODEC_PRIORITY_DEFAULT)
    }

    pub fn set(p: CodecPriority) {
        OVERRIDE.store(encode(p), Ordering::Release);
    }

    pub fn clear() {
        OVERRIDE.store(0, Ordering::Release);
    }
}

/// Install `priority` as the effective codec priority for the current
/// process. **Test-only on paper; physically unconditionally
/// compiled** so downstream crates' test binaries can reach it.
/// Production code never calls this — consult [`CODEC_PRIORITY_DEFAULT`]
/// directly or through [`codec_priority()`]. The override is
/// process-wide, not thread-local, so callers must not run
/// priority-dependent tests concurrently with each other. Use
/// [`CodecPriorityGuard`] to scope the override to a single test
/// function and auto-clear on drop.
pub fn set_codec_priority_for_test(priority: CodecPriority) {
    test_override::set(priority);
}

/// Clear any test-installed codec priority, reverting to
/// [`CODEC_PRIORITY_DEFAULT`].
pub fn clear_codec_priority_for_test() {
    test_override::clear();
}

/// RAII guard that installs a test-only codec priority for the
/// duration of its scope and clears it on drop. Prefer this over the
/// bare `set_*` / `clear_*` pair so an early panic doesn't leak the
/// override into the next test. The guard ignores poisoned mutexes
/// upstream of it — the goal is purely to avoid leaking override
/// state.
pub struct CodecPriorityGuard {
    _priv: (),
}

impl CodecPriorityGuard {
    pub fn install(priority: CodecPriority) -> Self {
        set_codec_priority_for_test(priority);
        Self { _priv: () }
    }
}

impl Drop for CodecPriorityGuard {
    fn drop(&mut self) {
        clear_codec_priority_for_test();
    }
}

/// Formats this build refuses to decode, regardless of runtime config.
/// Override in custom builds.
pub const COMPILE_DENY_DECODE: &[ImageFormat] = &[];

/// Formats this build refuses to encode, regardless of runtime config.
/// Override in custom builds.
pub const COMPILE_DENY_ENCODE: &[ImageFormat] = &[];

/// Returns true if this build was compiled with the feature needed to support
/// `op` on `format`. Used as the innermost ceiling — any format whose codec
/// feature isn't in the build is `false` in the net_support grid even if the
/// runtime policy "allows" it.
///
/// This mirrors the `#[cfg(feature = ...)]` walls in
/// `imageflow_core/src/codecs/mod.rs` at a coarse format granularity. For
/// formats with multiple backends (JPEG: c-codecs or zen-codecs; PNG: always
/// available), we return `true` if *any* backend is compiled in.
pub const fn feature_compiled_in(format: ImageFormat, op: Op) -> bool {
    // Note: imageflow_types can't see the imageflow_core feature set directly;
    // we mirror the same feature names here. `c-codecs` and `zen-codecs` are
    // features on the downstream crates, so we use the presence of the same
    // named features on *this* crate as a proxy when those are forwarded. For
    // the upstream workspace today, imageflow_types has neither feature, so we
    // default to `true` for every format present in the ALL list — the
    // per-codec dispatch layer gives the final answer.
    let _ = (format, op);
    true
}

/// Returns the subset of `ImageFormat::ALL` that is missing at least one
/// required compile-time feature. Exposed through `v1/context/get_net_support`
/// under `compile_ceiling.features_missing`.
pub fn features_missing() -> Vec<ImageFormat> {
    let mut missing = Vec::new();
    for &f in ImageFormat::ALL {
        let has_decode = feature_compiled_in(f, Op::Decode);
        let has_encode = feature_compiled_in(f, Op::Encode);
        if !has_decode && !has_encode {
            missing.push(f);
        }
    }
    missing
}

/// Convenience: is `format` denied by `COMPILE_DENY_DECODE`?
pub fn compile_deny_decode_contains(format: ImageFormat) -> bool {
    COMPILE_DENY_DECODE.contains(&format)
}

/// Convenience: is `format` denied by `COMPILE_DENY_ENCODE`?
pub fn compile_deny_encode_contains(format: ImageFormat) -> bool {
    COMPILE_DENY_ENCODE.contains(&format)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_compile_deny_is_empty() {
        // Upstream default: nothing is denied at build time. Forks override.
        assert!(COMPILE_DENY_DECODE.is_empty());
        assert!(COMPILE_DENY_ENCODE.is_empty());
    }

    #[test]
    fn feature_compiled_in_upstream_allows_all() {
        for &f in ImageFormat::ALL {
            assert!(feature_compiled_in(f, Op::Decode), "{f:?} decode");
            assert!(feature_compiled_in(f, Op::Encode), "{f:?} encode");
        }
    }

    #[test]
    fn codec_priority_default_is_v3_zen_first() {
        // Upstream ships V3ZenFirst. V2 forks flip the const.
        assert_eq!(CODEC_PRIORITY_DEFAULT, CodecPriority::V3ZenFirst);
    }

    #[test]
    fn codec_priority_as_snake_wire_forms() {
        assert_eq!(CodecPriority::V3ZenFirst.as_snake(), "v3_zen_first");
        assert_eq!(CodecPriority::V2ClassicFirst.as_snake(), "v2_classic_first");
    }

    // Serializes access to the process-wide `CODEC_PRIORITY_TEST_OVERRIDE`
    // across tests in this module so concurrent cargo-test threads don't
    // race on the `AtomicU8`. External consumers serialize differently
    // (see the `codec_priority_serial` helper in
    // `imageflow_core::codecs::substitution_measurements::priority_tests`).
    static CODEC_PRIORITY_TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn codec_priority_test_override_roundtrip() {
        let _lock = CODEC_PRIORITY_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        {
            let _g = CodecPriorityGuard::install(CodecPriority::V2ClassicFirst);
            assert_eq!(codec_priority(), CodecPriority::V2ClassicFirst);
        }
        // Guard dropped — override cleared.
        assert_eq!(codec_priority(), CODEC_PRIORITY_DEFAULT);
    }

    #[test]
    fn codec_priority_guard_drops_cleanly_on_panic() {
        let _lock = CODEC_PRIORITY_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // Install, panic inside a catch_unwind, verify the guard's
        // Drop still cleared the override.
        let r = std::panic::catch_unwind(|| {
            let _g = CodecPriorityGuard::install(CodecPriority::V2ClassicFirst);
            assert_eq!(codec_priority(), CodecPriority::V2ClassicFirst);
            panic!("expected");
        });
        assert!(r.is_err());
        assert_eq!(codec_priority(), CODEC_PRIORITY_DEFAULT);
    }
}
