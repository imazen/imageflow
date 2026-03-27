//! Top-level execution: v2 Framewise → zenpipe streaming pipeline → encoded output.
//!
//! Handles two Framewise modes:
//! - **Steps**: Linear `Vec<Node>` — sequential pipeline.
//! - **Graph**: DAG with explicit edges — compositing, fan-out, watermarks.
//!
//! Fully streaming: decode (row batches) → process (strips) → encode (push_rows).
//! No full-frame materialization for JPEG/PNG. Formats that don't support
//! streaming decode (WebP, TIFF) fall back to full-frame + MaterializedSource.

use std::collections::HashMap;

use imageflow_types::{self as s, Framewise, Node};
use zencodecs::{AllowedFormats, CodecPolicy, ImageFacts, select_format_from_intent};
use zennode::NodeDef as _;
use zenpipe::Source as _;

use super::translate::{self, TranslateError, TranslatedPipeline};

// ─── Tracing ───

/// Build a pipeline with optional tracing.
///
/// When `ZENPIPE_TRACE=1` (or `ZENPIPE_TRACE=full`), enables tracing and
/// prints the pipeline trace to stderr after compilation. When
/// `ZENPIPE_TRACE=svg`, also writes an SVG to `/tmp/zenpipe_trace.svg`.
///
/// Zero cost when the env var is unset — `build_pipeline` is called directly.
fn build_pipeline_maybe_traced(
    source: Box<dyn zenpipe::Source>,
    nodes: &[Box<dyn zennode::NodeInstance>],
    converters: &[&dyn zenpipe::bridge::NodeConverter],
) -> Result<zenpipe::PipelineResult, zenpipe::PipeError> {
    let trace_mode = std::env::var("ZENPIPE_TRACE").unwrap_or_default();
    if trace_mode.is_empty() {
        return zenpipe::bridge::build_pipeline(source, nodes, converters);
    }

    let config = if trace_mode == "full" || trace_mode == "svg" {
        zenpipe::trace::TraceConfig::full()
    } else {
        zenpipe::trace::TraceConfig::metadata_only()
    };

    // Build origin annotations from node schema IDs.
    let origins: Vec<(String, String)> = nodes
        .iter()
        .map(|n| {
            let schema_id = n.schema().id;
            let origin = match schema_id {
                "imageflow.resample2d" => "Ir4Expand:Resample2D",
                "imageflow.crop_whitespace" => "Ir4Expand:CropWhitespace",
                "imageflow.fill_rect" => "Ir4Expand:FillRect",
                "imageflow.round_corners" => "Ir4Expand:RoundImageCorners",
                "imageflow.remove_alpha" => "Ir4Expand:RemoveAlpha",
                id if id.starts_with("zenlayout.") => "Ir4Expand → translate.rs",
                id if id.starts_with("zenresize.") => "Ir4Expand → translate.rs",
                id if id.starts_with("zenfilters.") => "Ir4Expand:ColorFilterSrgb",
                _ => "translate.rs",
            };
            (schema_id.to_string(), origin.to_string())
        })
        .collect();

    let (result, mut trace) =
        zenpipe::bridge::build_pipeline_traced(source, nodes, converters, &config)?;

    // Annotate graph entries with origin info from the node list.
    // Match by node name (schema_id maps to NodeOp name).
    for entry in &mut trace.graph.entries {
        if entry.origin.is_none() && !entry.implicit {
            // Find matching origin by name heuristic.
            let origin = origins.iter().find(|(schema, _)| {
                schema.ends_with(&format!(".{}", entry.name.to_lowercase()))
                    || (entry.name == "Resize" && schema == "imageflow.resample2d")
                    || (entry.name == "Constrain" && schema.contains("constrain"))
                    || (entry.name == "AutoOrient" && schema.contains("orient"))
                    || (entry.name == "Crop" && schema.contains("crop"))
            });
            if let Some((_, orig)) = origin {
                entry.origin = Some(orig.clone());
            }
        }
    }

    // Print full trace to stderr.
    eprintln!("{}", trace.to_text());

    // Write SVG if requested.
    if trace_mode == "svg" {
        let svg = trace.graph.to_svg();
        let _ = std::fs::write("/tmp/zenpipe_trace.svg", &svg);
        eprintln!("[trace] SVG written to /tmp/zenpipe_trace.svg");
    }

    Ok(result)
}

// ─── Error type ───

/// Error from the zen pipeline execution.
#[derive(Debug)]
pub enum ZenError {
    Translate(TranslateError),
    Codec(String),
    Pipeline(zenpipe::PipeError),
    Io(String),
    SizeLimit(String),
}

impl std::fmt::Display for ZenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Translate(e) => write!(f, "translate: {e}"),
            Self::Codec(e) => write!(f, "codec: {e}"),
            Self::Pipeline(e) => write!(f, "pipeline: {e}"),
            Self::Io(e) => write!(f, "io: {e}"),
            Self::SizeLimit(e) => write!(f, "SizeLimitExceeded: {e}"),
        }
    }
}

impl std::error::Error for ZenError {}

impl From<TranslateError> for ZenError {
    fn from(e: TranslateError) -> Self { Self::Translate(e) }
}

impl From<zenpipe::PipeError> for ZenError {
    fn from(e: zenpipe::PipeError) -> Self { Self::Pipeline(e) }
}

// ─── Result type ───

/// Output from a single encode operation.
pub struct ZenEncodeResult {
    pub io_id: i32,
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub mime_type: &'static str,
    pub extension: &'static str,
}

// ─── Public API ───

/// Probe an image and return metadata without decoding pixels.
pub fn zen_get_image_info(
    data: &[u8],
) -> Result<zencodecs::ImageInfo, ZenError> {
    zencodecs::from_bytes(data).map_err(|e| ZenError::Codec(format!("{e}")))
}

/// Result of executing a framewise pipeline.
pub struct ExecuteResult {
    pub encode_results: Vec<ZenEncodeResult>,
    pub captured_dimensions: CapturedBitmaps,
    /// Source image info from probe, keyed by decode io_id.
    pub decode_infos: Vec<(i32, zencodecs::ImageInfo)>,
}

/// Execute a v2 Framewise pipeline through the zen streaming engine.
///
/// `io_buffers` maps io_id → input bytes. Returns encode results and
/// any dimensions captured by `CaptureBitmapKey` nodes.
pub fn execute_framewise(
    framewise: &Framewise,
    io_buffers: &HashMap<i32, Vec<u8>>,
    security: &imageflow_types::ExecutionSecurity,
) -> Result<ExecuteResult, ZenError> {
    // Collect decode info from probing input buffers.
    let decode_infos = collect_decode_infos(framewise, io_buffers);

    // Check decode dimensions against security limits.
    for (io_id, info) in &decode_infos {
        check_security_limit(info.width, info.height, &security.max_decode_size, "decode")?;
    }

    match framewise {
        Framewise::Steps(steps) => {
            let (results, captures) = execute_steps(steps, io_buffers, security)?;
            Ok(ExecuteResult { encode_results: results, captured_dimensions: captures, decode_infos })
        }
        Framewise::Graph(graph) => {
            let results = execute_graph(graph, io_buffers)?;
            Ok(ExecuteResult {
                encode_results: results,
                captured_dimensions: CapturedBitmaps { captures: HashMap::new() },
                decode_infos,
            })
        }
    }
}

// ─── Steps mode (linear pipeline) ───

use super::captured::CapturedBitmap;

/// Bitmaps captured by CaptureBitmapKey nodes.
pub(crate) struct CapturedBitmaps {
    pub captures: HashMap<i32, CapturedBitmap>,
}

