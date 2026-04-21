//! Runtime enforcement for the three-layer codec killbits system.
//!
//! The data types and allow/deny logic live in
//! `imageflow_types::killbits`. This module wires them into the
//! imageflow_core feature set (`c-codecs` / `zen-codecs`) and provides
//! the helpers used by `Context::configure_security`, the
//! `v1/context/set_policy` endpoint, and the decode/encode dispatch
//! path.

use crate::codecs::{EnabledCodecs, NamedDecoders, NamedEncoders};
use crate::{ErrorKind, FlowError, Result};
use imageflow_types as s;
use imageflow_types::build_killbits;
use imageflow_types::killbits::{FormatKillbits, ImageFormat, Op};
pub use imageflow_types::killbits::{FormatGrid, FormatPermissions};
use imageflow_types::{CodecKillbits, NamedDecoderName, NamedEncoderName};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Process-wide cache of the compile-time ceiling. The compile ceiling is
/// `&'static` the moment the binary is built — the list of denied
/// decode/encode formats and the missing-feature set are fixed by the
/// feature gates at compile time. Cache once and hand out references.
static COMPILE_CEILING: OnceLock<CompileCeiling> = OnceLock::new();

/// Return the process-wide compile ceiling, computing it on first access.
/// Never needs invalidation — the inputs (`build_killbits` const arrays
/// and the `feature_compiled_in` cfg map) don't change at runtime.
pub fn compile_ceiling() -> &'static CompileCeiling {
    COMPILE_CEILING.get_or_init(compute_compile_ceiling)
}

fn compute_compile_ceiling() -> CompileCeiling {
    CompileCeiling {
        denied_decode: build_killbits::COMPILE_DENY_DECODE
            .iter()
            .map(|f| f.as_snake().to_string())
            .collect(),
        denied_encode: build_killbits::COMPILE_DENY_ENCODE
            .iter()
            .map(|f| f.as_snake().to_string())
            .collect(),
        features_missing: features_missing_in_core()
            .iter()
            .map(|f| f.as_snake().to_string())
            .collect(),
    }
}

/// Returns whether this build has a codec backend compiled in for
/// `format` and `op`. Reflects the `c-codecs` / `zen-codecs` feature
/// gates in `imageflow_core/src/codecs/mod.rs`.
pub fn feature_compiled_in(format: ImageFormat, op: Op) -> bool {
    match (format, op) {
        // JPEG: any of c-codecs (mozjpeg) or zen-codecs (zenjpeg/mozjpeg-rs)
        (ImageFormat::Jpeg, _) => {
            cfg!(feature = "c-codecs") || cfg!(feature = "zen-codecs")
        }
        // PNG: always available (pngquant+lodepng baseline, always in default deps)
        (ImageFormat::Png, _) => true,
        // GIF: always available (gif crate + optional zengif)
        (ImageFormat::Gif, _) => true,
        // WebP: c-codecs (libwebp) or zen-codecs (zenwebp)
        (ImageFormat::Webp, _) => {
            cfg!(feature = "c-codecs") || cfg!(feature = "zen-codecs")
        }
        // AVIF: zen-codecs only (zenavif/rav1d/zenrav1e)
        (ImageFormat::Avif, _) => cfg!(feature = "zen-codecs"),
        // JXL: zen-codecs only (zenjxl)
        (ImageFormat::Jxl, _) => cfg!(feature = "zen-codecs"),
        // Heic: not yet implemented in this build.
        (ImageFormat::Heic, _) => false,
        // Bmp: zen-codecs only (zenbmp/zenbitmaps)
        (ImageFormat::Bmp, _) => cfg!(feature = "zen-codecs"),
        // Tiff: not yet implemented in this build.
        (ImageFormat::Tiff, _) => false,
        // Pnm: not yet implemented in this build.
        (ImageFormat::Pnm, _) => false,
        // `ImageFormat` is `#[non_exhaustive]`; default-deny any new
        // formats added upstream until we wire them up here.
        _ => false,
    }
}

/// Formats with at least one compiled-in backend, collapsed into a
/// list for the `compile_ceiling.features_missing` response.
pub fn features_missing_in_core() -> Vec<ImageFormat> {
    let mut out = Vec::new();
    for &f in ImageFormat::ALL {
        if !feature_compiled_in(f, Op::Decode) && !feature_compiled_in(f, Op::Encode) {
            out.push(f);
        }
    }
    out
}

/// The starting grid for the three-layer system: every format that is
/// compiled in *and* not on the `COMPILE_DENY_*` ceiling.
pub fn build_ceiling_grid() -> FormatGrid {
    let mut grid = FormatGrid::none();
    for &f in ImageFormat::ALL {
        let compile_allows_decode = !build_killbits::compile_deny_decode_contains(f)
            && feature_compiled_in(f, Op::Decode);
        let compile_allows_encode = !build_killbits::compile_deny_encode_contains(f)
            && feature_compiled_in(f, Op::Encode);
        grid.set(f, Op::Decode, compile_allows_decode);
        grid.set(f, Op::Encode, compile_allows_encode);
    }
    grid
}

