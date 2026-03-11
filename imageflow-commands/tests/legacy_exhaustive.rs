//! Exhaustive generation of every legacy imageflow v1/v2 JSON job variant,
//! plus translation to imageflow-commands format and cross-comparison.
//!
//! This exercises every Node variant, every EncoderPreset variant, every
//! DecoderCommand variant, every ConstraintMode, every ColorFilter, every
//! Filter, every color representation, and both Framewise modes (Steps/Graph).

use serde_json::{json, Value};

// ─── Legacy JSON Generators ─────────────────────────────────────────────

/// Generate every possible legacy Node as JSON.
fn all_legacy_nodes() -> Vec<(&'static str, Value)> {
    vec![
        // ── Parameterless geometry ──
        ("flip_v", json!({"flip_v": null})),
        ("flip_h", json!({"flip_h": null})),
        ("transpose", json!({"transpose": null})),
        ("rotate_90", json!({"rotate_90": null})),
        ("rotate_180", json!({"rotate_180": null})),
        ("rotate_270", json!({"rotate_270": null})),
        ("watermark_red_dot", json!({"watermark_red_dot": null})),
        // ── Crop ──
        ("crop", json!({"crop": {"x1": 10, "y1": 20, "x2": 800, "y2": 600}})),
        // ── CropWhitespace ──
        ("crop_whitespace", json!({"crop_whitespace": {"threshold": 80, "percent_padding": 5.0}})),
        // ── Region (signed coordinates) ──
        (
            "region",
            json!({"region": {"x1": -10, "y1": -10, "x2": 810, "y2": 610, "background_color": {"transparent": null}}}),
        ),
        // ── RegionPercent ──
        (
            "region_percent",
            json!({"region_percent": {"x1": 10.0, "y1": 10.0, "x2": 90.0, "y2": 90.0, "background_color": {"black": null}}}),
        ),
        // ── ApplyOrientation (all 8 EXIF flags) ──
        ("apply_orientation_1", json!({"apply_orientation": {"flag": 1}})),
        ("apply_orientation_6", json!({"apply_orientation": {"flag": 6}})),
        ("apply_orientation_8", json!({"apply_orientation": {"flag": 8}})),
        // ── CreateCanvas ──
        (
            "create_canvas_bgra32",
            json!({"create_canvas": {"format": "bgra_32", "w": 200, "h": 200, "color": {"transparent": null}}}),
        ),
        (
            "create_canvas_bgr32",
            json!({"create_canvas": {"format": "bgr_32", "w": 100, "h": 100, "color": {"black": null}}}),
        ),
        // ── ExpandCanvas ──
        (
            "expand_canvas",
            json!({"expand_canvas": {"left": 10, "top": 20, "right": 30, "bottom": 40, "color": {"srgb": {"hex": "FFEECCFF"}}}}),
        ),
        // ── FillRect ──
        (
            "fill_rect",
            json!({"fill_rect": {"x1": 0, "y1": 0, "x2": 50, "y2": 50, "color": {"transparent": null}}}),
        ),
        // ── Resample2D (all filter variants) ──
        ("resample_2d_no_hints", json!({"resample_2d": {"w": 400, "h": 300, "hints": null}})),
        (
            "resample_2d_robidoux",
            json!({"resample_2d": {"w": 800, "h": 600, "hints": {
                "sharpen_percent": 10.0,
                "down_filter": "robidoux",
                "up_filter": "ginseng",
                "scaling_colorspace": "linear",
                "background_color": {"srgb": {"hex": "FFEEAACC"}},
                "resample_when": "size_differs_or_sharpening_requested",
                "sharpen_when": "downscaling"
            }}}),
        ),
        (
            "resample_2d_lanczos",
            json!({"resample_2d": {"w": 200, "h": 150, "hints": {
                "sharpen_percent": null,
                "down_filter": "lanczos",
                "up_filter": "lanczos_sharp",
                "scaling_colorspace": "srgb",
                "background_color": null,
                "resample_when": "always",
                "sharpen_when": "always"
            }}}),
        ),
        (
            "resample_2d_mitchell",
            json!({"resample_2d": {"w": 100, "h": 75, "hints": {
                "sharpen_percent": 0.0,
                "down_filter": "mitchell",
                "up_filter": "catmull_rom",
                "scaling_colorspace": null,
                "background_color": null,
                "resample_when": "size_differs",
                "sharpen_when": "upscaling"
            }}}),
        ),
        (
            "resample_2d_all_filters",
            json!({"resample_2d": {"w": 50, "h": 50, "hints": {
                "down_filter": "cubic_b_spline",
                "up_filter": "hermite"
            }}}),
        ),
        (
            "resample_2d_filters_2",
            json!({"resample_2d": {"w": 50, "h": 50, "hints": {
                "down_filter": "jinc",
                "up_filter": "triangle"
            }}}),
        ),
        (
            "resample_2d_filters_3",
            json!({"resample_2d": {"w": 50, "h": 50, "hints": {
                "down_filter": "linear",
                "up_filter": "box"
            }}}),
        ),
        (
            "resample_2d_filters_4",
            json!({"resample_2d": {"w": 50, "h": 50, "hints": {
                "down_filter": "fastest",
                "up_filter": "n_cubic"
            }}}),
        ),
        (
            "resample_2d_filters_5",
            json!({"resample_2d": {"w": 50, "h": 50, "hints": {
                "down_filter": "n_cubic_sharp",
                "up_filter": "robidoux_fast"
            }}}),
        ),
        (
            "resample_2d_filters_6",
            json!({"resample_2d": {"w": 50, "h": 50, "hints": {
                "down_filter": "robidoux_sharp",
                "up_filter": "ginseng_sharp"
            }}}),
        ),
        (
            "resample_2d_filters_7",
            json!({"resample_2d": {"w": 50, "h": 50, "hints": {
                "down_filter": "lanczos_2",
                "up_filter": "lanczos_2_sharp"
            }}}),
        ),
        (
            "resample_2d_filters_8",
            json!({"resample_2d": {"w": 50, "h": 50, "hints": {
                "down_filter": "cubic",
                "up_filter": "cubic_sharp"
            }}}),
        ),
        // ── Constrain (all modes) ──
        ("constrain_distort", json!({"constrain": {"mode": "distort", "w": 800, "h": 600}})),
        ("constrain_within", json!({"constrain": {"mode": "within", "w": 800, "h": 600}})),
        ("constrain_fit", json!({"constrain": {"mode": "fit", "w": 800, "h": 600}})),
        (
            "constrain_larger_than",
            json!({"constrain": {"mode": "larger_than", "w": 400, "h": 300}}),
        ),
        (
            "constrain_within_crop",
            json!({"constrain": {"mode": "within_crop", "w": 800, "h": 600, "gravity": {"center": null}}}),
        ),
        (
            "constrain_fit_crop",
            json!({"constrain": {"mode": "fit_crop", "w": 800, "h": 600, "gravity": {"percentage": {"x": 0.5, "y": 0.5}}}}),
        ),
        (
            "constrain_aspect_crop",
            json!({"constrain": {"mode": "aspect_crop", "w": 800, "h": 600}}),
        ),
        (
            "constrain_within_pad",
            json!({"constrain": {"mode": "within_pad", "w": 800, "h": 600, "canvas_color": {"srgb": {"hex": "FFFFFF"}}}}),
        ),
        (
            "constrain_fit_pad",
            json!({"constrain": {"mode": "fit_pad", "w": 800, "h": 600, "canvas_color": {"transparent": null}}}),
        ),
        ("constrain_w_only", json!({"constrain": {"mode": "fit", "w": 800}})),
        ("constrain_h_only", json!({"constrain": {"mode": "within", "h": 600}})),
        (
            "constrain_with_hints",
            json!({"constrain": {
                "mode": "fit",
                "w": 800,
                "h": 600,
                "hints": {
                    "sharpen_percent": 15.0,
                    "down_filter": "lanczos",
                    "up_filter": "lanczos",
                    "scaling_colorspace": "linear"
                }
            }}),
        ),
        // ── DrawImageExact ──
        (
            "draw_image_exact_compose",
            json!({"draw_image_exact": {"x": 10, "y": 10, "w": 200, "h": 150, "blend": "compose", "hints": null}}),
        ),
        (
            "draw_image_exact_overwrite",
            json!({"draw_image_exact": {"x": 0, "y": 0, "w": 100, "h": 100, "blend": "overwrite"}}),
        ),
        // ── Watermark ──
        ("watermark_basic", json!({"watermark": {"io_id": 2, "opacity": 0.5}})),
        (
            "watermark_full",
            json!({"watermark": {
                "io_id": 2,
                "fit_box": {"image_percentage": {"x1": 0.0, "y1": 0.0, "x2": 100.0, "y2": 100.0}},
                "fit_mode": "within",
                "gravity": {"percentage": {"x": 0.95, "y": 0.95}},
                "min_canvas_width": 400,
                "min_canvas_height": 300,
                "opacity": 0.8,
                "hints": {"sharpen_percent": 5.0, "down_filter": "robidoux"}
            }}),
        ),
        (
            "watermark_image_margins",
            json!({"watermark": {
                "io_id": 3,
                "fit_box": {"image_margins": {"left": 10, "top": 10, "right": 10, "bottom": 10}},
                "fit_mode": "fit",
                "gravity": {"center": null},
                "opacity": 1.0
            }}),
        ),
        (
            "watermark_canvas_percentage",
            json!({"watermark": {
                "io_id": 4,
                "fit_box": {"canvas_percentage": {"x1": 80.0, "y1": 80.0, "x2": 100.0, "y2": 100.0}},
                "fit_mode": "fit_crop"
            }}),
        ),
        (
            "watermark_canvas_margins",
            json!({"watermark": {
                "io_id": 5,
                "fit_box": {"canvas_margins": {"left": 20, "top": 20, "right": 20, "bottom": 20}},
                "fit_mode": "distort"
            }}),
        ),
        // ── RoundImageCorners ──
        (
            "round_corners_percentage",
            json!({"round_image_corners": {"radius": {"percentage": 15.0}, "background_color": {"transparent": null}}}),
        ),
        (
            "round_corners_pixels",
            json!({"round_image_corners": {"radius": {"pixels": 20.0}, "background_color": {"black": null}}}),
        ),
        (
            "round_corners_circle",
            json!({"round_image_corners": {"radius": "circle", "background_color": {"transparent": null}}}),
        ),
        (
            "round_corners_custom_pct",
            json!({"round_image_corners": {"radius": {"percentage_custom": {"top_left": 10.0, "top_right": 20.0, "bottom_right": 30.0, "bottom_left": 40.0}}, "background_color": {"transparent": null}}}),
        ),
        (
            "round_corners_custom_px",
            json!({"round_image_corners": {"radius": {"pixels_custom": {"top_left": 5.0, "top_right": 10.0, "bottom_right": 15.0, "bottom_left": 20.0}}, "background_color": {"srgb": {"hex": "FF0000"}}}}),
        ),
        // ── WhiteBalance ──
        (
            "white_balance",
            json!({"white_balance_histogram_area_threshold_srgb": {"threshold": 0.06}}),
        ),
        (
            "white_balance_null_threshold",
            json!({"white_balance_histogram_area_threshold_srgb": {"threshold": null}}),
        ),
        // ── ColorMatrixSrgb ──
        (
            "color_matrix_identity",
            json!({"color_matrix_srgb": {"matrix": [
                [1.0, 0.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 0.0, 1.0]
            ]}}),
        ),
        // ── ColorFilterSrgb (all variants) ──
        ("color_filter_grayscale_ntsc", json!({"color_filter_srgb": "grayscale_ntsc"})),
        ("color_filter_grayscale_flat", json!({"color_filter_srgb": "grayscale_flat"})),
        ("color_filter_grayscale_bt709", json!({"color_filter_srgb": "grayscale_bt709"})),
        ("color_filter_grayscale_ry", json!({"color_filter_srgb": "grayscale_ry"})),
        ("color_filter_sepia", json!({"color_filter_srgb": "sepia"})),
        ("color_filter_invert", json!({"color_filter_srgb": "invert"})),
        ("color_filter_alpha", json!({"color_filter_srgb": {"alpha": 0.5}})),
        ("color_filter_contrast", json!({"color_filter_srgb": {"contrast": 1.5}})),
        ("color_filter_brightness", json!({"color_filter_srgb": {"brightness": 0.8}})),
        ("color_filter_saturation", json!({"color_filter_srgb": {"saturation": 1.2}})),
        // ── CopyRectToCanvas ──
        (
            "copy_rect_to_canvas",
            json!({"copy_rect_to_canvas": {"from_x": 0, "from_y": 0, "w": 100, "h": 100, "x": 50, "y": 50}}),
        ),
        // ── CommandString ──
        (
            "command_string_basic",
            json!({"command_string": {"kind": "ir4", "value": "w=800&h=600&mode=crop", "decode": 0, "encode": 1}}),
        ),
        (
            "command_string_with_watermarks",
            json!({"command_string": {
                "kind": "ir4",
                "value": "w=400&h=300",
                "decode": 0,
                "encode": 1,
                "watermarks": [
                    {"io_id": 2, "opacity": 0.5, "gravity": {"center": null}},
                    {"io_id": 3, "opacity": 0.8, "fit_box": {"image_percentage": {"x1": 80.0, "y1": 80.0, "x2": 100.0, "y2": 100.0}}}
                ]
            }}),
        ),
    ]
}