fn execute_steps(
    steps: &[Node],
    io_buffers: &HashMap<i32, Vec<u8>>,
    security: &imageflow_types::ExecutionSecurity,
) -> Result<(Vec<ZenEncodeResult>, CapturedBitmaps), ZenError> {
    // Pre-process: expand CommandString nodes using source dimensions from probe.
    let steps = expand_command_strings(steps, io_buffers)?;

    // Collect capture IDs before translation (CaptureBitmapKey is a no-op in translate).
    let capture_ids: Vec<i32> = steps.iter().filter_map(|n| match n {
        Node::CaptureBitmapKey { capture_id } => Some(*capture_id),
        _ => None,
    }).collect();

    let pipeline = translate::translate_nodes(&steps)?;
    let has_encode = pipeline.encode_io_id.is_some();

    // Handle CreateCanvas — create solid-color source instead of decoding.
    if let Some(ref canvas) = pipeline.create_canvas {
        check_security_limit(canvas.w, canvas.h, &security.max_frame_size, "max_frame_size")?;
        let source = create_canvas_source(canvas)?;
        let source = ensure_srgb_rgba8(source)?;

        let converters = super::converter::imageflow_converters();
        let converters: &[&dyn zenpipe::bridge::NodeConverter] = &converters;
        let pipe_result = build_pipeline_maybe_traced(source, &pipeline.nodes, converters)?;

        let out_w = pipe_result.source.width();
        let out_h = pipe_result.source.height();

        let mut captures = HashMap::new();
        if has_encode && capture_ids.is_empty() {
            let encode_io_id = pipeline.encode_io_id.unwrap();
            // Use the encoder preset's format, falling back to PNG.
            let codec_intent = pipeline.preset.as_ref()
                .map(|p| &p.intent)
                .cloned()
                .unwrap_or_default();
            let canvas_facts = zencodecs::ImageFacts {
                has_alpha: true,
                pixel_count: out_w as u64 * out_h as u64,
                ..Default::default()
            };
            let decision = zencodecs::select_format_from_intent(
                &codec_intent, &canvas_facts,
                &AllowedFormats::all(), &zencodecs::CodecPolicy::default(),
            ).unwrap_or_else(|_| zencodecs::FormatDecision::for_format(zencodecs::ImageFormat::Png));
            let results = stream_encode(pipe_result.source, &decision, encode_io_id)?;
            return Ok((results, CapturedBitmaps { captures }));
        } else {
            let materialized = pipe_result.materialize()?;
            let w = materialized.pixels.width();
            let h = materialized.pixels.height();
            let fmt = materialized.pixels.format();
            let data = materialized.pixels.data().to_vec();
            for id in &capture_ids {
                captures.insert(*id, CapturedBitmap {
                    width: w, height: h, pixels: data.clone(), format: fmt,
                });
            }
            if has_encode {
                let encode_io_id = pipeline.encode_io_id.unwrap();
                let stride = w as usize * fmt.bytes_per_pixel();
                let ps = zenpixels::PixelSlice::new(&data, w, h, stride, fmt)
                    .map_err(|e| ZenError::Codec(format!("pixel slice: {e}")))?;
                let registry = AllowedFormats::all();
                let decision = zencodecs::FormatDecision::for_format(zencodecs::ImageFormat::Png);
                let output = zencodecs::EncodeRequest::new(decision.format)
                    .with_registry(&registry)
                    .encode(ps, fmt.has_alpha())
                    .map_err(|e| ZenError::Codec(format!("encode: {e}")))?;
                return Ok((vec![ZenEncodeResult {
                    io_id: encode_io_id, bytes: output.into_vec(),
                    width: w, height: h,
                    mime_type: decision.format.mime_type(),
                    extension: decision.format.extension(),
                }], CapturedBitmaps { captures }));
            }
            return Ok((Vec::new(), CapturedBitmaps { captures }));
        }
    }

    let decode_io_id = pipeline.decode_io_id.ok_or_else(|| {
        ZenError::Io("no decode node in pipeline".into())
    })?;
    let input_data = io_buffers.get(&decode_io_id).ok_or_else(|| {
        ZenError::Io(format!("no input buffer for io_id {decode_io_id}"))
    })?;

    // Check for animation: if input is animated and encode format supports animation,
    // do a multi-frame passthrough (decode all → encode all).
    // Skip when SelectFrame is set — that means single-frame extraction, not animation.
    let has_select_frame = pipeline.decoder_commands.as_ref()
        .is_some_and(|cmds| cmds.iter().any(|c| matches!(c, imageflow_types::DecoderCommand::SelectFrame(_))));
    if has_encode && pipeline.nodes.is_empty() && !has_select_frame {
        let registry = AllowedFormats::all();
        let info = zencodecs::from_bytes(input_data)
            .map_err(|e| ZenError::Codec(format!("probe: {e}")))?;
        if info.is_animation() {
            let encode_io_id = pipeline.encode_io_id.unwrap();
            let codec_intent = pipeline.preset.as_ref()
                .map(|p| &p.intent).cloned().unwrap_or_default();
            let decision = select_format_from_intent(
                &codec_intent, &ImageFacts::from_image_info(&info),
                &registry, &CodecPolicy::default(),
            ).map_err(|e| ZenError::Codec(format!("format: {e}")))?;

            if let Ok(result) = encode_animation_passthrough(input_data, &registry, &decision, encode_io_id) {
                return Ok((result, CapturedBitmaps { captures: HashMap::new() }));
            }
        }
    }

    let (decision, source) = probe_resolve_decode(input_data, &pipeline, &pipeline.decoder_commands, security.cms_mode)?;

    let converters = super::converter::imageflow_converters();
    let converters: &[&dyn zenpipe::bridge::NodeConverter] = &converters;
    let pipe_result = build_pipeline_maybe_traced(source, &pipeline.nodes, converters)?;

    // Check encode dimensions against security limits.
    if has_encode {
        let out_w = pipe_result.source.width();
        let out_h = pipe_result.source.height();
        check_security_limit(out_w, out_h, &security.max_encode_size, "max_encode_size")?;
        check_security_limit(out_w, out_h, &security.max_frame_size, "max_frame_size")?;
    }

    let mut captures = HashMap::new();

    let results = if has_encode && capture_ids.is_empty() {
        // Standard path: stream directly to encoder.
        let encode_io_id = pipeline.encode_io_id.unwrap();
        stream_encode(pipe_result.source, &decision, encode_io_id)?
    } else if has_encode {
        // Has both encode and capture: materialize, capture, then one-shot encode.
        let materialized = pipe_result.materialize()?;
        let w = materialized.pixels.width();
        let h = materialized.pixels.height();
        let fmt = materialized.pixels.format();
        let data = materialized.pixels.data().to_vec();

        for id in &capture_ids {
            captures.insert(*id, CapturedBitmap {
                width: w, height: h, pixels: data.clone(), format: fmt,
            });
        }

        // One-shot encode from materialized data.
        let stride = w as usize * fmt.bytes_per_pixel();
        let pixel_slice = zenpixels::PixelSlice::new(&data, w, h, stride, fmt)
            .map_err(|e| ZenError::Codec(format!("pixel slice: {e}")))?;

        let encode_io_id = pipeline.encode_io_id.unwrap();
        let registry = AllowedFormats::all();
        let output = zencodecs::EncodeRequest::new(decision.format)
            .with_quality(decision.quality.quality)
            .with_lossless(decision.lossless)
            .with_registry(&registry)
            .encode(pixel_slice, fmt.has_alpha())
            .map_err(|e| ZenError::Codec(format!("encode: {e}")))?;

        vec![ZenEncodeResult {
            io_id: encode_io_id,
            bytes: output.into_vec(),
            width: w, height: h,
            mime_type: decision.format.mime_type(),
            extension: decision.format.extension(),
        }]
    } else {
        // No encode — materialize for capture only.
        let materialized = pipe_result.materialize()?;
        let w = materialized.pixels.width();
        let h = materialized.pixels.height();
        let fmt = materialized.pixels.format();
        let data = materialized.pixels.data().to_vec();

        for id in &capture_ids {
            captures.insert(*id, CapturedBitmap {
                width: w, height: h, pixels: data.clone(), format: fmt,
            });
        }
        Vec::new()
    };

    Ok((results, CapturedBitmaps { captures }))
}

// ─── Graph mode (DAG with compositing, fan-out) ───