/// Parse a format name string (snake_case) into an `ImageFormat`.
/// Accepts the aliases used elsewhere in imageflow ("jpg", "jpeg"). Used
/// by the dispatch layer to identify the format being decoded/encoded.
pub fn parse_format_name(name: &str) -> Option<ImageFormat> {
    match name.to_ascii_lowercase().as_str() {
        "jpeg" | "jpg" => Some(ImageFormat::Jpeg),
        "png" => Some(ImageFormat::Png),
        "gif" => Some(ImageFormat::Gif),
        "webp" => Some(ImageFormat::Webp),
        "avif" => Some(ImageFormat::Avif),
        "jxl" => Some(ImageFormat::Jxl),
        "heic" | "heif" => Some(ImageFormat::Heic),
        "bmp" => Some(ImageFormat::Bmp),
        "tiff" | "tif" => Some(ImageFormat::Tiff),
        "pnm" | "ppm" | "pgm" | "pbm" | "pam" | "pfm" => Some(ImageFormat::Pnm),
        _ => None,
    }
}

/// Map `OutputImageFormat` (the internal format enum used by encoder
/// dispatch) to the killbits `ImageFormat` where possible. Returns
/// `None` for `Keep` (the format isn't yet resolved).
pub fn from_output_format(fmt: s::OutputImageFormat) -> Option<ImageFormat> {
    match fmt {
        s::OutputImageFormat::Jpeg | s::OutputImageFormat::Jpg => Some(ImageFormat::Jpeg),
        s::OutputImageFormat::Png => Some(ImageFormat::Png),
        s::OutputImageFormat::Gif => Some(ImageFormat::Gif),
        s::OutputImageFormat::Webp => Some(ImageFormat::Webp),
        s::OutputImageFormat::Avif => Some(ImageFormat::Avif),
        s::OutputImageFormat::Jxl => Some(ImageFormat::Jxl),
        s::OutputImageFormat::Keep => None,
    }
}

/// The "net_support" format grid produced by combining all three layers:
///   build ceiling ∩ trusted policy ∩ job-level narrowing (if any),
/// **then folded through codec-level killbits**: if every compiled-in
/// encoder for a format is denied, the format flips to "encode: false"
/// regardless of what the format-level grid said. Same rule for decode.
///
/// `trusted` and `job_request` may be `None` to skip that layer.
pub fn compute_net_support(
    trusted: Option<&s::ExecutionSecurity>,
    job_request: Option<&s::ExecutionSecurity>,
) -> FormatGrid {
    compute_net_support_with_codecs(
        trusted,
        job_request,
        // Use the default enabled-codec baseline for convenience.
        &EnabledCodecs::default(),
    )
}

/// Same as [`compute_net_support`] but consults an explicit
/// `EnabledCodecs` to decide which codecs are live. The default
/// [`compute_net_support`] plugs in `EnabledCodecs::default()`.
pub fn compute_net_support_with_codecs(
    trusted: Option<&s::ExecutionSecurity>,
    job_request: Option<&s::ExecutionSecurity>,
    enabled: &EnabledCodecs,
) -> FormatGrid {
    let base = build_ceiling_grid();
    let after_trusted = match trusted.and_then(|t| t.formats.as_ref()) {
        Some(kb) => kb.apply_to(&base),
        None => base.clone(),
    };
    let after_job = match job_request.and_then(|r| r.formats.as_ref()) {
        Some(kb) => kb.apply_to(&after_trusted),
        None => after_trusted,
    };
    fold_codec_availability_into_format_grid(&after_job, trusted, job_request, enabled)
}

/// After the format-level grid is computed, walk every registered codec
/// and decide if it is available. A format stays "encode: true" iff at
/// least one compiled-in, enabled, not-killbitted encoder maps to it.
/// Same rule for decode.
fn fold_codec_availability_into_format_grid(
    format_grid: &FormatGrid,
    trusted: Option<&s::ExecutionSecurity>,
    job_request: Option<&s::ExecutionSecurity>,
    enabled: &EnabledCodecs,
) -> FormatGrid {
    let mut out = format_grid.clone();
    for &f in ImageFormat::ALL {
        if format_grid.decode(f) {
            let mut any_decoder = false;
            for decoder in enabled.decoders.iter() {
                if decoder.image_format() == f
                    && codec_decoder_allowed(decoder.wire_name(), trusted, job_request)
                {
                    any_decoder = true;
                    break;
                }
            }
            if !any_decoder {
                out.set(f, Op::Decode, false);
            }
        }
        if format_grid.encode(f) {
            let mut any_encoder = false;
            for encoder in enabled.encoders.iter() {
                if encoder.wire_name().image_format() == f
                    && codec_encoder_allowed(encoder.wire_name(), trusted, job_request)
                {
                    any_encoder = true;
                    break;
                }
            }
            if !any_encoder {
                out.set(f, Op::Encode, false);
            }
        }
    }
    out
}

