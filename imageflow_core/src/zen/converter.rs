//! NodeConverter implementations for zenfilters and zenlayout nodes.
//!
//! The zenpipe bridge has built-in converters for core geometry and resize nodes
//! but doesn't know about zenfilters or zenlayout.expand_canvas. These converters
//! bridge the gap.

use zennode::NodeInstance;
use zenpipe::bridge::NodeConverter;
use zenpipe::graph::NodeOp;
use zenpipe::PipeError;

/// Converter for `zenfilters.*` nodes → `NodeOp::Filter(pipeline)`.
///
/// Converts zenfilters NodeInstance types to a zenfilters::Pipeline wrapped in
/// NodeOp::Filter. Handles single nodes and fused groups.
pub struct ZenFiltersConverter;

impl NodeConverter for ZenFiltersConverter {
    fn can_convert(&self, schema_id: &str) -> bool {
        zenfilters::zennode_defs::is_zenfilters_node(schema_id)
    }

    fn convert(&self, node: &dyn NodeInstance) -> Result<NodeOp, PipeError> {
        let filter = zenfilters::zennode_defs::node_to_filter(node).ok_or_else(|| {
            PipeError::Op(format!("zenfilters converter: unrecognized node '{}'", node.schema().id))
        })?;

        let mut pipeline = zenfilters::Pipeline::new(zenfilters::PipelineConfig::default())
            .map_err(|e| PipeError::Op(format!("zenfilters pipeline creation failed: {e:?}")))?;
        pipeline.push(filter);
        Ok(NodeOp::Filter(pipeline))
    }

    fn convert_group(&self, nodes: &[&dyn NodeInstance]) -> Result<NodeOp, PipeError> {
        let mut pipeline = zenfilters::Pipeline::new(zenfilters::PipelineConfig::default())
            .map_err(|e| PipeError::Op(format!("zenfilters pipeline creation failed: {e:?}")))?;

        for node in nodes {
            let filter = zenfilters::zennode_defs::node_to_filter(*node).ok_or_else(|| {
                PipeError::Op(format!(
                    "zenfilters converter: unrecognized node '{}'",
                    node.schema().id
                ))
            })?;
            pipeline.push(filter);
        }

        Ok(NodeOp::Filter(pipeline))
    }

    fn fuse_group(&self, nodes: &[&dyn NodeInstance]) -> Result<Option<NodeOp>, PipeError> {
        // All zenfilters nodes in the fused_adjust coalesce group can be fused
        // into a single pipeline. Try to build a pipeline from all nodes.
        if nodes.len() < 2 {
            return Ok(None);
        }

        let mut pipeline = zenfilters::Pipeline::new(zenfilters::PipelineConfig::default())
            .map_err(|e| PipeError::Op(format!("zenfilters pipeline creation failed: {e:?}")))?;

        for node in nodes {
            if let Some(filter) = zenfilters::zennode_defs::node_to_filter(*node) {
                pipeline.push(filter);
            } else {
                return Ok(None); // Can't fuse if any node is unknown
            }
        }

        Ok(Some(NodeOp::Filter(pipeline)))
    }
}

/// Converter for `zenlayout.expand_canvas` → `NodeOp::ExpandCanvas`.
pub struct ExpandCanvasConverter;

impl NodeConverter for ExpandCanvasConverter {
    fn can_convert(&self, schema_id: &str) -> bool {
        schema_id == "zenlayout.expand_canvas"
    }

    fn convert(&self, node: &dyn NodeInstance) -> Result<NodeOp, PipeError> {
        use zennode::ParamValue;
        let get_u32 = |name| match node.get_param(name) {
            Some(ParamValue::U32(v)) => v,
            _ => 0,
        };
        let left = get_u32("left");
        let top = get_u32("top");
        let right = get_u32("right");
        let bottom = get_u32("bottom");

        // Extract background color from the node's `color` string param.
        // The zenlayout.expand_canvas node stores color as a CSS-style string:
        // "transparent", "white", "black", or "#RRGGBB" / "#RRGGBBAA".
        let bg_color = match node.get_param("color") {
            Some(ParamValue::Str(ref s)) => parse_css_color(s),
            _ => [0u8, 0, 0, 0], // transparent (default)
        };

        Ok(NodeOp::ExpandCanvas { left, top, right, bottom, bg_color })
    }