fn execute_graph(
    graph: &s::Graph,
    io_buffers: &HashMap<i32, Vec<u8>>,
) -> Result<Vec<ZenEncodeResult>, ZenError> {
    // Topological sort: v2 Graph has string keys + Edge{from, to, kind}.
    let mut id_order: Vec<String> = graph.nodes.keys().cloned().collect();
    id_order.sort_by(|a, b| {
        a.parse::<i32>().unwrap_or(i32::MAX).cmp(&b.parse::<i32>().unwrap_or(i32::MAX))
    });

    let i32_to_idx: HashMap<i32, usize> = id_order.iter().enumerate()
        .filter_map(|(i, id)| id.parse::<i32>().ok().map(|n| (n, i)))
        .collect();

    let mut dag_nodes: Vec<zenpipe::DagNode> = Vec::new();
    let mut decode_io_ids: Vec<(usize, i32)> = Vec::new();
    let mut encode_io_ids: Vec<(usize, i32)> = Vec::new();
    let mut encode_pipeline: Option<TranslatedPipeline> = None;

    for (dag_idx, id) in id_order.iter().enumerate() {
        let node = &graph.nodes[id];
        let mut partial = translate::translate_nodes(&[node.clone()])?;

        if let Some(io_id) = partial.decode_io_id {
            decode_io_ids.push((dag_idx, io_id));
        }
        if let Some(io_id) = partial.encode_io_id {
            encode_io_ids.push((dag_idx, io_id));
            encode_pipeline = Some(partial.clone_config());
        }

        let inputs: Vec<usize> = graph.edges.iter()
            .filter(|e| i32_to_idx.get(&e.to).copied() == Some(dag_idx))
            .filter_map(|e| i32_to_idx.get(&e.from).copied())
            .collect();

        let instance = if !partial.nodes.is_empty() {
            partial.nodes.remove(0)
        } else {
            create_encode_role_placeholder()
        };

        dag_nodes.push(zenpipe::DagNode { instance, inputs });
    }

    // Decode all input sources.
    let registry = AllowedFormats::all();
    let mut sources: Vec<(usize, Box<dyn zenpipe::Source>)> = Vec::new();
    for (dag_idx, io_id) in &decode_io_ids {
        let input_data = io_buffers.get(io_id).ok_or_else(|| {
            ZenError::Io(format!("no input buffer for io_id {io_id}"))
        })?;
        sources.push((*dag_idx, decode_to_source(input_data, &registry)?));
    }

    // Build DAG pipeline.
    let converters = super::converter::imageflow_converters();
    let converters: &[&dyn zenpipe::bridge::NodeConverter] = &converters;
    let pipe_result = zenpipe::bridge::build_pipeline_dag(sources, &dag_nodes, converters)?;

    // Resolve format + quality from the first input.
    let first_input = io_buffers.get(&decode_io_ids[0].1).ok_or_else(|| {
        ZenError::Io("no input for format probe".into())
    })?;
    let info = zencodecs::from_bytes(first_input)
        .map_err(|e| ZenError::Codec(format!("probe: {e}")))?;

    let codec_intent = encode_pipeline
        .as_ref()
        .and_then(|p| p.preset.as_ref())
        .map(|p| &p.intent)
        .cloned()
        .unwrap_or_default();

    let decision = select_format_from_intent(
        &codec_intent, &ImageFacts::from_image_info(&info),
        &registry, &CodecPolicy::default(),
    ).map_err(|e| ZenError::Codec(format!("format selection: {e}")))?;

    let encode_io_id = encode_io_ids.first().map(|(_, id)| *id).unwrap_or(1);
    stream_encode(pipe_result.source, &decision, encode_io_id)
}

// ─── Decode ───

/// Probe, resolve format/quality, and build a streaming decode source.
fn probe_resolve_decode(
    input_data: &[u8],
    pipeline: &TranslatedPipeline,
    decoder_commands: &Option<Vec<imageflow_types::DecoderCommand>>,
    cms_mode: imageflow_types::CmsMode,
) -> Result<(zencodecs::FormatDecision, Box<dyn zenpipe::Source>), ZenError> {
    let registry = AllowedFormats::all();
    let info = zencodecs::from_bytes(input_data)
        .map_err(|e| ZenError::Codec(format!("probe: {e}")))?;

    let codec_intent = pipeline.preset.as_ref()
        .map(|p| &p.intent)
        .cloned()
        .unwrap_or_default();

    let decision = select_format_from_intent(
        &codec_intent, &ImageFacts::from_image_info(&info),
        &registry, &CodecPolicy::default(),
    ).map_err(|e| ZenError::Codec(format!("format selection: {e}")))?;

    // Check for frame selection command.
    let frame_index = decoder_commands
        .as_ref()
        .and_then(|cmds| cmds.iter().find_map(|c| match c {
            imageflow_types::DecoderCommand::SelectFrame(i) => Some(*i as usize),
            _ => None,
        }));

    let mut source = if let Some(frame_idx) = frame_index {
        // Frame selection: use full-frame decode and select the requested frame.
        decode_to_source_frame(input_data, &registry, frame_idx)?
    } else {
        decode_to_source(input_data, &registry)?
    };

    // ICC color management: if the source has an embedded ICC profile that
    // isn't sRGB, transform to sRGB using moxcms. This matches v2 behavior
    // where CMS transforms to sRGB during decode.
    source = apply_icc_transform(source, &info, cms_mode)?;

    // PNG gAMA/cHRM: if no ICC profile, try to synthesize from PNG metadata.
    if info.source_color.icc_profile.is_none() && info.format == zencodecs::ImageFormat::Png {
        let honor_gama_only = decoder_commands
            .as_ref()
            .and_then(|cmds| cmds.iter().find_map(|c| match c {
                imageflow_types::DecoderCommand::HonorGamaOnly(v) => Some(*v),
                _ => None,
            }))
            .unwrap_or(false);
        // HonorGamaChrm(false) disables gAMA+cHRM transforms entirely.
        let honor_gama_chrm = decoder_commands
            .as_ref()
            .and_then(|cmds| cmds.iter().find_map(|c| match c {
                imageflow_types::DecoderCommand::HonorGamaChrm(v) => Some(*v),
                _ => None,
            }))
            .unwrap_or(true); // default: honor gAMA+cHRM
        if honor_gama_chrm {
            source = apply_png_gamma_transform(source, input_data, honor_gama_only)?;
        }
    }

    // Format conversion: ensure RGBA8 sRGB pixel format for downstream.
    source = ensure_srgb_rgba8(source)?;

    Ok((decision, source))
}

/// Apply ICC→sRGB transform if the source image has a non-sRGB ICC profile.
///
/// On failure (unsupported pixel format, bad ICC data), returns the source
/// unchanged — falling back to format-only conversion.
fn apply_icc_transform(
    source: Box<dyn zenpipe::Source>,
    info: &zencodecs::ImageInfo,
    cms_mode: imageflow_types::CmsMode,
) -> Result<Box<dyn zenpipe::Source>, ZenError> {
    // 1. Try embedded ICC profile first.
    let src_icc = if let Some(icc) = &info.source_color.icc_profile {
        if !icc.is_empty() { Some(icc.clone()) } else { None }
    } else {
        None
    };

    // 2. If no ICC, skip CMS.
    let src_icc = match src_icc {
        Some(icc) => icc,
        None => return Ok(source),
    };

    // In compat mode, skip transforms for sRGB-like profiles (loose match).
    // In scene-referred mode, only skip for exact sRGB (strict match).
    match cms_mode {
        imageflow_types::CmsMode::Imageflow2Compat => {
            // V2 behavior: skip any profile that looks like sRGB.
            // Uses description-tag heuristic — catches vendor variants.
            if info.source_color.is_srgb() || is_srgb_icc_profile_loose(&src_icc) {
                return Ok(source);
            }
        }
        imageflow_types::CmsMode::SceneReferred => {
            // Strict: only skip for exact sRGB (primaries + TRC match).
            if is_srgb_icc_profile(&src_icc) {
                return Ok(source);
            }
        }
    }

    let srgb_icc = srgb_icc_profile();
    let src_format = source.format();
    let pixel_format = src_format.pixel_format();

    // Pre-check: try to build the CMS transform without consuming the source.
    use zenpipe::ColorManagement as _;
    let transform = zenpipe::MoxCms.build_transform_for_format(
        &src_icc, &srgb_icc, pixel_format, pixel_format,
    );

    match transform {
        Ok(row_transform) => {
            let dst_icc: std::sync::Arc<[u8]> = std::sync::Arc::from(srgb_icc.as_slice());
            let transformed = zenpipe::sources::IccTransformSource::from_transform(
                source, row_transform, dst_icc,
            );
            Ok(Box::new(transformed))
        }
        Err(_e) => {
            // ICC transform not possible for this pixel format.
            // Fall back to format-only conversion (preserves source).
            Ok(source)
        }
    }
}

/// Get the sRGB ICC profile bytes.
fn srgb_icc_profile() -> Vec<u8> {
    use std::sync::OnceLock;
    static SRGB: OnceLock<Vec<u8>> = OnceLock::new();
    SRGB.get_or_init(|| {
        moxcms::ColorProfile::new_srgb().encode().unwrap_or_default()
    }).clone()
}