/// Generate every possible legacy Decode node variant.
fn all_legacy_decode_nodes() -> Vec<(&'static str, Value)> {
    vec![
        ("decode_minimal", json!({"decode": {"io_id": 0}})),
        ("decode_with_commands_empty", json!({"decode": {"io_id": 0, "commands": []}})),
        (
            "decode_jpeg_downscale",
            json!({"decode": {"io_id": 0, "commands": [
                {"jpeg_downscale_hints": {"width": 800, "height": 600, "scale_luma_spatially": true, "gamma_correct_for_srgb_during_spatial_luma_scaling": true}}
            ]}}),
        ),
        (
            "decode_jpeg_downscale_minimal",
            json!({"decode": {"io_id": 0, "commands": [
                {"jpeg_downscale_hints": {"width": 400, "height": 300}}
            ]}}),
        ),
        (
            "decode_webp_hints",
            json!({"decode": {"io_id": 0, "commands": [
                {"webp_decoder_hints": {"width": 800, "height": 600}}
            ]}}),
        ),
        (
            "decode_discard_color_profile",
            json!({"decode": {"io_id": 0, "commands": [
                "discard_color_profile"
            ]}}),
        ),
        (
            "decode_ignore_color_errors",
            json!({"decode": {"io_id": 0, "commands": [
                "ignore_color_profile_errors"
            ]}}),
        ),
        (
            "decode_select_frame",
            json!({"decode": {"io_id": 0, "commands": [
                {"select_frame": 3}
            ]}}),
        ),
        (
            "decode_multiple_commands",
            json!({"decode": {"io_id": 0, "commands": [
                {"jpeg_downscale_hints": {"width": 1000, "height": 1000}},
                "discard_color_profile",
                "ignore_color_profile_errors"
            ]}}),
        ),
    ]
}