    fn convert_group(&self, nodes: &[&dyn NodeInstance]) -> Result<NodeOp, PipeError> {
        if let Some(node) = nodes.first() {
            self.convert(*node)
        } else {
            Err(PipeError::Op("empty expand_canvas group".into()))
        }
    }
}

// Color parsing delegated to super::color module.
use super::color::parse_css_color;

/// Converter for `zenlayout.region` — viewport with crop + expand.
///
/// Region defines a viewport in source coordinates. Negative coords extend
/// beyond the source (padding), positive coords within the source (crop).
/// Output is a Materialize that places the source onto a canvas at the
/// correct offset using ExpandCanvasSource.
pub struct RegionConverter;

impl NodeConverter for RegionConverter {
    fn can_convert(&self, schema_id: &str) -> bool {
        schema_id == "zenlayout.region"
    }

    fn convert(&self, node: &dyn NodeInstance) -> Result<NodeOp, PipeError> {
        use zennode::ParamValue;
        let x1 = match node.get_param("x1") {
            Some(ParamValue::I32(v)) => v,
            _ => 0,
        };
        let y1 = match node.get_param("y1") {
            Some(ParamValue::I32(v)) => v,
            _ => 0,
        };
        let x2 = match node.get_param("x2") {
            Some(ParamValue::I32(v)) => v,
            _ => 0,
        };
        let y2 = match node.get_param("y2") {
            Some(ParamValue::I32(v)) => v,
            _ => 0,
        };

        // Region(x1,y1,x2,y2) defines a viewport. The output canvas is
        // (x2-x1) × (y2-y1) pixels. The source image is placed at (-x1,-y1)
        // in the canvas. ExpandCanvasSource handles clipping and padding.
        let canvas_w = (x2 - x1).max(1) as u32;
        let canvas_h = (y2 - y1).max(1) as u32;
        let place_x = -x1; // source's (0,0) goes to canvas position (-x1, -y1)
        let place_y = -y1;

        // Use Materialize to apply the region viewport via ExpandCanvasSource.
        // ExpandCanvasSource handles negative placement (crop) and positive
        // placement (padding) transparently.
        Ok(NodeOp::Materialize {
            label: "region_viewport",
            transform: Box::new(
                move |data: &mut Vec<u8>,
                      w: &mut u32,
                      h: &mut u32,
                      fmt: &mut zenpipe::PixelFormat| {
                    let src_w = *w;
                    let src_h = *h;
                    let bpp = fmt.bytes_per_pixel();
                    let src_stride = src_w as usize * bpp;

                    // Build the output canvas
                    let out_stride = canvas_w as usize * bpp;
                    let mut out = vec![0u8; canvas_h as usize * out_stride];

                    for out_y in 0..canvas_h as i32 {
                        let src_y = out_y - place_y;
                        if src_y < 0 || src_y >= src_h as i32 {
                            continue;
                        }
                        for out_x in 0..canvas_w as i32 {
                            let src_x = out_x - place_x;
                            if src_x < 0 || src_x >= src_w as i32 {
                                continue;
                            }
                            let src_off = src_y as usize * src_stride + src_x as usize * bpp;
                            let dst_off = out_y as usize * out_stride + out_x as usize * bpp;
                            if src_off + bpp <= data.len() && dst_off + bpp <= out.len() {
                                out[dst_off..dst_off + bpp]
                                    .copy_from_slice(&data[src_off..src_off + bpp]);
                            }
                        }
                    }

                    *data = out;
                    *w = canvas_w;
                    *h = canvas_h;
                },
            ),
        })
    }

    fn convert_group(&self, nodes: &[&dyn NodeInstance]) -> Result<NodeOp, PipeError> {
        if let Some(node) = nodes.first() {
            self.convert(*node)
        } else {
            Err(PipeError::Op("empty region group".into()))
        }
    }
}