/// Check if an ICC profile represents sRGB (or close enough to skip transform).
///
/// Parses the profile with moxcms and checks if its primaries and TRC match sRGB.
/// Camera JPEGs embed vendor-specific sRGB profiles with different bytes but
/// same color space — byte comparison doesn't work, need semantic comparison.
/// Loose sRGB check matching v2 behavior: skip if profile description says "sRGB".
///
/// This is intentionally loose — vendor-calibrated profiles (Canon, Sony) have
/// "sRGB" in their description but slightly different primaries/TRC. V2 skips
/// transforms for these, so we do too in compat mode.
fn is_srgb_icc_profile_loose(icc_bytes: &[u8]) -> bool {
    // Check if the ICC profile description contains "sRGB".
    zencodec::icc_profile_is_srgb(icc_bytes)
}

/// Check if an ICC profile is sRGB-equivalent by comparing primaries AND TRC curves.
///
/// Uses moxcms to parse the profile and compares colorants (with 0.0001 tolerance
/// via Xyzd::PartialEq) and TRC parametric parameters (with tolerance for vendor
/// rounding). Catches vendor sRGB variants (Canon, Sony, etc.) that have different
/// bytes but identical color behavior.
fn is_srgb_icc_profile(icc_bytes: &[u8]) -> bool {
    let Ok(src) = moxcms::ColorProfile::new_from_slice(icc_bytes) else {
        return false;
    };
    let srgb = moxcms::ColorProfile::new_srgb();

    // 1. Primaries must match (Xyzd::PartialEq has 0.0001 tolerance).
    if src.red_colorant != srgb.red_colorant
        || src.green_colorant != srgb.green_colorant
        || src.blue_colorant != srgb.blue_colorant
    {
        return false;
    }

    // 2. TRC: must be sRGB-equivalent (parametric or LUT).
    trc_matches_srgb(&src.red_trc)
        && trc_matches_srgb(&src.green_trc)
        && trc_matches_srgb(&src.blue_trc)
}

/// Check if a TRC curve matches the sRGB parametric curve within tolerance.
///
/// sRGB TRC is parametric type 4: [2.4, 1/1.055, 0.055/1.055, 1/12.92, 0.04045]
/// Vendor profiles may round these differently (e.g., 0.947867... vs 0.9479).
fn trc_matches_srgb(trc: &Option<moxcms::ToneReprCurve>) -> bool {
    let Some(trc) = trc else { return false };

    match trc {
        moxcms::ToneReprCurve::Parametric(params) => {
            // sRGB parametric: [gamma, a, b, c, d]
            // Expected: [2.4, 1/1.055 ≈ 0.94787, 0.055/1.055 ≈ 0.05213, 1/12.92 ≈ 0.07739, 0.04045]
            const SRGB_PARAMS: [f32; 5] = [
                2.4,
                1.0 / 1.055,     // 0.947867...
                0.055 / 1.055,   // 0.052132...
                1.0 / 12.92,     // 0.077399...
                0.04045,
            ];
            const TOL: f32 = 0.001;

            if params.len() < 5 { return false; }
            params[..5].iter().zip(SRGB_PARAMS.iter()).all(|(a, b)| (a - b).abs() < TOL)
        }
        moxcms::ToneReprCurve::Lut(lut) => {
            // Some profiles encode sRGB as a 1024 or 4096 entry LUT.
            // Check a few diagnostic points against expected sRGB values.
            if lut.is_empty() { return false; }
            let n = lut.len();

            // sRGB curve: output = ((input/1.055 + 0.055/1.055)^2.4) for input > 0.04045
            // Check at 25%, 50%, 75% input.
            let check_points = [n / 4, n / 2, 3 * n / 4];
            for &idx in &check_points {
                let input = idx as f64 / (n - 1) as f64;
                let expected = if input <= 0.04045 {
                    input / 12.92
                } else {
                    ((input + 0.055) / 1.055).powf(2.4)
                };
                let actual = lut[idx] as f64 / 65535.0;
                if (actual - expected).abs() > 0.002 {
                    return false;
                }
            }
            true
        }
    }
}

/// Synthesize an ICC profile from PNG gAMA (and optional cHRM) metadata.
///
/// If gAMA is close to sRGB (0.45455), returns None (no transform needed).
/// Otherwise, creates a gamma+primaries profile using moxcms.
fn synthesize_icc_from_gama(
    gamma_scaled: u32,
    chromaticities: &Option<[u32; 8]>,
) -> Option<Vec<u8>> {
    let gamma_f = gamma_scaled as f64 / 100000.0;
    let neutral_low = 0.4318;
    let neutral_high = 0.4773;

    let chrm_is_srgb = chromaticities.map_or(true, |c| {
        // sRGB primaries scaled by 100000. Tolerance: 1% (1000) to handle rounding.
        let srgb = [31270u32, 32900, 64000, 33000, 30000, 60000, 15000, 6000];
        c.iter().zip(srgb.iter()).all(|(a, b)| (*a as i64 - *b as i64).unsigned_abs() < 1000)
    });

    if gamma_f >= neutral_low && gamma_f <= neutral_high && chrm_is_srgb {
        return None;
    }

    // Build profile using moxcms: start from sRGB, update colorimetry + TRC, clear CICP.
    // Pattern from moxcms issue #154.
    let display_gamma = 1.0 / gamma_f;

    let mut profile = moxcms::ColorProfile::new_srgb();

    // Update primaries if cHRM is present and non-sRGB.
    if let Some(c) = chromaticities {
        if !chrm_is_srgb {
            let white = moxcms::XyY::new(
                c[0] as f64 / 100000.0,
                c[1] as f64 / 100000.0,
                1.0,
            );
            let primaries = moxcms::ColorPrimaries {
                red: moxcms::Chromaticity { x: c[2] as f32 / 100000.0, y: c[3] as f32 / 100000.0 },
                green: moxcms::Chromaticity { x: c[4] as f32 / 100000.0, y: c[5] as f32 / 100000.0 },
                blue: moxcms::Chromaticity { x: c[6] as f32 / 100000.0, y: c[7] as f32 / 100000.0 },
            };
            profile.update_rgb_colorimetry(white, primaries);
        }
    }

    // Override TRC with pure gamma curve (parametric type 0: Y = X^gamma).
    let trc = moxcms::ToneReprCurve::Parametric(vec![display_gamma as f32]);
    profile.red_trc = Some(trc.clone());
    profile.green_trc = Some(trc.clone());
    profile.blue_trc = Some(trc);

    // Clear CICP to prevent it from overriding our TRC (issue #154).
    profile.cicp = None;

    profile.encode().ok()
}