/// Is `codec` permitted for encoding under the combined trusted+job
/// codec killbits? `None` on either layer means "no opinion" on codec
/// selection.
pub fn codec_encoder_allowed(
    codec: NamedEncoderName,
    trusted: Option<&s::ExecutionSecurity>,
    job_request: Option<&s::ExecutionSecurity>,
) -> bool {
    let trusted_ok = match trusted.and_then(|t| t.codecs.as_deref()) {
        Some(kb) => kb.encoder_allowed(codec, true),
        None => true,
    };
    if !trusted_ok {
        return false;
    }
    match job_request.and_then(|r| r.codecs.as_deref()) {
        Some(kb) => kb.encoder_allowed(codec, true),
        None => true,
    }
}

/// Mirror of `codec_encoder_allowed` for decoders.
pub fn codec_decoder_allowed(
    codec: NamedDecoderName,
    trusted: Option<&s::ExecutionSecurity>,
    job_request: Option<&s::ExecutionSecurity>,
) -> bool {
    let trusted_ok = match trusted.and_then(|t| t.codecs.as_deref()) {
        Some(kb) => kb.decoder_allowed(codec, true),
        None => true,
    };
    if !trusted_ok {
        return false;
    }
    match job_request.and_then(|r| r.codecs.as_deref()) {
        Some(kb) => kb.decoder_allowed(codec, true),
        None => true,
    }
}

/// Validate a trusted-policy codec killbits block against the compiled
/// baseline. Returns an error if any `allow_*` entry names a codec
/// that isn't compiled in for this build.
pub fn validate_trusted_codec_killbits(kb: &CodecKillbits) -> Result<()> {
    let baseline = EnabledCodecs::default();
    let compiled_encoders: Vec<NamedEncoderName> =
        baseline.encoders.iter().map(|e| e.wire_name()).collect();
    let compiled_decoders: Vec<NamedDecoderName> =
        baseline.decoders.iter().map(|d| d.wire_name()).collect();

    if let Some(list) = &kb.allow_encoders {
        for &codec in list {
            if !compiled_encoders.contains(&codec) {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "cannot allow encoder {}: not compiled in for this build",
                    codec
                ));
            }
        }
    }
    if let Some(list) = &kb.allow_decoders {
        for &codec in list {
            if !compiled_decoders.contains(&codec) {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "cannot allow decoder {}: not compiled in for this build",
                    codec
                ));
            }
        }
    }
    Ok(())
}

/// Validate a trusted-policy killbits block against the build-time
/// ceiling. Returns an error if any `allow_*` entry names a format
/// that's unavailable at build time.
pub fn validate_trusted_policy(kb: &FormatKillbits) -> Result<()> {
    if let Some(list) = &kb.allow_decode {
        for &f in list {
            if build_killbits::compile_deny_decode_contains(f) {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "cannot allow {}: disabled at build time (decode)",
                    f
                ));
            }
            if !feature_compiled_in(f, Op::Decode) {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "cannot allow {}: disabled at build time (decode)",
                    f
                ));
            }
        }
    }
    if let Some(list) = &kb.allow_encode {
        for &f in list {
            if build_killbits::compile_deny_encode_contains(f) {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "cannot allow {}: disabled at build time (encode)",
                    f
                ));
            }
            if !feature_compiled_in(f, Op::Encode) {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "cannot allow {}: disabled at build time (encode)",
                    f
                ));
            }
        }
    }
    Ok(())
}

/// Structured error returned when a specific-codec `EncoderPreset`
/// (or `EncoderPreset::Auto`/`Format` without any available backend)
/// is rejected by the killbits grid.
///
/// `codec` is the wire name of the required encoder, or `None` when the
/// request resolved to a format but every encoder for that format was
/// killed. `reasons` mirrors the per-codec or per-format `reasons`
/// vector from `v1/context/get_net_support`.
pub fn codec_not_available_error(
    codec: Option<NamedEncoderName>,
    format: ImageFormat,
    reasons: Vec<String>,
    trusted: Option<&s::ExecutionSecurity>,
    job_request: Option<&s::ExecutionSecurity>,
    enabled: &EnabledCodecs,
) -> FlowError {
    let grid = CodecSupportGrid::compute(trusted, job_request, enabled);
    let grid_json = codec_grid_to_json(&grid);
    let codec_field = match codec {
        Some(c) => format!("\"codec\": \"{}\", ", c.as_snake()),
        None => String::new(),
    };
    let reasons_json = serde_json::to_string(&reasons).unwrap_or_else(|_| "[]".to_string());
    let msg = format!(
        "{{\"error\": \"codec_not_available\", {codec_field}\"format\": \"{fmt}\", \"reasons\": {reasons_json}, \"net_support\": {grid_json}}}",
        fmt = format.as_snake()
    );
    FlowError {
        kind: ErrorKind::CodecDisabledError,
        message: msg,
        at: ::smallvec::SmallVec::new(),
        node: None,
    }
}