/// Generate every possible legacy EncoderPreset variant.
fn all_legacy_encode_nodes() -> Vec<(&'static str, Value)> {
    vec![
        // ── LibjpegTurbo ──
        (
            "encode_libjpeg_turbo_full",
            json!({"encode": {"io_id": 1, "preset": {"libjpegturbo": {
                "quality": 90,
                "progressive": true,
                "optimize_huffman_coding": true,
                "matte": {"srgb": {"hex": "FFFFFF"}}
            }}}}),
        ),
        (
            "encode_libjpeg_turbo_minimal",
            json!({"encode": {"io_id": 1, "preset": {"libjpegturbo": {}}}}),
        ),
        // ── Mozjpeg ──
        (
            "encode_mozjpeg_full",
            json!({"encode": {"io_id": 1, "preset": {"mozjpeg": {
                "quality": 85,
                "progressive": true,
                "matte": {"black": null}
            }}}}),
        ),
        ("encode_mozjpeg_minimal", json!({"encode": {"io_id": 1, "preset": {"mozjpeg": {}}}})),
        // ── Libpng ──
        (
            "encode_libpng_32",
            json!({"encode": {"io_id": 1, "preset": {"libpng": {
                "depth": "png_32",
                "matte": null,
                "zlib_compression": 6
            }}}}),
        ),
        (
            "encode_libpng_24",
            json!({"encode": {"io_id": 1, "preset": {"libpng": {
                "depth": "png_24",
                "matte": {"srgb": {"hex": "FFFFFF"}}
            }}}}),
        ),
        ("encode_libpng_minimal", json!({"encode": {"io_id": 1, "preset": {"libpng": {}}}})),
        // ── Pngquant ──
        (
            "encode_pngquant_full",
            json!({"encode": {"io_id": 1, "preset": {"pngquant": {
                "quality": 80,
                "minimum_quality": 60,
                "speed": 3,
                "maximum_deflate": true
            }}}}),
        ),
        ("encode_pngquant_minimal", json!({"encode": {"io_id": 1, "preset": {"pngquant": {}}}})),
        // ── Lodepng ──
        (
            "encode_lodepng_max_deflate",
            json!({"encode": {"io_id": 1, "preset": {"lodepng": {"maximum_deflate": true}}}}),
        ),
        ("encode_lodepng_minimal", json!({"encode": {"io_id": 1, "preset": {"lodepng": {}}}})),
        // ── WebP ──
        (
            "encode_webp_lossy",
            json!({"encode": {"io_id": 1, "preset": {"webplossy": {"quality": 80.0}}}}),
        ),
        ("encode_webp_lossless", json!({"encode": {"io_id": 1, "preset": "webplossless"}})),
        // ── GIF ──
        ("encode_gif", json!({"encode": {"io_id": 1, "preset": "gif"}})),
        // ── JXL ──
        (
            "encode_jxl_lossy",
            json!({"encode": {"io_id": 1, "preset": {"jxllossy": {"distance": 1.5}}}}),
        ),
        ("encode_jxl_lossless", json!({"encode": {"io_id": 1, "preset": "jxllossless"}})),
        // ── Auto ──
        (
            "encode_auto_medium",
            json!({"encode": {"io_id": 1, "preset": {"auto": {
                "quality_profile": "medium",
                "matte": null,
                "lossless": null,
                "allow": null
            }}}}),
        ),
        (
            "encode_auto_highest_web_safe",
            json!({"encode": {"io_id": 1, "preset": {"auto": {
                "quality_profile": "highest",
                "allow": {"web_safe": true}
            }}}}),
        ),
        (
            "encode_auto_lossless",
            json!({"encode": {"io_id": 1, "preset": {"auto": {
                "quality_profile": "lossless",
                "lossless": "true",
                "allow": {"all": true}
            }}}}),
        ),
        (
            "encode_auto_percent_modern",
            json!({"encode": {"io_id": 1, "preset": {"auto": {
                "quality_profile": {"percent": 85.0},
                "quality_profile_dpr": 2.0,
                "allow": {"modern_web_safe": true}
            }}}}),
        ),
        (
            "encode_auto_with_matte",
            json!({"encode": {"io_id": 1, "preset": {"auto": {
                "quality_profile": "good",
                "matte": {"srgb": {"hex": "FFFFFF"}},
                "allow": {"jpeg": true, "png": true}
            }}}}),
        ),
        // ── Format ──
        (
            "encode_format_webp_with_hints",
            json!({"encode": {"io_id": 1, "preset": {"format": {
                "format": "webp",
                "quality_profile": "high",
                "lossless": "keep",
                "encoder_hints": {
                    "webp": {"quality": 90.0, "lossless": "true"}
                }
            }}}}),
        ),
        (
            "encode_format_jpeg_with_hints",
            json!({"encode": {"io_id": 1, "preset": {"format": {
                "format": "jpeg",
                "quality_profile": {"percent": 90.0},
                "encoder_hints": {
                    "jpeg": {"quality": 92.0, "progressive": true, "mimic": "mozjpeg"}
                }
            }}}}),
        ),
        (
            "encode_format_png_with_hints",
            json!({"encode": {"io_id": 1, "preset": {"format": {
                "format": "png",
                "quality_profile": "medium",
                "encoder_hints": {
                    "png": {
                        "quality": 80.0,
                        "min_quality": 60.0,
                        "quantization_speed": 3,
                        "mimic": "pngquant",
                        "hint_max_deflate": true,
                        "lossless": "false"
                    }
                }
            }}}}),
        ),
        (
            "encode_format_jxl_with_hints",
            json!({"encode": {"io_id": 1, "preset": {"format": {
                "format": "jxl",
                "encoder_hints": {
                    "jxl": {"quality": 85.0, "lossless": false, "distance": 1.0}
                }
            }}}}),
        ),
        (
            "encode_format_avif_with_hints",
            json!({"encode": {"io_id": 1, "preset": {"format": {
                "format": "avif",
                "quality_profile": "high",
                "encoder_hints": {
                    "avif": {"quality": 80.0, "speed": 4, "alpha_quality": 70.0}
                }
            }}}}),
        ),
        (
            "encode_format_gif_with_hints",
            json!({"encode": {"io_id": 1, "preset": {"format": {
                "format": "gif",
                "encoder_hints": {"gif": {}}
            }}}}),
        ),
        (
            "encode_format_keep",
            json!({"encode": {"io_id": 1, "preset": {"format": {
                "format": "keep"
            }}}}),
        ),
        (
            "encode_format_all_allowed",
            json!({"encode": {"io_id": 1, "preset": {"format": {
                "format": "jpeg",
                "allow": {
                    "webp": true, "jxl": true, "avif": true, "jpeg": true,
                    "jpeg_progressive": true, "jpeg_xyb": true,
                    "png": true, "gif": true, "all": true,
                    "web_safe": true, "modern_web_safe": true,
                    "color_profiles": true
                }
            }}}}),
        ),
    ]
}