/// Build a minimal ICC v2 RGB profile with gamma TRC and optional custom primaries.
/// Fallback when moxcms profile creation fails.
fn build_minimal_icc_v2(gamma: f32, chrm: &Option<[u32; 8]>) -> Option<Vec<u8>> {
    // ICC v2 profile structure:
    // Header (128 bytes) + Tag table (4 + N*12) + Tag data

    // Primaries: convert chromaticity xy → XYZ (D50 adapted).
    // Use sRGB primaries as default.
    let (wx, wy) = chrm.map_or((0.3127f64, 0.3290), |c| (c[0] as f64 / 100000.0, c[1] as f64 / 100000.0));
    let (rx, ry) = chrm.map_or((0.64, 0.33), |c| (c[2] as f64 / 100000.0, c[3] as f64 / 100000.0));
    let (gx, gy) = chrm.map_or((0.30, 0.60), |c| (c[4] as f64 / 100000.0, c[5] as f64 / 100000.0));
    let (bx, by) = chrm.map_or((0.15, 0.06), |c| (c[6] as f64 / 100000.0, c[7] as f64 / 100000.0));

    // Convert xyY → XYZ with Y=1 for white point
    let w_xyz = xy_to_xyz(wx, wy);
    let r_xyz = xy_to_xyz(rx, ry);
    let g_xyz = xy_to_xyz(gx, gy);
    let b_xyz = xy_to_xyz(bx, by);

    // Compute RGB→XYZ matrix (before chromatic adaptation)
    let (m_r, m_g, m_b) = compute_rgb_to_xyz_matrix(
        r_xyz, g_xyz, b_xyz, w_xyz,
    )?;

    // Chromatic adapt to D50 (ICC PCS) using Bradford
    let d50 = [0.9642, 1.0000, 0.8249];
    let chad = bradford_matrix(w_xyz, d50);
    let mr = mat3_mul_vec3(&chad, &m_r);
    let mg = mat3_mul_vec3(&chad, &m_g);
    let mb = mat3_mul_vec3(&chad, &m_b);

    // Build the profile
    let gamma_fixed = s15fixed16(gamma as f64);

    // 9 tags: rXYZ, gXYZ, bXYZ, rTRC, gTRC, bTRC, wtpt, cprt, desc
    let n_tags = 9u32;
    let tag_table_size = 4 + n_tags * 12;
    let header_size = 128u32;

    // Tag data: each curv tag = 12 bytes (type + reserved + count + gamma)
    // Each XYZ tag = 20 bytes (type + reserved + 3 × s15Fixed16)
    // wtpt = 20 bytes, cprt = 12+4 bytes, desc = 12+12 bytes

    let curv_size = 12u32;
    let xyz_size = 20u32;
    let cprt_data = b"CC0 ";
    let cprt_size = (12 + cprt_data.len() as u32 + 3) & !3; // pad to 4
    let desc_text = b"Synthetic";
    let desc_size = (12 + 4 + desc_text.len() as u32 + 1 + 12 + 67 + 3) & !3;

    let data_start = header_size + tag_table_size;
    let mut offset = data_start;

    let mut buf = Vec::with_capacity(512);

    // Placeholder header (128 bytes)
    buf.resize(128, 0u8);

    // Header fields
    let total_size_pos = 0; // will be patched
    buf[4..8].copy_from_slice(b"acsp"); // signature
    buf[12..16].copy_from_slice(b"mntr"); // device class: monitor
    buf[16..20].copy_from_slice(b"RGB "); // color space
    buf[20..24].copy_from_slice(b"XYZ "); // PCS
    // Date/time: 2024-01-01
    buf[24..26].copy_from_slice(&2024u16.to_be_bytes());
    buf[26..28].copy_from_slice(&1u16.to_be_bytes());
    buf[28..30].copy_from_slice(&1u16.to_be_bytes());
    buf[36..40].copy_from_slice(b"acsp"); // file signature
    buf[64..68].copy_from_slice(&s15fixed16(1.0).to_be_bytes()); // illuminant X
    buf[68..72].copy_from_slice(&s15fixed16(1.0).to_be_bytes()); // Y
    buf[72..76].copy_from_slice(&s15fixed16(1.0).to_be_bytes()); // Z
    buf[40..44].copy_from_slice(b"APPL"); // preferred CMM
    buf[8..12].copy_from_slice(&0x02100000u32.to_be_bytes()); // version 2.1

    // Tag count
    buf.extend_from_slice(&n_tags.to_be_bytes());

    // Tag table entries: [sig:4][offset:4][size:4]
    let tags: Vec<(&[u8; 4], u32)> = vec![
        (b"rXYZ", xyz_size), (b"gXYZ", xyz_size), (b"bXYZ", xyz_size),
        (b"rTRC", curv_size), (b"gTRC", curv_size), (b"bTRC", curv_size),
        (b"wtpt", xyz_size), (b"cprt", cprt_size), (b"desc", desc_size),
    ];
    for (sig, size) in &tags {
        buf.extend_from_slice(sig.as_slice());
        buf.extend_from_slice(&offset.to_be_bytes());
        buf.extend_from_slice(&size.to_be_bytes());
        offset += size;
    }

    // Tag data
    fn write_xyz(buf: &mut Vec<u8>, xyz: &[f64; 3]) {
        buf.extend_from_slice(b"XYZ "); // type
        buf.extend_from_slice(&[0u8; 4]); // reserved
        for v in xyz {
            buf.extend_from_slice(&s15fixed16(*v).to_be_bytes());
        }
    }
    fn write_curv_gamma(buf: &mut Vec<u8>, gamma_val: f32) {
        buf.extend_from_slice(b"curv"); // type
        buf.extend_from_slice(&[0u8; 4]); // reserved
        buf.extend_from_slice(&1u32.to_be_bytes()); // count = 1 (gamma)
        let uf8 = (gamma_val as f64 * 256.0) as u16;
        buf.extend_from_slice(&uf8.to_be_bytes());
        buf.extend_from_slice(&[0u8; 2]); // padding
    }

    write_xyz(&mut buf, &mr);
    write_xyz(&mut buf, &mg);
    write_xyz(&mut buf, &mb);
    write_curv_gamma(&mut buf, gamma);
    write_curv_gamma(&mut buf, gamma);
    write_curv_gamma(&mut buf, gamma);
    // wtpt (D50)
    write_xyz(&mut buf, &d50);
    // cprt
    buf.extend_from_slice(b"text");
    buf.extend_from_slice(&[0u8; 4]);
    buf.extend_from_slice(cprt_data);
    while buf.len() % 4 != 0 { buf.push(0); }
    // desc (minimal)
    let desc_start = buf.len();
    buf.extend_from_slice(b"desc");
    buf.extend_from_slice(&[0u8; 4]);
    buf.extend_from_slice(&(desc_text.len() as u32 + 1).to_be_bytes());
    buf.extend_from_slice(desc_text);
    buf.push(0);
    // Unicode and ScriptCode records (empty)
    buf.extend_from_slice(&[0u8; 4]); // Unicode language code
    buf.extend_from_slice(&0u32.to_be_bytes()); // Unicode count
    buf.extend_from_slice(&[0u8; 2]); // ScriptCode code
    buf.push(0); // ScriptCode count
    buf.extend_from_slice(&[0u8; 67]); // ScriptCode data
    while buf.len() % 4 != 0 { buf.push(0); }

    // Patch total size
    let total = buf.len() as u32;
    buf[0..4].copy_from_slice(&total.to_be_bytes());

    Some(buf)
}

fn s15fixed16(v: f64) -> i32 {
    (v * 65536.0) as i32
}

fn xy_to_xyz(x: f64, y: f64) -> [f64; 3] {
    if y.abs() < 1e-10 { return [0.0, 0.0, 0.0]; }
    [x / y, 1.0, (1.0 - x - y) / y]
}

fn compute_rgb_to_xyz_matrix(
    r: [f64; 3], g: [f64; 3], b: [f64; 3], w: [f64; 3],
) -> Option<([f64; 3], [f64; 3], [f64; 3])> {
    // Solve: [R G B] * S = W, where S = [Sr, Sg, Sb]
    let m = [
        [r[0], g[0], b[0]],
        [r[1], g[1], b[1]],
        [r[2], g[2], b[2]],
    ];
    let inv = mat3_inv(&m)?;
    let s = mat3_mul_vec3(&inv, &w);

    Some(([r[0]*s[0], r[1]*s[0], r[2]*s[0]],
          [g[0]*s[1], g[1]*s[1], g[2]*s[1]],
          [b[0]*s[2], b[1]*s[2], b[2]*s[2]]))
}

fn bradford_matrix(src_w: [f64; 3], dst_w: [f64; 3]) -> [[f64; 3]; 3] {
    // Bradford chromatic adaptation matrix
    let brad = [
        [ 0.8951,  0.2664, -0.1614],
        [-0.7502,  1.7135,  0.0367],
        [ 0.0389, -0.0685,  1.0296],
    ];
    let brad_inv = [
        [ 0.9870, -0.1471, 0.1600],
        [ 0.4323,  0.5184, 0.0493],
        [-0.0085,  0.0400, 0.9685],
    ];
    let src_lms = mat3_mul_vec3(&brad, &src_w);
    let dst_lms = mat3_mul_vec3(&brad, &dst_w);
    let scale = [
        [dst_lms[0]/src_lms[0], 0.0, 0.0],
        [0.0, dst_lms[1]/src_lms[1], 0.0],
        [0.0, 0.0, dst_lms[2]/src_lms[2]],
    ];
    let tmp = mat3_mul(&scale, &brad);
    mat3_mul(&brad_inv, &tmp)
}

fn mat3_mul_vec3(m: &[[f64; 3]; 3], v: &[f64; 3]) -> [f64; 3] {
    [
        m[0][0]*v[0] + m[0][1]*v[1] + m[0][2]*v[2],
        m[1][0]*v[0] + m[1][1]*v[1] + m[1][2]*v[2],
        m[2][0]*v[0] + m[2][1]*v[1] + m[2][2]*v[2],
    ]
}

fn mat3_mul(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut r = [[0.0f64; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = a[i][0]*b[0][j] + a[i][1]*b[1][j] + a[i][2]*b[2][j];
        }
    }
    r
}