fn codec_grid_to_json(grid: &CodecSupportGrid) -> String {
    let mut s = String::from("{\"formats\":{");
    let mut first = true;
    for (f, perms) in grid.formats.entries() {
        if !first {
            s.push(',');
        }
        first = false;
        s.push_str(&format!(
            "\"{name}\":{{\"decode\":{d},\"encode\":{e}}}",
            name = f.as_snake(),
            d = perms.decode,
            e = perms.encode
        ));
    }
    s.push_str("},\"codecs\":{");
    let mut first = true;
    for (name, entry) in &grid.codecs {
        if !first {
            s.push(',');
        }
        first = false;
        let reasons_json = serde_json::to_string(&entry.reasons).unwrap_or_else(|_| "[]".into());
        s.push_str(&format!(
            "\"{name}\":{{\"available\":{a},\"format\":\"{fmt}\",\"role\":\"{role}\",\"reasons\":{reasons}}}",
            a = entry.available,
            fmt = entry.format,
            role = entry.role,
            reasons = reasons_json,
        ));
    }
    s.push_str("}}");
    s
}

/// Error kind produced when a decode/encode operation is rejected by
/// the killbits grid. Encoded into `FlowError::message` as JSON that
/// callers can parse.
pub fn denied_error(op: Op, format: ImageFormat, grid: &FormatGrid) -> FlowError {
    let reasons = gather_denial_reasons(op, format);
    let kind = match op {
        Op::Decode => "decode_not_available",
        Op::Encode => "encode_not_available",
    };
    let grid_json = grid_to_json(grid);
    let reasons_json = serde_json::to_string(&reasons).unwrap_or_else(|_| "[]".to_string());
    let msg = format!(
        "{{\"error\": \"{kind}\", \"format\": \"{fmt}\", \"reasons\": {reasons_json}, \"net_support\": {grid_json}}}",
        fmt = format.as_snake()
    );
    FlowError {
        kind: ErrorKind::CodecDisabledError,
        message: msg,
        at: ::smallvec::SmallVec::new(),
        node: None,
    }
}

fn gather_denial_reasons(op: Op, format: ImageFormat) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    match op {
        Op::Decode => {
            if build_killbits::compile_deny_decode_contains(format) {
                reasons.push("compile.deny_decode");
            }
            if !feature_compiled_in(format, Op::Decode) {
                reasons.push("compile.feature_missing");
            }
        }
        Op::Encode => {
            if build_killbits::compile_deny_encode_contains(format) {
                reasons.push("compile.deny_encode");
            }
            if !feature_compiled_in(format, Op::Encode) {
                reasons.push("compile.feature_missing");
            }
        }
    }
    // The trusted/job layers are reflected in the final `net_support`
    // grid; callers can inspect that to see which layer denied them.
    // We report "trusted_policy.deny" as a general reason when the
    // build-time layer allows it but the net grid doesn't.
    if reasons.is_empty() {
        reasons.push("trusted_policy_or_job_deny");
    }
    reasons
}

fn grid_to_json(grid: &FormatGrid) -> String {
    let mut s = String::from("{\"formats\":{");
    let mut first = true;
    for (f, perms) in grid.entries() {
        if !first {
            s.push(',');
        }
        first = false;
        s.push_str(&format!(
            "\"{name}\":{{\"decode\":{d},\"encode\":{e}}}",
            name = f.as_snake(),
            d = perms.decode,
            e = perms.encode
        ));
    }
    s.push_str("}}");
    s
}

/// Ensure `op` on `format` is permitted by the combined grid.
/// Returns a structured error if not.
pub fn enforce(grid: &FormatGrid, op: Op, format: ImageFormat) -> Result<()> {
    if grid.get(format, op) {
        Ok(())
    } else {
        Err(denied_error(op, format, grid))
    }
}

// -- Wire types for the v1/context/* endpoints --

/// Per-format `{decode, encode}` pair serialized in net_support responses.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct FormatSupport {
    pub decode: bool,
    pub encode: bool,
}

/// Wire-level grid: `{ formats: { "jpeg": {decode, encode}, ... } }`.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct NetSupport {
    pub formats: std::collections::BTreeMap<String, FormatSupport>,
}

impl NetSupport {
    pub fn from_grid(grid: &FormatGrid) -> Self {
        let mut formats = std::collections::BTreeMap::new();
        for (f, perms) in grid.entries() {
            formats.insert(
                f.as_snake().to_string(),
                FormatSupport { decode: perms.decode, encode: perms.encode },
            );
        }
        NetSupport { formats }
    }
}

/// Compile-time ceiling summary for `v1/context/get_net_support`.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct CompileCeiling {
    pub denied_decode: Vec<String>,
    pub denied_encode: Vec<String>,
    pub features_missing: Vec<String>,
}

impl CompileCeiling {
    /// Clone the process-wide cached ceiling. Prefer [`compile_ceiling`]
    /// when a `&'static CompileCeiling` is acceptable — it avoids the
    /// allocation and `String` clone this entry point pays.
    pub fn current() -> Self {
        compile_ceiling().clone()
    }
}

/// Response payload from `Context::set_trusted_policy` (exposed by
/// `v1/context/set_policy`).
pub struct LockedPolicyReport {
    pub ok: bool,
    pub locked: bool,
    pub net_support: FormatGrid,
}