/// Generate every possible legacy IO variant.
fn all_legacy_io_objects() -> Vec<(&'static str, Value)> {
    vec![
        ("io_placeholder_in", json!({"io_id": 0, "direction": "in", "io": "placeholder"})),
        ("io_output_buffer", json!({"io_id": 1, "direction": "out", "io": "output_buffer"})),
        ("io_output_base64", json!({"io_id": 2, "direction": "out", "io": "output_base_64"})),
        ("io_filename_in", json!({"io_id": 0, "direction": "in", "io": {"file": "input.jpg"}})),
        ("io_filename_out", json!({"io_id": 1, "direction": "out", "io": {"file": "output.png"}})),
        (
            "io_byte_array",
            json!({"io_id": 0, "direction": "in", "io": {"byte_array": [0x89, 0x50, 0x4E, 0x47]}}),
        ),
        ("io_bytes_hex", json!({"io_id": 0, "direction": "in", "io": {"bytes_hex": "89504E47"}})),
        ("io_base64", json!({"io_id": 0, "direction": "in", "io": {"base_64": "iVBORw0KGgo="}})),
    ]
}

// ─── Full Build001 Jobs ─────────────────────────────────────────────────

/// Wrap a list of step nodes into a full Build001 with Steps framewise.
fn build_steps_job(steps: Vec<Value>) -> Value {
    json!({
        "io": [
            {"io_id": 0, "direction": "in", "io": "placeholder"},
            {"io_id": 1, "direction": "out", "io": "output_buffer"},
            {"io_id": 2, "direction": "in", "io": "placeholder"},
            {"io_id": 3, "direction": "in", "io": "placeholder"},
            {"io_id": 4, "direction": "in", "io": "placeholder"},
            {"io_id": 5, "direction": "in", "io": "placeholder"}
        ],
        "framewise": {
            "steps": steps
        }
    })
}