fn mat3_inv(m: &[[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let det = m[0][0]*(m[1][1]*m[2][2] - m[1][2]*m[2][1])
            - m[0][1]*(m[1][0]*m[2][2] - m[1][2]*m[2][0])
            + m[0][2]*(m[1][0]*m[2][1] - m[1][1]*m[2][0]);
    if det.abs() < 1e-10 { return None; }
    let inv_det = 1.0 / det;
    Some([
        [(m[1][1]*m[2][2]-m[1][2]*m[2][1])*inv_det, (m[0][2]*m[2][1]-m[0][1]*m[2][2])*inv_det, (m[0][1]*m[1][2]-m[0][2]*m[1][1])*inv_det],
        [(m[1][2]*m[2][0]-m[1][0]*m[2][2])*inv_det, (m[0][0]*m[2][2]-m[0][2]*m[2][0])*inv_det, (m[0][2]*m[1][0]-m[0][0]*m[1][2])*inv_det],
        [(m[1][0]*m[2][1]-m[1][1]*m[2][0])*inv_det, (m[0][1]*m[2][0]-m[0][0]*m[2][1])*inv_det, (m[0][0]*m[1][1]-m[0][1]*m[1][0])*inv_det],
    ])
}

/// Parse gAMA and optional cHRM from raw PNG bytes, synthesize ICC, and apply transform.
fn apply_png_gamma_transform(
    source: Box<dyn zenpipe::Source>,
    png_data: &[u8],
    honor_gama_only: bool,
) -> Result<Box<dyn zenpipe::Source>, ZenError> {
    let (gamma, chrm, has_srgb, has_cicp) = parse_png_color_chunks(png_data);
    let gamma = match gamma {
        Some(g) if g > 0 => g,
        _ => return Ok(source),
    };

    // sRGB chunk → already sRGB, no transform.
    if has_srgb {
        return Ok(source);
    }

    // cICP chunk takes precedence over gAMA+cHRM (PNG 3rd Ed spec).
    // cICP handling is done via ICC path; don't double-transform.
    if has_cicp {
        return Ok(source);
    }

    // Validate cHRM: reject degenerate chromaticities (y=0 causes division by zero).
    if let Some(ref c) = chrm {
        if c.iter().enumerate().any(|(i, v)| i % 2 == 1 && *v == 0) {
            return Ok(source); // Degenerate cHRM — skip
        }
    }

    // gAMA-only (no cHRM) is ignored unless HonorGamaOnly is set.
    if chrm.is_none() && !honor_gama_only {
        return Ok(source);
    }

    let icc = match synthesize_icc_from_gama(gamma, &chrm) {
        Some(icc) => icc,
        None => return Ok(source), // Gamma is neutral sRGB — no transform
    };

    let srgb_icc = srgb_icc_profile();
    let src_format = source.format();
    let pixel_format = src_format.pixel_format();

    use zenpipe::ColorManagement as _;
    let transform = zenpipe::MoxCms.build_transform_for_format(
        &icc, &srgb_icc, pixel_format, pixel_format,
    );

    match transform {
        Ok(row_transform) => {
            let dst_icc: std::sync::Arc<[u8]> = std::sync::Arc::from(srgb_icc.as_slice());
            let transformed = zenpipe::sources::IccTransformSource::from_transform(
                source, row_transform, dst_icc,
            );
            Ok(Box::new(transformed))
        }
        Err(_) => Ok(source),
    }
}

/// Stream-encode an animated image via zencodecs: decode frame → push_frame → repeat → finish.
/// Streaming: only one frame in memory at a time.
fn encode_animation_passthrough(
    input_data: &[u8],
    registry: &AllowedFormats,
    decision: &zencodecs::FormatDecision,
    encode_io_id: i32,
) -> Result<Vec<ZenEncodeResult>, ZenError> {
    let mut decoder = zencodecs::DecodeRequest::new(input_data)
        .with_registry(registry)
        .animation_frame_decoder()
        .map_err(|e| ZenError::Codec(format!("animation decoder: {e}")))?;

    let info = decoder.info().clone();
    let w = info.width;
    let h = info.height;

    // Create animation frame encoder via zencodecs.
    let mut encoder = zencodecs::EncodeRequest::new(decision.format)
        .with_quality(decision.quality.quality)
        .with_lossless(decision.lossless)
        .with_registry(registry)
        .animation_frame_encoder(w, h)
        .map_err(|e| ZenError::Codec(format!("animation encoder: {e}")))?;

    // Stream: decode one frame, push to encoder, release frame memory.
    while let Some(frame) = decoder.render_next_frame_owned(None)
        .map_err(|e| ZenError::Codec(format!("decode frame: {e}")))? {
        let duration = frame.duration_ms();
        let pixels = frame.pixels();
        encoder.push_frame(pixels, duration, None)
            .map_err(|e| ZenError::Codec(format!("push_frame: {e}")))?;
    }

    let output = encoder.finish(None)
        .map_err(|e| ZenError::Codec(format!("finish animation: {e}")))?;

    let mut bytes = output.into_vec();
    // Ensure GIF trailer.
    if decision.format == zencodecs::ImageFormat::Gif && bytes.last() != Some(&0x3B) {
        bytes.push(0x3B);
    }

    Ok(vec![ZenEncodeResult {
        io_id: encode_io_id,
        bytes,
        width: w,
        height: h,
        mime_type: decision.format.mime_type(),
        extension: decision.format.extension(),
    }])
}

/// Parse PNG color-related chunks: gAMA, cHRM, sRGB, cICP.
fn parse_png_color_chunks(data: &[u8]) -> (Option<u32>, Option<[u32; 8]>, bool, bool) {
    let mut gamma = None;
    let mut chrm = None;
    let mut has_srgb = false;
    let mut has_cicp = false;

    if data.len() < 8 || &data[0..8] != b"\x89PNG\r\n\x1a\n" {
        return (None, None, false, false);
    }
    let mut pos = 8;
    while pos + 8 <= data.len() {
        let len = u32::from_be_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]) as usize;
        let chunk_type = &data[pos+4..pos+8];
        let chunk_data_start = pos + 8;
        let chunk_end = chunk_data_start + len + 4;
        if chunk_end > data.len() { break; }

        match chunk_type {
            b"gAMA" if len == 4 => {
                gamma = Some(u32::from_be_bytes([
                    data[chunk_data_start], data[chunk_data_start+1],
                    data[chunk_data_start+2], data[chunk_data_start+3],
                ]));
            }
            b"cHRM" if len == 32 => {
                let d = &data[chunk_data_start..];
                let r = |off: usize| u32::from_be_bytes([d[off], d[off+1], d[off+2], d[off+3]]);
                chrm = Some([r(0), r(4), r(8), r(12), r(16), r(20), r(24), r(28)]);
            }
            b"sRGB" => { has_srgb = true; }
            b"cICP" => { has_cicp = true; }
            b"IDAT" | b"IEND" => break,
            _ => {}
        }
        pos = chunk_end;
    }
    (gamma, chrm, has_srgb, has_cicp)
}

/// Wrap a source with a format conversion to RGBA8 sRGB if needed.
///
/// v2 compatibility: the v2 engine always decodes to BGRA32 sRGB via CMS.
/// The zen pipeline preserves the source format. This function inserts a
/// conversion to RGBA8 sRGB when the source isn't already in that format.
fn ensure_srgb_rgba8(
    source: Box<dyn zenpipe::Source>,
) -> Result<Box<dyn zenpipe::Source>, ZenError> {
    let src_format = source.format();
    let target = zenpipe::format::RGBA8_SRGB;

    if src_format == target {
        return Ok(source);
    }
    // Try to create a format conversion.
    if let Some(converter) = zenpipe::ops::RowConverterOp::new(src_format, target) {
        let transform = zenpipe::sources::TransformSource::new(source)
            .push_boxed(Box::new(converter));
        Ok(Box::new(transform))
    } else {
        // No conversion path — log and proceed with original format.
        // The pipeline will attempt format negotiation at later stages.
        Ok(source)
    }
}