/// Per-codec availability row reported by `v1/context/get_net_support`.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct CodecSupport {
    pub available: bool,
    pub format: String,
    /// "encode" or "decode".
    pub role: String,
    /// When `available == false`, a list of machine-readable reasons
    /// (e.g. `"codec_killbits.deny_encoders"`, `"format_denied"`,
    /// `"feature_missing"`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reasons: Vec<String>,
}

/// Full codec-level availability map + format-level grid combined, in
/// the shape returned by `v1/context/get_net_support`.
#[derive(Clone, PartialEq, Debug)]
pub struct CodecSupportGrid {
    /// Format grid (after codec-availability fold-in). Format entries
    /// also get a `reasons` vector populated when codec-level kill
    /// flipped a previously-true cell off.
    pub formats: FormatGrid,
    /// Per-format fold-down reasons: a format ends up denied because
    /// every eligible codec was denied, feature-missing, etc.
    pub format_reasons: std::collections::BTreeMap<ImageFormat, FormatReasons>,
    /// Per-codec map keyed by the wire name (`mozjpeg_encoder`, etc.).
    pub codecs: std::collections::BTreeMap<String, CodecSupport>,
}

/// Machine-readable reasons attached to a format cell when codec-level
/// kills cause a previously-available format to flip off.
#[derive(Default, Clone, PartialEq, Debug)]
pub struct FormatReasons {
    pub decode: Vec<String>,
    pub encode: Vec<String>,
}

impl CodecSupportGrid {
    pub fn compute(
        trusted: Option<&s::ExecutionSecurity>,
        job_request: Option<&s::ExecutionSecurity>,
        enabled: &EnabledCodecs,
    ) -> Self {
        let base = build_ceiling_grid();
        let after_trusted = match trusted.and_then(|t| t.formats.as_ref()) {
            Some(kb) => kb.apply_to(&base),
            None => base.clone(),
        };
        let format_grid_before_codecs = match job_request.and_then(|r| r.formats.as_ref()) {
            Some(kb) => kb.apply_to(&after_trusted),
            None => after_trusted,
        };

        // Per-codec rows + format-level rollup.
        let mut codecs = std::collections::BTreeMap::new();
        let mut any_encoder_for_format: std::collections::BTreeMap<ImageFormat, bool> =
            std::collections::BTreeMap::new();
        let mut any_decoder_for_format: std::collections::BTreeMap<ImageFormat, bool> =
            std::collections::BTreeMap::new();
        for &f in ImageFormat::ALL {
            any_encoder_for_format.insert(f, false);
            any_decoder_for_format.insert(f, false);
        }

        for encoder in enabled.encoders.iter() {
            let name = encoder.wire_name();
            let fmt = name.image_format();
            let format_allowed = format_grid_before_codecs.encode(fmt);
            let killbits_allowed = codec_encoder_allowed(name, trusted, job_request);
            let available = format_allowed && killbits_allowed;
            let mut reasons = Vec::new();
            if !format_allowed {
                reasons.push("format_denied".to_string());
            }
            if !killbits_allowed {
                reasons.push("codec_killbits.deny_encoders".to_string());
            }
            if available {
                *any_encoder_for_format.entry(fmt).or_default() = true;
            }
            codecs.insert(
                name.as_snake().to_string(),
                CodecSupport {
                    available,
                    format: fmt.as_snake().to_string(),
                    role: "encode".to_string(),
                    reasons,
                },
            );
        }
        for decoder in enabled.decoders.iter() {
            let name = decoder.wire_name();
            let fmt = name.image_format();
            let format_allowed = format_grid_before_codecs.decode(fmt);
            let killbits_allowed = codec_decoder_allowed(name, trusted, job_request);
            let available = format_allowed && killbits_allowed;
            let mut reasons = Vec::new();
            if !format_allowed {
                reasons.push("format_denied".to_string());
            }
            if !killbits_allowed {
                reasons.push("codec_killbits.deny_decoders".to_string());
            }
            if available {
                *any_decoder_for_format.entry(fmt).or_default() = true;
            }
            codecs.insert(
                name.as_snake().to_string(),
                CodecSupport {
                    available,
                    format: fmt.as_snake().to_string(),
                    role: "decode".to_string(),
                    reasons,
                },
            );
        }

        // Fold codec availability back into the format grid.
        let mut formats = format_grid_before_codecs.clone();
        let mut format_reasons: std::collections::BTreeMap<ImageFormat, FormatReasons> =
            std::collections::BTreeMap::new();
        for &f in ImageFormat::ALL {
            let mut rr = FormatReasons::default();
            if format_grid_before_codecs.decode(f) && !any_decoder_for_format[&f] {
                formats.set(f, Op::Decode, false);
                rr.decode.push("no_available_decoder".to_string());
            }
            if format_grid_before_codecs.encode(f) && !any_encoder_for_format[&f] {
                formats.set(f, Op::Encode, false);
                rr.encode.push("no_available_encoder".to_string());
            }
            if !rr.decode.is_empty() || !rr.encode.is_empty() {
                format_reasons.insert(f, rr);
            }
        }

        CodecSupportGrid { formats, format_reasons, codecs }
    }
}