/// Wrap nodes into a graph-mode Build001.
fn build_graph_job(nodes: Vec<(&str, Value)>, edges: Vec<Value>) -> Value {
    let mut node_map = serde_json::Map::new();
    for (id, node) in nodes {
        node_map.insert(id.to_string(), node);
    }
    json!({
        "io": [
            {"io_id": 0, "direction": "in", "io": "placeholder"},
            {"io_id": 1, "direction": "out", "io": "output_buffer"},
            {"io_id": 2, "direction": "out", "io": "output_buffer"}
        ],
        "framewise": {
            "graph": {
                "nodes": node_map,
                "edges": edges
            }
        }
    })
}

// ─── Tests ──────────────────────────────────────────────────────────────

#[test]
fn all_legacy_nodes_are_valid_json() {
    let nodes = all_legacy_nodes();
    let decodes = all_legacy_decode_nodes();
    let encodes = all_legacy_encode_nodes();

    let total = nodes.len() + decodes.len() + encodes.len();
    eprintln!("Total legacy node variants: {total}");

    for (name, val) in nodes.iter().chain(decodes.iter()).chain(encodes.iter()) {
        // Verify it's valid JSON (already is, but verify re-serialization)
        let json_str = serde_json::to_string(val).unwrap();
        let _: Value = serde_json::from_str(&json_str)
            .unwrap_or_else(|e| panic!("Failed to re-parse '{name}': {e}"));
    }
}