/// Converter for `imageflow.white_balance_srgb` → `NodeOp::Materialize`.
///
/// Implements automatic white balance correction via histogram analysis in sRGB
/// space. Materializes the full frame, builds per-channel histograms, finds
/// the threshold percentile boundaries, then applies a linear stretch mapping
/// to normalize each channel independently.
///
/// Algorithm matches the v2 `WhiteBalanceHistogramAreaThresholdSrgb` node.
pub struct WhiteBalanceConverter;

impl NodeConverter for WhiteBalanceConverter {
    fn can_convert(&self, schema_id: &str) -> bool {
        schema_id == "imageflow.white_balance_srgb"
    }

    fn convert(&self, node: &dyn NodeInstance) -> Result<NodeOp, PipeError> {
        use zennode::ParamValue;
        let threshold = match node.get_param("threshold") {
            Some(ParamValue::F32(v)) => v as f64,
            _ => 0.006,
        };

        Ok(NodeOp::Materialize {
            label: "white_balance",
            transform: Box::new(
                move |data: &mut Vec<u8>,
                      w: &mut u32,
                      h: &mut u32,
                      fmt: &mut zenpipe::PixelFormat| {
                    let width = *w as usize;
                    let height = *h as usize;
                    let bpp = fmt.bytes_per_pixel();

                    if bpp < 3 {
                        return; // grayscale not supported, skip
                    }

                    let total_pixels = (width * height) as u64;
                    if total_pixels == 0 {
                        return;
                    }

                    // Build per-channel histograms (R, G, B).
                    let mut hist_r = [0u64; 256];
                    let mut hist_g = [0u64; 256];
                    let mut hist_b = [0u64; 256];

                    for y in 0..height {
                        let row_start = y * width * bpp;
                        for x in 0..width {
                            let off = row_start + x * bpp;
                            hist_r[data[off] as usize] += 1;
                            hist_g[data[off + 1] as usize] += 1;
                            hist_b[data[off + 2] as usize] += 1;
                        }
                    }

                    let (r_low, r_high) = area_threshold(&hist_r, total_pixels, threshold);
                    let (g_low, g_high) = area_threshold(&hist_g, total_pixels, threshold);
                    let (b_low, b_high) = area_threshold(&hist_b, total_pixels, threshold);

                    let map_r = create_byte_mapping(r_low, r_high);
                    let map_g = create_byte_mapping(g_low, g_high);
                    let map_b = create_byte_mapping(b_low, b_high);

                    for y in 0..height {
                        let row_start = y * width * bpp;
                        for x in 0..width {
                            let off = row_start + x * bpp;
                            data[off] = map_r[data[off] as usize];
                            data[off + 1] = map_g[data[off + 1] as usize];
                            data[off + 2] = map_b[data[off + 2] as usize];
                        }
                    }
                },
            ),
        })
    }

    fn convert_group(&self, nodes: &[&dyn NodeInstance]) -> Result<NodeOp, PipeError> {
        if let Some(node) = nodes.first() {
            self.convert(*node)
        } else {
            Err(PipeError::Op("empty white_balance group".into()))
        }
    }
}

/// Find histogram low/high boundary indices by scanning from both ends
/// until cumulative area exceeds the threshold fraction.
fn area_threshold(histogram: &[u64; 256], total_pixels: u64, threshold: f64) -> (usize, usize) {
    let pixel_count = total_pixels as f64;

    let mut low = 0;
    let mut area = 0u64;
    for (ix, &value) in histogram.iter().enumerate() {
        area += value;
        if area as f64 / pixel_count > threshold {
            low = ix;
            break;
        }
    }

    let mut high = 255;
    area = 0;
    for (ix, &value) in histogram.iter().enumerate().rev() {
        area += value;
        if area as f64 / pixel_count > threshold {
            high = ix;
            break;
        }
    }

    (low, high)
}

/// Create a 256-entry byte lookup table mapping [low, high] → [0, 255].
#[allow(clippy::manual_clamp)]
fn create_byte_mapping(low: usize, high: usize) -> [u8; 256] {
    let mut map = [0u8; 256];
    let range = if high > low { high - low } else { 1 };
    let scale = 255.0 / range as f64;
    for v in 0..256usize {
        map[v] = (v.saturating_sub(low) as f64 * scale).round().min(255.0).max(0.0) as u8;
    }
    map
}