/// View wrapper over a grid for return from `Context::net_support`.
/// Lets callers convert to JSON / query individual cells.
pub struct FormatGridView {
    pub grid: FormatGrid,
}

impl FormatGridView {
    pub fn decode(&self, f: ImageFormat) -> bool {
        self.grid.decode(f)
    }
    pub fn encode(&self, f: ImageFormat) -> bool {
        self.grid.encode(f)
    }
    pub fn grid(&self) -> &FormatGrid {
        &self.grid
    }
}

/// Intersect two `ExecutionSecurity` blocks — narrow both the scalar
/// limits and the `formats` killbits. Result is always a valid
/// job-level form.
pub(crate) fn intersect_security(
    trusted: &s::ExecutionSecurity,
    job: &s::ExecutionSecurity,
) -> s::ExecutionSecurity {
    // Scalar limits: pick the more restrictive of the two when both are set.
    let max_decode_size = min_optional_frame(&trusted.max_decode_size, &job.max_decode_size);
    let max_frame_size = min_optional_frame(&trusted.max_frame_size, &job.max_frame_size);
    let max_encode_size = min_optional_frame(&trusted.max_encode_size, &job.max_encode_size);
    let max_input_file_bytes = min_optional(trusted.max_input_file_bytes, job.max_input_file_bytes);
    let max_json_bytes = min_optional(trusted.max_json_bytes, job.max_json_bytes);
    let max_total_file_pixels =
        min_optional(trusted.max_total_file_pixels, job.max_total_file_pixels);

    // Killbits: apply both layers over an all-allowed grid, emit a table form.
    let formats = match (&trusted.formats, &job.formats) {
        (None, None) => None,
        (Some(t), None) => Some(t.clone()),
        (None, Some(j)) => Some(j.clone()),
        (Some(t), Some(j)) => Some(Box::new(FormatKillbits::intersect(t, j))),
    };

    // Codec-level killbits: intersect. Either layer absent preserves the
    // other; both present produces a union of their deny lists (see
    // `CodecKillbits::intersect`).
    let codecs = match (&trusted.codecs, &job.codecs) {
        (None, None) => None,
        (Some(t), None) => Some(t.clone()),
        (None, Some(j)) => Some(j.clone()),
        (Some(t), Some(j)) => Some(Box::new(s::CodecKillbits::intersect(t, j))),
    };

    // ExecutionSecurity is `#[non_exhaustive]`, so start from `unspecified()`
    // and mutate fields in-place.
    let mut out = s::ExecutionSecurity::unspecified();
    out.max_decode_size = max_decode_size;
    out.max_frame_size = max_frame_size;
    out.max_encode_size = max_encode_size;
    out.max_input_file_bytes = max_input_file_bytes;
    out.max_json_bytes = max_json_bytes;
    out.max_total_file_pixels = max_total_file_pixels;
    out.formats = formats;
    out.codecs = codecs;
    out
}

fn min_optional<T: Ord + Copy>(a: Option<T>, b: Option<T>) -> Option<T> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (Some(x), None) => Some(x),
        (None, Some(y)) => Some(y),
        (None, None) => None,
    }
}

fn min_optional_frame(
    a: &Option<s::FrameSizeLimit>,
    b: &Option<s::FrameSizeLimit>,
) -> Option<s::FrameSizeLimit> {
    match (a, b) {
        (Some(x), Some(y)) => Some(s::FrameSizeLimit {
            w: x.w.min(y.w),
            h: x.h.min(y.h),
            megapixels: x.megapixels.min(y.megapixels),
        }),
        (Some(x), None) => Some(*x),
        (None, Some(y)) => Some(*y),
        (None, None) => None,
    }
}

