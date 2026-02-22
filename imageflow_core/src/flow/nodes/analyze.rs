use super::internal_prelude::*;
use crate::graphics::bitmaps::BitmapKey;

pub static ANALYZE: AnalyzeDef = AnalyzeDef {};

#[derive(Debug, Clone)]
pub struct AnalyzeDef;

impl NodeDef for AnalyzeDef {
    fn fqn(&self) -> &'static str {
        "imazen.analyze"
    }

    fn edges_required(&self, _p: &NodeParams) -> Result<(EdgesIn, EdgesOut)> {
        Ok((EdgesIn::OneInput, EdgesOut::Any))
    }

    fn validate_params(&self, p: &NodeParams) -> Result<()> {
        if matches!(p, NodeParams::Json(s::Node::Analyze { .. })) {
            Ok(())
        } else {
            Err(nerror!(crate::ErrorKind::NodeParamsMismatch, "Need Analyze, got {:?}", p))
        }
    }

    fn estimate(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<FrameEstimate> {
        // Pass through the input estimate — analyze doesn't change frame dimensions
        ctx.frame_est_from(ix, EdgeKind::Input).map_err(|e| e.at(here!()))
    }

    fn can_execute(&self) -> bool {
        true
    }

    fn execute(&self, ctx: &mut OpCtxMut, ix: NodeIndex) -> Result<NodeResult> {
        let bitmap_key = ctx.bitmap_key_from(ix, EdgeKind::Input).map_err(|e| e.at(here!()))?;

        let mode = match ctx.weight(ix).params {
            NodeParams::Json(s::Node::Analyze { mode }) => mode,
            ref other => {
                return Err(nerror!(
                    crate::ErrorKind::NodeParamsMismatch,
                    "Need Analyze, got {:?}",
                    other
                ));
            }
        };

        let now = std::time::Instant::now();

        // Access bitmap pixel data
        let (width, height, pixels) = {
            let bitmaps = ctx.c.borrow_bitmaps().map_err(|e| e.at(here!()))?;
            let mut bitmap = bitmaps.try_borrow_mut(bitmap_key).map_err(|e| e.at(here!()))?;
            let w = bitmap.w();
            let h = bitmap.h();

            // Get BGRA pixel data as a flat u8 slice
            let window = bitmap.get_window_u8().ok_or_else(|| {
                nerror!(
                    crate::ErrorKind::InvalidState,
                    "Cannot get u8 window from bitmap for analysis"
                )
            })?;

            // Copy pixel data since we can't hold the borrow across the analysis call
            let stride = window.info().t_stride() as usize;
            let mut pixel_data = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h as usize {
                let row_start = y * stride;
                let row_end = row_start + (w as usize * 4);
                pixel_data.extend_from_slice(&window.get_slice()[row_start..row_end]);
            }
            (w, h, pixel_data)
        };

        let config = imageflow_focus::AnalysisConfig::default();

        let focus_regions = match mode {
            s::AnalyzeMode::Saliency | s::AnalyzeMode::Focus | s::AnalyzeMode::All => {
                imageflow_focus::analyze_saliency(&pixels, width, height, &config)
            }
            s::AnalyzeMode::Faces => {
                // Face detection requires the `faces` feature and a model file.
                // Without it, return empty results.
                vec![]
            }
        };

        let elapsed_ms = now.elapsed().as_millis() as u64;

        Ok(NodeResult::Analyzed(s::AnalyzeResult {
            focus_regions,
            image_width: width,
            image_height: height,
            analysis_ms: elapsed_ms,
        }))
    }
}