/// Decode a specific frame from an animated/multi-frame image.
fn decode_to_source_frame(
    data: &[u8],
    registry: &AllowedFormats,
    frame_index: usize,
) -> Result<Box<dyn zenpipe::Source>, ZenError> {
    let mut decoder = zencodecs::DecodeRequest::new(data)
        .with_registry(registry)
        .animation_frame_decoder()
        .map_err(|e| ZenError::Codec(format!("frame decoder: {e}")))?;

    // Iterate to the requested frame.
    for i in 0..=frame_index {
        let frame = decoder.render_next_frame_owned(None)
            .map_err(|e| ZenError::Codec(format!("decode frame {i}: {e}")))?
            .ok_or_else(|| ZenError::Codec(format!("frame index {frame_index} out of range (only {i} frames)")))?;

        if i == frame_index {
            let buf = frame.into_buffer();
            let w = buf.width();
            let h = buf.height();
            let format = buf.descriptor();
            let bytes = buf.copy_to_contiguous_bytes();
            return Ok(Box::new(zenpipe::sources::MaterializedSource::from_data(bytes, w, h, format)));
        }
    }
    unreachable!()
}

/// Build a streaming decode source. Tries row-level streaming first
/// (JPEG, PNG, GIF, AVIF, HEIC), falls back to full-frame + MaterializedSource.
fn decode_to_source(
    data: &[u8],
    registry: &AllowedFormats,
) -> Result<Box<dyn zenpipe::Source>, ZenError> {
    // Reject truncated/corrupt files (v2 compat) but allow everything else.
    let mut policy = zencodecs::DecodePolicy::none();
    policy.allow_truncated = Some(false);
    match zencodecs::DecodeRequest::new(data)
        .with_registry(registry)
        .with_decode_policy(policy.clone())
        .build_streaming_decoder()
    {
        Ok(streaming) => {
            let source = zenpipe::codec::DecoderSource::new(streaming)?;
            Ok(Box::new(source))
        }
        Err(_) => {
            let decoded = zencodecs::DecodeRequest::new(data)
                .with_registry(registry)
                .with_decode_policy(policy)
                .decode_full_frame()
                .map_err(|e| ZenError::Codec(format!("decode: {e}")))?;

            let w = decoded.width();
            let h = decoded.height();
            let format = decoded.descriptor();
            let bytes = decoded.into_buffer().copy_to_contiguous_bytes();
            Ok(Box::new(zenpipe::sources::MaterializedSource::from_data(bytes, w, h, format)))
        }
    }
}

// ─── Encode ───

/// Stream pipeline output into an encoder via EncoderSink.
/// Falls back to one-shot materialized encode if streaming isn't supported (e.g., GIF).
/// Build a `CodecConfig` from `FormatDecision.hints` when format-specific
/// encoder configuration is needed (e.g., mozjpeg preset for JPEG).
fn build_codec_config_from_hints(
    decision: &zencodecs::FormatDecision,
) -> Option<zencodecs::config::CodecConfig> {
    let preset_hint = decision.hints.get("preset")?;
    let quality = decision.quality.quality;
    zencodecs::jpeg_codec_config_for_preset(preset_hint, quality)
}

fn stream_encode(
    mut source: Box<dyn zenpipe::Source>,
    decision: &zencodecs::FormatDecision,
    encode_io_id: i32,
) -> Result<Vec<ZenEncodeResult>, ZenError> {
    let out_w = source.width();
    let out_h = source.height();
    let out_format = source.format();
    let registry = AllowedFormats::all();

    // GIF doesn't support streaming row-level encode — always use one-shot.
    let use_oneshot = matches!(decision.format, zencodecs::ImageFormat::Gif);

    // Build codec config from hints (e.g., mozjpeg preset for JPEG).
    let codec_config = build_codec_config_from_hints(decision);

    let output = if !use_oneshot {
        // Try streaming encode first.
        let mut req = zencodecs::EncodeRequest::new(decision.format)
            .with_quality(decision.quality.quality)
            .with_lossless(decision.lossless)
            .with_registry(&registry);
        if let Some(ref cfg) = codec_config {
            req = req.with_codec_config(cfg);
        }
        let streaming_enc = req
            .build_streaming_encoder(out_w, out_h)
            .map_err(|e| ZenError::Codec(format!("encoder: {e}")))?;

        let mut sink = zenpipe::codec::EncoderSink::new(streaming_enc.encoder, out_format);
        zenpipe::execute(source.as_mut(), &mut sink)?;
        sink.take_output().ok_or_else(|| {
            ZenError::Codec("encoder produced no output".into())
        })?
    } else {
        // One-shot encode: materialize and encode in one pass.
        let mat = zenpipe::sources::MaterializedSource::from_source(source)?;
        let w = mat.width();
        let h = mat.height();
        let fmt = mat.format();
        let data = mat.data();
        let stride = w as usize * fmt.bytes_per_pixel();
        let pixel_slice = zenpixels::PixelSlice::new(data, w, h, stride, fmt)
            .map_err(|e| ZenError::Codec(format!("pixel slice: {e}")))?;

        let mut req = zencodecs::EncodeRequest::new(decision.format)
            .with_quality(decision.quality.quality)
            .with_lossless(decision.lossless)
            .with_registry(&registry);
        if let Some(ref cfg) = codec_config {
            req = req.with_codec_config(cfg);
        }
        req.encode(pixel_slice, fmt.has_alpha())
            .map_err(|e| ZenError::Codec(format!("one-shot encode: {e}")))?
    };

    // Ensure GIF trailer byte is present (workaround for gif crate not writing it).
    let mut output_bytes = output.into_vec();
    if matches!(decision.format, zencodecs::ImageFormat::Gif) && output_bytes.last() != Some(&0x3B) {
        output_bytes.push(0x3B);
    }

    Ok(vec![ZenEncodeResult {
        io_id: encode_io_id,
        bytes: output_bytes,
        width: out_w,
        height: out_h,
        mime_type: decision.format.mime_type(),
        extension: decision.format.extension(),
    }])
}

// ─── Helpers ───

/// Collect decode info by probing input buffers for each Decode node.
fn collect_decode_infos(
    framewise: &Framewise,
    io_buffers: &HashMap<i32, Vec<u8>>,
) -> Vec<(i32, zencodecs::ImageInfo)> {
    let nodes = match framewise {
        Framewise::Steps(steps) => steps.as_slice(),
        Framewise::Graph(g) => return Vec::new(), // TODO: extract from graph
    };

    let mut infos = Vec::new();
    for node in nodes {
        let io_id = match node {
            Node::Decode { io_id, .. } => Some(*io_id),
            Node::CommandString { decode: Some(io_id), .. } => Some(*io_id),
            _ => None,
        };
        if let Some(io_id) = io_id {
            if let Some(data) = io_buffers.get(&io_id) {
                if let Ok(info) = zencodecs::from_bytes(data) {
                    infos.push((io_id, info));
                }
            }
        }
    }
    infos
}

/// Maximum pixel count for a canvas (100 megapixels).
const MAX_CANVAS_PIXELS: u64 = 100_000_000;

/// Check dimensions against a security FrameSizeLimit.
fn check_security_limit(
    w: u32,
    h: u32,
    limit: &Option<imageflow_types::FrameSizeLimit>,
    label: &str,
) -> Result<(), ZenError> {
    if let Some(ref lim) = limit {
        if w > lim.w as u32 {
            return Err(ZenError::SizeLimit(format!(
                "Frame width {w} exceeds {label}.w {}",
                lim.w
            )));
        }
        if h > lim.h as u32 {
            return Err(ZenError::SizeLimit(format!(
                "Frame height {h} exceeds {label}.h {}",
                lim.h
            )));
        }
        let mp = w as f32 * h as f32 / 1_000_000.0;
        if mp > lim.megapixels {
            return Err(ZenError::SizeLimit(format!(
                "Frame dimensions {w}x{h} ({mp:.1}MP) exceed {label}.megapixels {:.1}MP",
                lim.megapixels
            )));
        }
    }
    Ok(())
}

/// Check that image dimensions are within safe limits.
fn check_dimensions(w: u32, h: u32) -> Result<(), ZenError> {
    let pixels = w as u64 * h as u64;
    if pixels > MAX_CANVAS_PIXELS {
        return Err(ZenError::SizeLimit(format!(
            "canvas dimensions {w}x{h} ({pixels} pixels) exceed limit ({MAX_CANVAS_PIXELS} pixels)"
        )));
    }
    // Check for i32 overflow in pixel product (v2 compat)
    if w as i64 * h as i64 > i32::MAX as i64 {
        return Err(ZenError::SizeLimit(format!(
            "canvas dimensions {w}x{h} would overflow i32 in pixel product"
        )));
    }
    Ok(())
}

