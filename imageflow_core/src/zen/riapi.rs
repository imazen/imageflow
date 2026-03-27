//! RIAPI querystring expansion — dual implementation.
//!
//! Two paths for expanding RIAPI querystrings into zennode instances:
//!
//! - **Legacy path** (`expand_legacy`): Uses `imageflow_riapi::ir4::Ir4Expand` to parse
//!   the full IR4 querystring and produce v2 `Node` steps, then translates via `translate.rs`.
//!   Battle-tested v2-compatible path with full 68-key coverage.
//!
//! - **Zen-native path** (`expand_zen`): Uses `zenlayout::riapi::parse()` for geometry,
//!   then feeds non-geometry keys through `zennode::NodeRegistry::from_querystring()`
//!   so each codec/filter crate handles its own keys. Modular and extensible.
//!
//! Both produce `Vec<Box<dyn NodeInstance>>` that zenpipe can execute.

use zennode::{NodeDef, NodeInstance, NodeRegistry};

use super::preset_map::PresetMapping;
use super::translate::{self, TranslateError};

/// Which RIAPI parser to use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RiapiEngine {
    /// Use imageflow_riapi (legacy v2 parser, full coverage).
    #[default]
    Legacy,
    /// Use zenlayout::riapi + zennode registry (modular zen-native parser).
    ZenNative,
}

/// Result of expanding a RIAPI querystring.
pub struct ExpandedRiapi {
    /// Zenode instances for pixel-processing operations.
    pub nodes: Vec<Box<dyn NodeInstance>>,
    /// Encoder configuration derived from format/quality keys.
    pub preset: Option<PresetMapping>,
    /// Warnings from parsing (unknown keys, deprecated keys, etc.).
    pub warnings: Vec<String>,
}

/// Expand a RIAPI querystring into zennode instances using the legacy parser.
///
/// Uses `imageflow_riapi::ir4::Ir4Expand` to parse the full IR4 querystring,
/// produces v2 `Node` steps (with source dimensions for layout), then translates
/// to zennode instances.
///
/// # Arguments
/// * `querystring` — the raw querystring (without leading `?`)
/// * `source_w` — source image width (needed for layout computation)
/// * `source_h` — source image height
/// * `source_mime` — source MIME type (for format auto-detection)
/// * `source_lossless` — whether source is lossless
/// * `exif_flag` — EXIF orientation flag (1-8)
/// * `encode_io_id` — io_id for the encode output (None if not encoding)
pub fn expand_legacy(
    querystring: &str,
    source_w: i32,
    source_h: i32,
    source_mime: Option<&str>,
    source_lossless: bool,
    exif_flag: u8,
    encode_io_id: Option<i32>,
) -> Result<ExpandedRiapi, TranslateError> {
    use imageflow_riapi::ir4::*;
    use imageflow_types as s;

    let command = Ir4Command::QueryString(querystring.to_string());

    let expand = Ir4Expand {
        i: command,
        source: Ir4SourceFrameInfo {
            w: source_w,
            h: source_h,
            fmt: s::PixelFormat::Bgra32,
            original_mime: source_mime.map(|s| s.to_string()),
            lossless: source_lossless,
        },
        reference_width: source_w,
        reference_height: source_h,
        encode_id: encode_io_id,
        watermarks: None,
    };

    let result = expand
        .expand_steps()
        .map_err(|e| TranslateError::InvalidParam(format!("RIAPI expansion error: {e:?}")))?;

    let steps = result.steps.unwrap_or_default();
    let mut warnings: Vec<String> =
        result.parse_warnings.iter().map(|w| format!("{w:?}")).collect();

    // Translate v2 Node steps → zennode instances.
    // RIAPI path never produces Watermark nodes, so pass empty io_buffers.
    let pipeline = translate::translate_nodes(&steps, &std::collections::HashMap::new())?;

    Ok(ExpandedRiapi { nodes: pipeline.nodes, preset: pipeline.preset, warnings })
}

/// Expand a RIAPI querystring using the zen-native parser.
///
/// Uses `zenlayout::riapi::parse()` for geometry, then feeds remaining
/// keys through zennode registry for filter/codec nodes.
///
/// This is the modular path — each crate only handles its own keys.
/// Currently handles fewer keys than `expand_legacy` but is more extensible.
pub fn expand_zen(
    querystring: &str,
    source_w: u32,
    source_h: u32,
    exif_flag: Option<u8>,
) -> Result<ExpandedRiapi, TranslateError> {
    // 1. Build a registry with all zen-native nodes.
    let mut registry = NodeRegistry::new();
    zenlayout::zennode_defs::register(&mut registry);
    zenresize::zennode_defs::register(&mut registry);
    zenfilters::zennode_defs::register_all(&mut registry);
    // zencodecs quality intent node — auto-generated name from derive.
    registry.register(&zencodecs::zennode_defs::QUALITY_INTENT_NODE_NODE);

    // 2. Parse via zennode's unified querystring dispatch.
    // Each registered node claims its own keys via #[kv(...)] annotations.
    let kv_result = registry.from_querystring(querystring);

    let mut nodes: Vec<Box<dyn NodeInstance>> = Vec::new();
    let mut preset = None;
    let mut warnings: Vec<String> = Vec::new();

    for w in &kv_result.warnings {
        warnings.push(format!("{}: {}", w.key, w.message));
    }

    // 3. Separate pixel-processing nodes from codec intent nodes.
    for inst in kv_result.instances {
        let schema_id = inst.schema().id;
        if schema_id == "zencodecs.quality_intent" {
            // Extract codec intent from QualityIntentNode.
            if let Some(qin) =
                inst.as_any().downcast_ref::<zencodecs::zennode_defs::QualityIntentNode>()
            {
                let intent = qin.to_codec_intent();
                preset = Some(PresetMapping {
                    intent: intent.clone(),
                    explicit_format: match &intent.format {
                        Some(zencodecs::FormatChoice::Specific(f)) => Some(*f),
                        _ => None,
                    },
                });
            }
        } else {
            nodes.push(inst);
        }
    }

    Ok(ExpandedRiapi { nodes, preset, warnings })
}