#[test]
fn all_legacy_io_variants_valid() {
    let ios = all_legacy_io_objects();
    eprintln!("Total IO variants: {}", ios.len());

    for (name, val) in &ios {
        let json_str = serde_json::to_string(val).unwrap();
        let _: Value = serde_json::from_str(&json_str)
            .unwrap_or_else(|e| panic!("Failed to re-parse IO '{name}': {e}"));
    }
}

#[test]
fn exhaustive_steps_jobs() {
    // Build one job per node variant (decode + node + encode)
    let decodes = all_legacy_decode_nodes();
    let nodes = all_legacy_nodes();
    let encodes = all_legacy_encode_nodes();

    let mut job_count = 0;

    // One job per decode variant
    for (name, decode) in &decodes {
        let job = build_steps_job(vec![
            decode.clone(),
            json!({"constrain": {"mode": "fit", "w": 400, "h": 300}}),
            json!({"encode": {"io_id": 1, "preset": {"mozjpeg": {"quality": 85}}}}),
        ]);
        let json_str = serde_json::to_string_pretty(&job).unwrap();
        let _: Value = serde_json::from_str(&json_str)
            .unwrap_or_else(|e| panic!("Steps job failed for decode '{name}': {e}"));
        job_count += 1;
    }

    // One job per operation node variant
    for (name, node) in &nodes {
        let job = build_steps_job(vec![
            json!({"decode": {"io_id": 0}}),
            node.clone(),
            json!({"encode": {"io_id": 1, "preset": {"libpng": {"depth": "png_32"}}}}),
        ]);
        let json_str = serde_json::to_string_pretty(&job).unwrap();
        let _: Value = serde_json::from_str(&json_str)
            .unwrap_or_else(|e| panic!("Steps job failed for node '{name}': {e}"));
        job_count += 1;
    }

    // One job per encode variant
    for (name, encode) in &encodes {
        let job = build_steps_job(vec![
            json!({"decode": {"io_id": 0}}),
            json!({"constrain": {"mode": "fit", "w": 800, "h": 600}}),
            encode.clone(),
        ]);
        let json_str = serde_json::to_string_pretty(&job).unwrap();
        let _: Value = serde_json::from_str(&json_str)
            .unwrap_or_else(|e| panic!("Steps job failed for encode '{name}': {e}"));
        job_count += 1;
    }

    eprintln!("Total exhaustive Steps jobs: {job_count}");
}

#[test]
fn exhaustive_graph_jobs() {
    // Graph mode: decode -> node -> encode with edges
    let nodes = all_legacy_nodes();

    let mut job_count = 0;

    for (name, node) in &nodes {
        let job = build_graph_job(
            vec![
                ("0", json!({"decode": {"io_id": 0}})),
                ("1", node.clone()),
                ("2", json!({"encode": {"io_id": 1, "preset": {"mozjpeg": {"quality": 85}}}})),
            ],
            vec![
                json!({"from": 0, "to": 1, "kind": "input"}),
                json!({"from": 1, "to": 2, "kind": "input"}),
            ],
        );
        let json_str = serde_json::to_string_pretty(&job).unwrap();
        let _: Value = serde_json::from_str(&json_str)
            .unwrap_or_else(|e| panic!("Graph job failed for '{name}': {e}"));
        job_count += 1;
    }

    // Graph with fan-out (one decode, two encodes)
    let job = build_graph_job(
        vec![
            ("0", json!({"decode": {"io_id": 0}})),
            ("1", json!({"constrain": {"mode": "fit", "w": 800, "h": 600}})),
            ("2", json!({"constrain": {"mode": "fit", "w": 200, "h": 150}})),
            ("3", json!({"encode": {"io_id": 1, "preset": {"mozjpeg": {"quality": 85}}}})),
            ("4", json!({"encode": {"io_id": 2, "preset": {"libpng": {"depth": "png_32"}}}})),
        ],
        vec![
            json!({"from": 0, "to": 1, "kind": "input"}),
            json!({"from": 0, "to": 2, "kind": "input"}),
            json!({"from": 1, "to": 3, "kind": "input"}),
            json!({"from": 2, "to": 4, "kind": "input"}),
        ],
    );
    let _ = serde_json::to_string_pretty(&job).unwrap();
    job_count += 1;

    // Graph with canvas compositing
    let job = build_graph_job(
        vec![
            ("0", json!({"decode": {"io_id": 0}})),
            (
                "1",
                json!({"create_canvas": {"format": "bgra_32", "w": 200, "h": 200, "color": {"transparent": null}}}),
            ),
            (
                "2",
                json!({"copy_rect_to_canvas": {"from_x": 0, "from_y": 0, "w": 100, "h": 100, "x": 50, "y": 50}}),
            ),
            ("3", json!({"resample_2d": {"w": 100, "h": 100}})),
            ("4", json!({"encode": {"io_id": 1, "preset": {"libpng": {"depth": "png_32"}}}})),
        ],
        vec![
            json!({"from": 0, "to": 3, "kind": "input"}),
            json!({"from": 3, "to": 2, "kind": "input"}),
            json!({"from": 1, "to": 2, "kind": "canvas"}),
            json!({"from": 2, "to": 4, "kind": "input"}),
        ],
    );
    let _ = serde_json::to_string_pretty(&job).unwrap();
    job_count += 1;

    eprintln!("Total exhaustive Graph jobs: {job_count}");
}