/// Converter for `imageflow.color_matrix_srgb` → `NodeOp::Materialize`.
///
/// Applies a 5×5 color matrix in sRGB gamma space (u8 values), matching v2 behavior.
pub struct ColorMatrixSrgbConverter;

impl NodeConverter for ColorMatrixSrgbConverter {
    fn can_convert(&self, schema_id: &str) -> bool {
        schema_id == "imageflow.color_matrix_srgb"
    }

    fn convert(&self, node: &dyn NodeInstance) -> Result<NodeOp, PipeError> {
        use zennode::ParamValue;
        let matrix: [f32; 25] = match node.get_param("matrix") {
            Some(ParamValue::F32Array(v)) if v.len() == 25 => {
                let mut arr = [0.0f32; 25];
                arr.copy_from_slice(&v);
                arr
            }
            _ => {
                return Err(PipeError::Op(
                    "color_matrix_srgb: missing or invalid matrix param".into(),
                ))
            }
        };

        Ok(NodeOp::Materialize {
            label: "color_matrix_srgb",
            transform: Box::new(
                move |data: &mut Vec<u8>,
                      w: &mut u32,
                      h: &mut u32,
                      fmt: &mut zenpipe::PixelFormat| {
                    let width = *w as usize;
                    let height = *h as usize;
                    let bpp = fmt.bytes_per_pixel();

                    if bpp < 4 {
                        return; // need RGBA
                    }

                    // Apply 5×5 matrix: out[c] = sum(m[c*5+i] * in[i], i=0..4) + m[c*5+4] * 255
                    for y in 0..height {
                        let row_start = y * width * bpp;
                        for x in 0..width {
                            let off = row_start + x * bpp;
                            let r = data[off] as f32;
                            let g = data[off + 1] as f32;
                            let b = data[off + 2] as f32;
                            let a = data[off + 3] as f32;

                            let out_r = matrix[0] * r
                                + matrix[1] * g
                                + matrix[2] * b
                                + matrix[3] * a
                                + matrix[4] * 255.0;
                            let out_g = matrix[5] * r
                                + matrix[6] * g
                                + matrix[7] * b
                                + matrix[8] * a
                                + matrix[9] * 255.0;
                            let out_b = matrix[10] * r
                                + matrix[11] * g
                                + matrix[12] * b
                                + matrix[13] * a
                                + matrix[14] * 255.0;
                            let out_a = matrix[15] * r
                                + matrix[16] * g
                                + matrix[17] * b
                                + matrix[18] * a
                                + matrix[19] * 255.0;

                            data[off] = out_r.round().min(255.0).max(0.0) as u8;
                            data[off + 1] = out_g.round().min(255.0).max(0.0) as u8;
                            data[off + 2] = out_b.round().min(255.0).max(0.0) as u8;
                            data[off + 3] = out_a.round().min(255.0).max(0.0) as u8;
                        }
                    }
                },
            ),
        })
    }

    fn convert_group(&self, nodes: &[&dyn NodeInstance]) -> Result<NodeOp, PipeError> {
        if let Some(node) = nodes.first() {
            self.convert(*node)
        } else {
            Err(PipeError::Op("empty color_matrix_srgb group".into()))
        }
    }
}

/// Build the standard set of converters for the imageflow zen pipeline.
///
/// Includes zenfilters, expand_canvas, region, and white_balance converters.
/// All other operations (fill_rect, crop_whitespace, remove_alpha,
/// round_corners, resize) are handled by zenpipe's built-in bridge converters.
pub fn imageflow_converters() -> [&'static dyn NodeConverter; 6] {
    static FILTERS: ZenFiltersConverter = ZenFiltersConverter;
    static EXPAND: ExpandCanvasConverter = ExpandCanvasConverter;
    static REGION: RegionConverter = RegionConverter;
    static WHITE_BAL: WhiteBalanceConverter = WhiteBalanceConverter;
    static COLOR_MAT: ColorMatrixSrgbConverter = ColorMatrixSrgbConverter;
    static WATERMARK: super::watermark::WatermarkConverter = super::watermark::WatermarkConverter;
    [&FILTERS, &EXPAND, &REGION, &WHITE_BAL, &COLOR_MAT, &WATERMARK]
}
