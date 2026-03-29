//! Imageflow-specific zennode definitions for the zen pipeline.
//!
//! These are operations that don't belong in any zen crate but need
//! zennode integration for the bridge/converter pattern.

use std::any::Any;
use zennode::*;

/// White balance via histogram area thresholding (sRGB space).
///
/// Materializes the upstream image, builds per-channel histograms,
/// finds the low/high thresholds per channel, then applies a linear
/// mapping to stretch the histogram range to [0, 255].
///
/// The threshold (0.0-1.0) controls what fraction of the histogram
/// area is clipped at each end. Default 0.006 (0.6%).
#[derive(Node, Clone, Debug, Default)]
#[node(id = "imageflow.white_balance_srgb", group = Tone, role = Filter)]
#[node(tags("white_balance", "histogram", "color", "auto"))]
pub struct WhiteBalanceSrgb {
    /// Histogram area threshold (0.0-1.0).
    ///
    /// Fraction of total pixels clipped at each end of the histogram.
    /// Lower values preserve more dynamic range; higher values clip more.
    /// Default: 0.006 (0.6%).
    #[param(range(0.0..=1.0), default = 0.006, step = 0.001)]
    #[param(section = "Main", label = "Threshold")]
    pub threshold: f32,
}

/// 5×5 color matrix applied in sRGB gamma space.
///
/// Each output channel is computed as:
///   out[c] = clamp(sum(matrix[c*5+i] * in[i] for i in 0..4) + matrix[c*5+4] * 255)
///
/// This matches v2's ColorMatrixSrgb behavior — the matrix operates on u8 values
/// in sRGB gamma space, NOT in linear light or perceptual color spaces.
///
/// Implemented as a custom [`NodeInstance`] rather than `#[derive(Node)]` because
/// the zennode derive doesn't support `Vec<f32>` params.
#[derive(Clone)]
pub struct ColorMatrixSrgbNode {
    pub matrix: [f32; 25],
}

static COLOR_MATRIX_SRGB_SCHEMA: NodeSchema = NodeSchema {
    id: "imageflow.color_matrix_srgb",
    label: "Color Matrix (sRGB)",
    description: "5x5 color matrix applied in sRGB gamma space (u8 values)",
    group: NodeGroup::Tone,
    role: NodeRole::Filter,
    format: FormatHint {
        preferred: PixelFormatPreference::Srgb8,
        alpha: AlphaHandling::Process,
        changes_dimensions: false,
        is_neighborhood: false,
    },
    tags: &["color_matrix", "color", "srgb"],
    inputs: &[],
    params: &[],
    coalesce: None,
    version: 1,
    compat_version: 1,
    json_key: "",
    deny_unknown_fields: false,
};

impl NodeInstance for ColorMatrixSrgbNode {
    fn schema(&self) -> &'static NodeSchema {
        &COLOR_MATRIX_SRGB_SCHEMA
    }

    fn to_params(&self) -> ParamMap {
        let mut map = ParamMap::new();
        map.insert("matrix".into(), ParamValue::F32Array(self.matrix.to_vec()));
        map
    }

    fn get_param(&self, name: &str) -> Option<ParamValue> {
        if name == "matrix" {
            Some(ParamValue::F32Array(self.matrix.to_vec()))
        } else {
            None
        }
    }

    fn set_param(&mut self, name: &str, value: ParamValue) -> bool {
        if name == "matrix" {
            if let ParamValue::F32Array(v) = value {
                if v.len() == 25 {
                    self.matrix.copy_from_slice(&v);
                    return true;
                }
            }
        }
        false
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn clone_boxed(&self) -> Box<dyn NodeInstance> {
        Box::new(self.clone())
    }
}

/// Register imageflow-specific node definitions into a registry.
pub fn register(registry: &mut NodeRegistry) {
    registry.register(&WHITE_BALANCE_SRGB_NODE);
}