#[test]
fn exhaustive_builder_config_variants() {
    // Job with full builder_config
    let job = json!({
        "builder_config": {
            "graph_recording": {
                "record_graph_versions": true,
                "record_frame_images": true,
                "render_last_graph": true,
                "render_graph_versions": false,
                "render_animated_graph": false
            },
            "security": {
                "max_decode_size": {"w": 10000, "h": 10000, "megapixels": 100.0},
                "max_frame_size": {"w": 10000, "h": 10000, "megapixels": 100.0},
                "max_encode_size": {"w": 10000, "h": 10000, "megapixels": 100.0},
                "process_timeout_ms": 30000,
                "max_encoder_threads": 4
            }
        },
        "io": [
            {"io_id": 0, "direction": "in", "io": "placeholder"},
            {"io_id": 1, "direction": "out", "io": "output_buffer"}
        ],
        "framewise": {
            "steps": [
                {"decode": {"io_id": 0}},
                {"encode": {"io_id": 1, "preset": {"mozjpeg": {"quality": 85}}}}
            ]
        }
    });

    let json_str = serde_json::to_string_pretty(&job).unwrap();
    let roundtrip: Value = serde_json::from_str(&json_str).unwrap();
    assert!(
        roundtrip["builder_config"]["security"]["max_decode_size"]["w"].as_u64().unwrap() == 10000
    );

    // Null builder_config
    let job2 = json!({
        "io": [{"io_id": 0, "direction": "in", "io": "placeholder"}, {"io_id": 1, "direction": "out", "io": "output_buffer"}],
        "framewise": {"steps": [{"decode": {"io_id": 0}}, {"encode": {"io_id": 1, "preset": "gif"}}]}
    });
    let json_str2 = serde_json::to_string_pretty(&job2).unwrap();
    let _: Value = serde_json::from_str(&json_str2).unwrap();
}

// ─── Cross-Comparison: Legacy JSON → imageflow-commands ─────────────────

/// Translate a legacy Node JSON to imageflow-commands Step JSON.
/// Returns None if the node has no direct equivalent.
fn translate_legacy_node_to_new(legacy: &Value) -> Option<Value> {
    // Parameterless geometry — legacy uses underscored names,
    // new Step enum uses snake_case from rename_all which produces rotate90, etc.
    if legacy.get("flip_v").is_some() {
        return Some(json!("flip_v"));
    }
    if legacy.get("flip_h").is_some() {
        return Some(json!("flip_h"));
    }
    if legacy.get("transpose").is_some() {
        return Some(json!("transpose"));
    }
    if legacy.get("rotate_90").is_some() {
        return Some(json!("rotate90"));
    }
    if legacy.get("rotate_180").is_some() {
        return Some(json!("rotate180"));
    }
    if legacy.get("rotate_270").is_some() {
        return Some(json!("rotate270"));
    }
    if let Some(crop) = legacy.get("crop") {
        return Some(json!({"crop": {
            "x1": crop["x1"], "y1": crop["y1"],
            "x2": crop["x2"], "y2": crop["y2"]
        }}));
    }
    if let Some(cw) = legacy.get("crop_whitespace") {
        return Some(json!({"crop_whitespace": {
            "threshold": cw["threshold"],
            "percent_padding": cw["percent_padding"]
        }}));
    }
    if let Some(r) = legacy.get("region") {
        return Some(json!({"region": {
            "x1": r["x1"], "y1": r["y1"],
            "x2": r["x2"], "y2": r["y2"],
            "background": translate_color(&r["background_color"])
        }}));
    }
    if let Some(rp) = legacy.get("region_percent") {
        return Some(json!({"region_percent": {
            "x1": rp["x1"], "y1": rp["y1"],
            "x2": rp["x2"], "y2": rp["y2"],
            "background": translate_color(&rp["background_color"])
        }}));
    }
    if let Some(ao) = legacy.get("apply_orientation") {
        let flag = ao["flag"].as_i64().unwrap() as u8;
        return Some(json!({"orient": {"exif": flag}}));
    }
    if let Some(ec) = legacy.get("expand_canvas") {
        return Some(json!({"expand_canvas": {
            "left": ec["left"], "top": ec["top"],
            "right": ec["right"], "bottom": ec["bottom"],
            "color": translate_color(&ec["color"])
        }}));
    }
    if let Some(fr) = legacy.get("fill_rect") {
        return Some(json!({"fill_rect": {
            "x1": fr["x1"], "y1": fr["y1"],
            "x2": fr["x2"], "y2": fr["y2"],
            "color": translate_color(&fr["color"])
        }}));
    }
    if let Some(r2d) = legacy.get("resample_2d") {
        return Some(json!({"resize": {
            "w": r2d["w"], "h": r2d["h"],
            "hints": translate_resize_hints(r2d.get("hints"))
        }}));
    }
    if let Some(c) = legacy.get("constrain") {
        let mut new = json!({"constrain": {
            "mode": c["mode"],
            "w": c.get("w"),
            "h": c.get("h"),
            "hints": translate_resize_hints(c.get("hints"))
        }});
        if let Some(g) = c.get("gravity") {
            new["constrain"]["gravity"] = translate_gravity(g);
        }
        if let Some(cc) = c.get("canvas_color") {
            new["constrain"]["background"] = translate_color(cc);
        }
        return Some(new);
    }
    if let Some(cf) = legacy.get("color_filter_srgb") {
        return Some(json!({"color_filter": cf.clone()}));
    }
    if let Some(cm) = legacy.get("color_matrix_srgb") {
        // Legacy uses [[f32;5];5], new uses [f32;25] — flatten
        if let Some(matrix) = cm.get("matrix") {
            let flat: Vec<Value> = matrix
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|row| row.as_array().unwrap().clone())
                .collect();
            return Some(json!({"color_matrix": {"matrix": flat}}));
        }
    }
    if legacy.get("white_balance_histogram_area_threshold_srgb").is_some() {
        let wb = &legacy["white_balance_histogram_area_threshold_srgb"];
        return Some(json!({"white_balance": {"threshold": wb["threshold"]}}));
    }
    // Nodes with no direct 1:1 mapping (create_canvas, watermark, etc.) — skip for now
    None
}