/// Check that `new_policy` doesn't widen any dimension of `existing`.
/// Returns an error naming the first widening found.
pub(crate) fn ensure_narrowing(
    existing: &s::ExecutionSecurity,
    new_policy: &s::ExecutionSecurity,
) -> Result<()> {
    // Scalar limits: new must be <= existing (or same). `None` in existing
    // means "no limit" — new can tighten; `None` in new means "no change
    // requested", which is fine.
    fn check_scalar<T: Ord + std::fmt::Display + Copy>(
        name: &str,
        existing: Option<T>,
        new_val: Option<T>,
    ) -> Result<()> {
        if let (Some(old), Some(new)) = (existing, new_val) {
            if new > old {
                return Err(nerror!(
                    ErrorKind::InvalidArgument,
                    "trusted policy cannot widen {}: existing={}, new={}",
                    name,
                    old,
                    new
                ));
            }
        } else if existing.is_some() && new_val.is_none() {
            // new_val = None would remove the limit, which is a widen.
            // Accept only when we're keeping it equal.
            // Re-setting without that scalar is OK (we preserve the old via
            // `apply_security` skip-on-None), so don't error here.
        }
        Ok(())
    }
    check_scalar(
        "max_input_file_bytes",
        existing.max_input_file_bytes,
        new_policy.max_input_file_bytes,
    )?;
    check_scalar("max_json_bytes", existing.max_json_bytes, new_policy.max_json_bytes)?;
    check_scalar(
        "max_total_file_pixels",
        existing.max_total_file_pixels,
        new_policy.max_total_file_pixels,
    )?;
    // Frame sizes: compare each component. None on new = no change requested.
    check_frame("max_decode_size", &existing.max_decode_size, &new_policy.max_decode_size)?;
    check_frame("max_frame_size", &existing.max_frame_size, &new_policy.max_frame_size)?;
    check_frame("max_encode_size", &existing.max_encode_size, &new_policy.max_encode_size)?;

    // Killbits: every cell allowed in `new` must also be allowed in `existing`.
    let existing_grid = match &existing.formats {
        Some(kb) => kb.apply_to(&build_ceiling_grid()),
        None => build_ceiling_grid(),
    };
    let new_grid = match &new_policy.formats {
        Some(kb) => kb.apply_to(&build_ceiling_grid()),
        None => build_ceiling_grid(),
    };
    for &f in ImageFormat::ALL {
        if new_grid.decode(f) && !existing_grid.decode(f) {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "trusted policy cannot widen: {} decode was denied, new policy would allow",
                f
            ));
        }
        if new_grid.encode(f) && !existing_grid.encode(f) {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "trusted policy cannot widen: {} encode was denied, new policy would allow",
                f
            ));
        }
    }
    Ok(())
}