/// Create a solid-color image source from CreateCanvas parameters.
fn create_canvas_source(
    canvas: &translate::CreateCanvasParams,
) -> Result<Box<dyn zenpipe::Source>, ZenError> {
    let w = canvas.w;
    let h = canvas.h;
    check_dimensions(w, h)?;
    let bpp = 4usize; // RGBA8
    let format = zenpipe::format::RGBA8_SRGB;

    // Parse color to RGBA bytes.
    let (r, g, b, a) = match &canvas.color {
        imageflow_types::Color::Transparent => (0u8, 0, 0, 0),
        imageflow_types::Color::Black => (0, 0, 0, 255),
        imageflow_types::Color::Srgb(imageflow_types::ColorSrgb::Hex(hex)) => {
            let hex = hex.trim_start_matches('#');
            let r = u8::from_str_radix(&hex.get(0..2).unwrap_or("00"), 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex.get(2..4).unwrap_or("00"), 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex.get(4..6).unwrap_or("00"), 16).unwrap_or(0);
            let a = if hex.len() >= 8 {
                u8::from_str_radix(&hex[6..8], 16).unwrap_or(255)
            } else {
                255
            };
            (r, g, b, a)
        }
    };

    // Create pixel buffer filled with the color.
    let row_bytes = w as usize * bpp;
    let mut pixels = vec![0u8; h as usize * row_bytes];
    for y in 0..h as usize {
        for x in 0..w as usize {
            let offset = y * row_bytes + x * bpp;
            pixels[offset] = r;
            pixels[offset + 1] = g;
            pixels[offset + 2] = b;
            pixels[offset + 3] = a;
        }
    }

    Ok(Box::new(zenpipe::sources::MaterializedSource::from_data(pixels, w, h, format)))
}

/// Create a placeholder Encode-role node for DAG slots that are decode/encode.
/// The bridge separates these by role — the placeholder just needs a valid schema.
fn create_encode_role_placeholder() -> Box<dyn zennode::NodeInstance> {
    zencodecs::zennode_defs::QUALITY_INTENT_NODE_NODE
        .create_default()
        .expect("placeholder creation")
}

/// Expand CommandString nodes into concrete steps using RIAPI parsing.
///
/// CommandString needs source dimensions for layout computation. We probe
/// the decode source to get dimensions, then use `Ir4Expand::expand_steps()`
/// to produce concrete v2 Node steps.
fn expand_command_strings(
    steps: &[Node],
    io_buffers: &HashMap<i32, Vec<u8>>,
) -> Result<Vec<Node>, ZenError> {
    // Check if any CommandString nodes exist.
    let has_command_string = steps.iter().any(|n| matches!(n, Node::CommandString { .. }));
    if !has_command_string {
        return Ok(steps.to_vec());
    }

    // Find decode io_id to probe source dimensions.
    // Check both explicit Decode nodes and CommandString's decode field.
    let decode_io_id = steps.iter().find_map(|n| match n {
        Node::Decode { io_id, .. } => Some(*io_id),
        Node::CommandString { decode: Some(io_id), .. } => Some(*io_id),
        _ => None,
    });

    let (source_w, source_h, source_mime, source_lossless) = if let Some(io_id) = decode_io_id {
        if let Some(data) = io_buffers.get(&io_id) {
            let info = zencodecs::from_bytes(data)
                .map_err(|e| ZenError::Codec(format!("probe for CommandString: {e}")))?;
            (
                info.width as i32,
                info.height as i32,
                Some(info.format.mime_type().to_string()),
                info.source_encoding.as_ref().map_or(false, |se| se.is_lossless()),
            )
        } else {
            (0, 0, None, false)
        }
    } else {
        (0, 0, None, false)
    };

    // Expand each CommandString into concrete steps.
    let mut result = Vec::new();
    for node in steps {
        match node {
            Node::CommandString { kind: _, value, decode, encode, watermarks } => {
                use imageflow_riapi::ir4::*;

                // Inject Decode node if CommandString specifies a decode io_id.
                // Parse `frame=N` from querystring for frame selection.
                if let Some(dec_id) = decode {
                    let mut commands: Option<Vec<imageflow_types::DecoderCommand>> = None;
                    if let Ok(parsed) = Ir4Command::QueryString(value.clone()).parse() {
                        if let Some(frame) = parsed.parsed.frame {
                            commands = Some(vec![imageflow_types::DecoderCommand::SelectFrame(frame)]);
                        }
                    }
                    result.push(Node::Decode { io_id: *dec_id, commands });
                }

                let expand = Ir4Expand {
                    i: Ir4Command::QueryString(value.clone()),
                    source: Ir4SourceFrameInfo {
                        w: source_w,
                        h: source_h,
                        fmt: imageflow_types::PixelFormat::Bgra32,
                        original_mime: source_mime.clone(),
                        lossless: source_lossless,
                    },
                    reference_width: source_w,
                    reference_height: source_h,
                    encode_id: *encode,
                    watermarks: watermarks.clone(),
                };

                match expand.expand_steps() {
                    Ok(ir4_result) => {
                        if let Some(expanded_steps) = ir4_result.steps {
                            result.extend(expanded_steps);
                        }
                    }
                    Err(e) => {
                        // ContentDependent means trim_whitespace is in the querystring.
                        // Strip trim keys and retry, adding CropWhitespace node before layout.
                        let parsed = Ir4Command::QueryString(value.clone()).parse();
                        if let Ok(ref ir4_result) = parsed {
                            if ir4_result.parsed.trim_whitespace_threshold.is_some() {
                                // Retry without trim by building a new querystring
                                let qs_without_trim = strip_trim_from_qs(value);
                                let expand2 = Ir4Expand {
                                    i: Ir4Command::QueryString(qs_without_trim),
                                    source: Ir4SourceFrameInfo {
                                        w: source_w,
                                        h: source_h,
                                        fmt: imageflow_types::PixelFormat::Bgra32,
                                        original_mime: source_mime.clone(),
                                        lossless: source_lossless,
                                    },
                                    reference_width: source_w,
                                    reference_height: source_h,
                                    encode_id: *encode,
                                    watermarks: watermarks.clone(),
                                };
                                // Add CropWhitespace before any resize.
                                let threshold = ir4_result.parsed.trim_whitespace_threshold.unwrap_or(80) as u32;
                                let padding = ir4_result.parsed.trim_whitespace_padding_percent.unwrap_or(0.0);
                                result.push(Node::CropWhitespace {
                                    threshold,
                                    percent_padding: padding,
                                });
                                match expand2.expand_steps() {
                                    Ok(ir4_result) => {
                                        if let Some(expanded_steps) = ir4_result.steps {
                                            result.extend(expanded_steps);
                                        }
                                    }
                                    Err(e2) => {
                                        return Err(ZenError::Translate(TranslateError::InvalidParam(
                                            format!("RIAPI expansion (post-trim strip): {e2:?}"),
                                        )));
                                    }
                                }
                                continue;
                            }
                        }
                        return Err(ZenError::Translate(TranslateError::InvalidParam(
                            format!("RIAPI expansion: {e:?}"),
                        )));
                    }
                }
            }
            other => result.push(other.clone()),
        }
    }

    Ok(result)
}

/// Strip trim-related keys from a RIAPI querystring so expansion can proceed.
fn strip_trim_from_qs(qs: &str) -> String {
    qs.split('&')
        .filter(|part| {
            let key = part.split('=').next().unwrap_or("");
            !key.eq_ignore_ascii_case("s.trimwhitespace")
                && !key.eq_ignore_ascii_case("s.trim.threshold")
                && !key.eq_ignore_ascii_case("s.trim.percentpadding")
                && !key.eq_ignore_ascii_case("trim.threshold")
                && !key.eq_ignore_ascii_case("trim.percentpadding")
        })
        .collect::<Vec<_>>()
        .join("&")
}

impl TranslatedPipeline {
    pub(crate) fn clone_config(&self) -> TranslatedPipeline {
        TranslatedPipeline {
            nodes: Vec::new(),
            preset: self.preset.as_ref().map(|p| super::preset_map::PresetMapping {
                intent: p.intent.clone(),
                explicit_format: p.explicit_format,
            }),
            decode_io_id: self.decode_io_id,
            encode_io_id: self.encode_io_id,
            decoder_commands: self.decoder_commands.clone(),
            create_canvas: self.create_canvas.clone(),
        }
    }
}