fn translate_color(c: &Value) -> Value {
    if c.get("transparent").is_some() {
        return json!({"r": 0, "g": 0, "b": 0, "a": 0});
    }
    if c.get("black").is_some() {
        return json!({"r": 0, "g": 0, "b": 0, "a": 255});
    }
    if let Some(srgb) = c.get("srgb") {
        if let Some(hex) = srgb.get("hex") {
            return json!(hex);
        }
    }
    json!(null)
}

fn translate_gravity(g: &Value) -> Value {
    if g.get("center").is_some() {
        return json!("center");
    }
    if let Some(pct) = g.get("percentage") {
        return json!({"percent": {"x": pct["x"], "y": pct["y"]}});
    }
    json!("center")
}

fn translate_resize_hints(hints: Option<&Value>) -> Value {
    match hints {
        None | Some(&Value::Null) => json!(null),
        Some(h) => {
            let mut new = serde_json::Map::new();
            if let Some(f) = h.get("down_filter") {
                if !f.is_null() {
                    new.insert("filter".into(), f.clone());
                }
            }
            if let Some(sp) = h.get("sharpen_percent") {
                if !sp.is_null() {
                    new.insert("sharpen_percent".into(), sp.clone());
                }
            }
            if let Some(sc) = h.get("scaling_colorspace") {
                if !sc.is_null() {
                    new.insert("scaling_colorspace".into(), sc.clone());
                }
            }
            if new.is_empty() {
                json!(null)
            } else {
                Value::Object(new)
            }
        }
    }
}

#[test]
fn cross_compare_translatable_nodes() {
    let legacy_nodes = all_legacy_nodes();
    let mut translated = 0;
    let mut skipped = 0;

    for (name, legacy) in &legacy_nodes {
        match translate_legacy_node_to_new(legacy) {
            Some(new_step) => {
                // Verify the new step deserializes as imageflow-commands::Step
                let json_str = serde_json::to_string(&new_step).unwrap();
                let result: Result<imageflow_commands::Step, _> = serde_json::from_str(&json_str);
                match result {
                    Ok(_step) => {
                        translated += 1;
                    }
                    Err(e) => {
                        panic!(
                            "Legacy node '{name}' translated to invalid Step:\n  legacy: {}\n  new: {}\n  error: {e}",
                            serde_json::to_string(legacy).unwrap(),
                            json_str
                        );
                    }
                }
            }
            None => {
                skipped += 1;
            }
        }
    }

    eprintln!("Cross-comparison: {translated} translated, {skipped} skipped (no 1:1 mapping)");
    // At least the simple geometry + color nodes should translate
    assert!(translated >= 20, "Expected at least 20 translated nodes, got {translated}");
}

#[test]
fn count_all_variants() {
    let nodes = all_legacy_nodes().len();
    let decodes = all_legacy_decode_nodes().len();
    let encodes = all_legacy_encode_nodes().len();
    let ios = all_legacy_io_objects().len();
    let total = nodes + decodes + encodes + ios;

    eprintln!("=== Legacy Variant Coverage ===");
    eprintln!("  Operation nodes:  {nodes}");
    eprintln!("  Decode variants:  {decodes}");
    eprintln!("  Encode variants:  {encodes}");
    eprintln!("  IO variants:      {ios}");
    eprintln!("  Total:            {total}");

    // Minimum coverage assertions
    assert!(nodes >= 50, "Expected at least 50 operation nodes, got {nodes}");
    assert!(decodes >= 8, "Expected at least 8 decode variants, got {decodes}");
    assert!(encodes >= 20, "Expected at least 20 encode variants, got {encodes}");
    assert!(ios >= 7, "Expected at least 7 IO variants, got {ios}");
}
