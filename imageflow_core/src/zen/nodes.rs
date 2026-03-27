//! Imageflow-specific zennode definitions for the zen pipeline.
//!
//! These are operations that don't belong in any zen crate but need
//! zennode integration for the bridge/converter pattern.

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

/// Register imageflow-specific node definitions into a registry.
pub fn register(registry: &mut NodeRegistry) {
    registry.register(&WHITE_BALANCE_SRGB_NODE);
}