fn check_frame(
    name: &str,
    existing: &Option<s::FrameSizeLimit>,
    new_val: &Option<s::FrameSizeLimit>,
) -> Result<()> {
    if let (Some(old), Some(new)) = (existing, new_val) {
        if new.w > old.w || new.h > old.h || new.megapixels > old.megapixels {
            return Err(nerror!(
                ErrorKind::InvalidArgument,
                "trusted policy cannot widen {}: existing w={},h={},mp={} vs new w={},h={},mp={}",
                name,
                old.w,
                old.h,
                old.megapixels,
                new.w,
                new.h,
                new.megapixels
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use imageflow_types::killbits::{FormatKillbits, ImageFormat as If, Op};

    #[test]
    fn compile_ceiling_upstream_is_empty() {
        let ceiling = CompileCeiling::current();
        assert!(ceiling.denied_decode.is_empty(), "upstream: no compile deny decode");
        assert!(ceiling.denied_encode.is_empty(), "upstream: no compile deny encode");
        // `features_missing` may be non-empty (heic/tiff/pnm have no backend yet).
    }

    #[test]
    fn build_ceiling_grid_reflects_features() {
        let grid = build_ceiling_grid();
        // PNG/GIF are always available.
        assert!(grid.decode(If::Png));
        assert!(grid.encode(If::Png));
        assert!(grid.decode(If::Gif));
        assert!(grid.encode(If::Gif));
        // Heic/Tiff/Pnm are not implemented yet.
        assert!(!grid.decode(If::Heic));
        assert!(!grid.encode(If::Heic));
        assert!(!grid.decode(If::Tiff));
        assert!(!grid.decode(If::Pnm));
    }

    #[test]
    fn compile_ceiling_allow_unavailable_format_errors() {
        // Attempting to allow Heic via trusted policy should fail because no
        // backend is compiled in.
        let kb = FormatKillbits {
            allow_decode: Some(vec![If::Heic]),
            ..Default::default()
        };
        let result = validate_trusted_policy(&kb);
        assert!(result.is_err(), "expected error for allow_decode: [heic]");
    }

    #[test]
    fn enforce_denies_when_grid_says_no() {
        let grid = {
            // Deny avif encode explicitly.
            let mut g = build_ceiling_grid();
            g.set(If::Avif, Op::Encode, false);
            g
        };
        let err = enforce(&grid, Op::Encode, If::Avif).unwrap_err();
        // The FlowError message is JSON containing "encode_not_available".
        assert!(err.message.contains("encode_not_available"), "got: {}", err.message);
        assert!(err.message.contains("avif"), "got: {}", err.message);
    }

    #[test]
    fn intersect_security_narrows_scalar_limits() {
        let mut trusted = s::ExecutionSecurity::unspecified();
        trusted.max_json_bytes = Some(10_000_000);
        trusted.max_input_file_bytes = Some(50_000_000);
        let mut job = s::ExecutionSecurity::unspecified();
        job.max_json_bytes = Some(5_000_000); // tighter
        job.max_input_file_bytes = Some(100_000_000); // looser — trusted wins

        let out = intersect_security(&trusted, &job);
        assert_eq!(out.max_json_bytes, Some(5_000_000));
        assert_eq!(out.max_input_file_bytes, Some(50_000_000));
    }

    #[test]
    fn intersect_security_combines_killbits() {
        let mut trusted = s::ExecutionSecurity::unspecified();
        trusted.formats = Some(Box::new(FormatKillbits {
            deny_encode: Some(vec![If::Avif]),
            ..Default::default()
        }));
        let mut job = s::ExecutionSecurity::unspecified();
        job.formats = Some(Box::new(FormatKillbits {
            deny_encode: Some(vec![If::Webp]),
            ..Default::default()
        }));
        let out = intersect_security(&trusted, &job);
        let kb = out.formats.expect("formats set");
        let grid = kb.apply_to(&imageflow_types::killbits::FormatGrid::all_allowed());
        assert!(!grid.encode(If::Avif));
        assert!(!grid.encode(If::Webp));
        assert!(grid.encode(If::Jpeg));
    }

    #[test]
    fn ensure_narrowing_accepts_same_scalars() {
        let mut existing = s::ExecutionSecurity::unspecified();
        existing.max_json_bytes = Some(10_000_000);
        let mut new_policy = s::ExecutionSecurity::unspecified();
        new_policy.max_json_bytes = Some(5_000_000);
        assert!(ensure_narrowing(&existing, &new_policy).is_ok());
    }

    #[test]
    fn ensure_narrowing_rejects_widening_scalar() {
        let mut existing = s::ExecutionSecurity::unspecified();
        existing.max_json_bytes = Some(5_000_000);
        let mut new_policy = s::ExecutionSecurity::unspecified();
        new_policy.max_json_bytes = Some(10_000_000);
        let err = ensure_narrowing(&existing, &new_policy).unwrap_err();
        assert!(err.message.contains("cannot widen"), "got: {}", err.message);
    }

    #[test]
    fn ensure_narrowing_rejects_widening_formats() {
        // Existing denies PNG encode; new policy doesn't. That's a widen.
        // (PNG is always compiled in so the build-time ceiling allows it,
        // which makes the delta between the two policies visible to the
        // narrowing check.)
        let mut existing = s::ExecutionSecurity::unspecified();
        existing.formats = Some(Box::new(FormatKillbits {
            deny_encode: Some(vec![If::Png]),
            ..Default::default()
        }));
        let new_policy = s::ExecutionSecurity::unspecified();
        let err = ensure_narrowing(&existing, &new_policy).unwrap_err();
        assert!(err.message.contains("widen"), "got: {}", err.message);
    }

    #[test]
    fn net_support_with_no_policy_is_build_ceiling() {
        let grid = compute_net_support(None, None);
        let base = build_ceiling_grid();
        for &f in If::ALL {
            assert_eq!(grid.decode(f), base.decode(f));
            assert_eq!(grid.encode(f), base.encode(f));
        }
    }

    #[test]
    fn net_support_with_trusted_denies() {
        let mut trusted = s::ExecutionSecurity::unspecified();
        trusted.formats = Some(Box::new(FormatKillbits {
            deny_encode: Some(vec![If::Avif]),
            ..Default::default()
        }));
        let grid = compute_net_support(Some(&trusted), None);
        assert!(!grid.encode(If::Avif));
        // Decode is still allowed.
        if build_ceiling_grid().decode(If::Avif) {
            assert!(grid.decode(If::Avif));
        }
    }

    #[test]
    fn enabled_codec_wire_names_are_distinct_and_parseable() {
        // Every NamedEncoders / NamedDecoders variant compiled into
        // this build must map to a distinct wire name and that wire
        // name must be a valid NamedEncoderName / NamedDecoderName
        // (i.e. present in ::ALL). Catches copy/paste regressions
        // between the core and types-side enums.
        use imageflow_types::{NamedDecoderName, NamedEncoderName};
        let baseline = crate::codecs::EnabledCodecs::default();
        let mut seen_enc = std::collections::BTreeSet::new();
        for enc in baseline.encoders.iter() {
            let wire = enc.wire_name();
            assert!(NamedEncoderName::ALL.contains(&wire), "not in ALL: {:?}", wire);
            assert!(seen_enc.insert(wire.as_snake()), "duplicate wire_name: {:?}", wire);
        }
        let mut seen_dec = std::collections::BTreeSet::new();
        for dec in baseline.decoders.iter() {
            let wire = dec.wire_name();
            assert!(NamedDecoderName::ALL.contains(&wire), "not in ALL: {:?}", wire);
            assert!(seen_dec.insert(wire.as_snake()), "duplicate wire_name: {:?}", wire);
        }
    }

    #[test]
    fn net_support_with_job_narrows_further() {
        let mut trusted = s::ExecutionSecurity::unspecified();
        trusted.formats = Some(Box::new(FormatKillbits {
            deny_encode: Some(vec![If::Avif]),
            ..Default::default()
        }));
        let mut job = s::ExecutionSecurity::unspecified();
        job.formats = Some(Box::new(FormatKillbits {
            deny_encode: Some(vec![If::Jpeg]),
            ..Default::default()
        }));
        let grid = compute_net_support(Some(&trusted), Some(&job));
        assert!(!grid.encode(If::Avif));
        assert!(!grid.encode(If::Jpeg));
        // PNG encode still allowed.
        assert!(grid.encode(If::Png));
    }
}
