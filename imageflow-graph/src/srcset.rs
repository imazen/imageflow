//! Srcset/short querystring expansion.
//!
//! `&srcset=webp-70,100w,fit-crop` is syntactic sugar for individual query
//! parameters. This module expands the compact form into key-value pairs
//! before any parser sees the querystring.
//!
//! The expansion is purely mechanical — no semantic interpretation. The
//! resulting pairs are routed by [`super::key_router`] to their respective
//! subsystems.

/// A warning from srcset expansion.
#[derive(Debug, Clone, PartialEq)]
pub struct SrcsetWarning {
    pub message: String,
}

/// Expand a srcset/short value into individual key-value pairs.
///
/// Input: the value of `&srcset=` (e.g. `"webp-70,100w,fit-crop"`).
/// Output: equivalent key-value pairs (e.g. `[("format","webp"), ("webp.quality","70"), ...]`).
pub fn expand_srcset(value: &str) -> (Vec<(String, String)>, Vec<SrcsetWarning>) {
    let mut pairs = Vec::new();
    let mut warnings = Vec::new();
    let mut modes_specified = 0u32;
    let mut formats_specified = 0u32;

    for command_untrimmed in value.to_ascii_lowercase().split(',') {
        let command = command_untrimmed.trim();
        if command.is_empty() {
            continue;
        }

        let mut args = command.split('-');
        let Some(name) = args.next() else { continue };

        match name {
            // ── Format commands ──
            "webp" => {
                formats_specified += 1;
                pairs.push(("format".into(), "webp".into()));
                expand_format_tuning("webp", &mut args, &mut pairs, &mut warnings);
            }
            "jpeg" | "jpg" => {
                formats_specified += 1;
                pairs.push(("format".into(), "jpeg".into()));
                expand_format_tuning("jpeg", &mut args, &mut pairs, &mut warnings);
            }
            "png" => {
                formats_specified += 1;
                pairs.push(("format".into(), "png".into()));
                expand_format_tuning("png", &mut args, &mut pairs, &mut warnings);
            }
            "jxl" => {
                formats_specified += 1;
                pairs.push(("format".into(), "jxl".into()));
                expand_format_tuning("jxl", &mut args, &mut pairs, &mut warnings);
            }
            "avif" => {
                formats_specified += 1;
                pairs.push(("format".into(), "avif".into()));
                expand_format_tuning("avif", &mut args, &mut pairs, &mut warnings);
            }
            "gif" => {
                formats_specified += 1;
                pairs.push(("format".into(), "gif".into()));
            }
            "auto" => {
                formats_specified += 1;
                pairs.push(("format".into(), "auto".into()));
                expand_format_tuning("auto", &mut args, &mut pairs, &mut warnings);
            }

            // ── Lossless/lossy as top-level command ──
            "lossless" => {
                formats_specified += 1;
                pairs.push(("format".into(), "auto".into()));
                pairs.push(("lossless".into(), "true".into()));
            }
            "lossy" => {
                formats_specified += 1;
                pairs.push(("format".into(), "auto".into()));
                pairs.push(("lossless".into(), "false".into()));
            }

            // ── Quality profile ──
            "qp" => {
                expand_qp(&mut args, &mut pairs, &mut warnings);
            }

            // ── Crop ──
            "crop" => {
                let coords: Vec<&str> = args.collect();
                if coords.len() == 4 && coords.iter().all(|c| c.parse::<f64>().is_ok()) {
                    let joined = coords.join(",");
                    pairs.push(("crop".into(), joined));
                    pairs.push(("cropxunits".into(), "100".into()));
                    pairs.push(("cropyunits".into(), "100".into()));
                } else {
                    warnings.push(SrcsetWarning {
                        message: format!(
                            "crop requires 4 numeric args: crop-x1-y1-x2-y2, got: {command}"
                        ),
                    });
                }
                continue; // args consumed
            }

            // ── Fit mode ──
            "fit" => {
                if let Some(mode) = args.next() {
                    let mapped = match mode {
                        "pad" => Some("pad"),
                        "crop" | "cover" => Some("crop"),
                        "max" | "scale" | "contain" => Some("max"),
                        "distort" | "fill" => Some("stretch"),
                        _ => None,
                    };
                    if let Some(m) = mapped {
                        modes_specified += 1;
                        pairs.push(("mode".into(), m.into()));
                    } else {
                        warnings.push(SrcsetWarning {
                            message: format!("unrecognized fit mode: {mode}"),
                        });
                    }
                } else {
                    warnings.push(SrcsetWarning {
                        message: "fit requires a mode: fit-[pad|crop|max|distort]".into(),
                    });
                }
            }

            // ── Upscale ──
            "upscale" => {
                pairs.push(("scale".into(), "both".into()));
            }

            // ── Sharpen ──
            "sharp" | "sharpen" => {
                if let Some(val) = args.next() {
                    if val.parse::<f32>().is_ok() {
                        pairs.push(("f.sharpen".into(), val.into()));
                    } else {
                        warnings.push(SrcsetWarning {
                            message: format!("sharpen value must be numeric, got: {val}"),
                        });
                    }
                } else {
                    warnings.push(SrcsetWarning {
                        message: "sharpen requires a value: sharp-[0-100]".into(),
                    });
                }
            }

            // ── Size/zoom suffixes (100w, 200h, 2.5x) ──
            other => {
                if let Some(parsed) = parse_size_command(other) {
                    pairs.push(parsed);
                } else {
                    warnings.push(SrcsetWarning {
                        message: format!("unrecognized srcset command: {other}"),
                    });
                }
            }
        }
    }

    if modes_specified > 1 {
        warnings.push(SrcsetWarning { message: "multiple fit modes specified in srcset".into() });
    }
    if formats_specified > 1 {
        warnings.push(SrcsetWarning { message: "multiple formats specified in srcset".into() });
    }

    (pairs, warnings)
}

/// Parse a size command: "100w" → ("w", "100"), "2.5x" → ("zoom", "2.5").
fn parse_size_command(s: &str) -> Option<(String, String)> {
    if s.len() < 2 {
        return None;
    }
    let suffix = s.as_bytes()[s.len() - 1];
    let number = &s[..s.len() - 1];
    match suffix {
        b'w' => number.parse::<f32>().ok().map(|v| ("w".into(), format!("{}", v.round() as i32))),
        b'h' => number.parse::<f32>().ok().map(|v| ("h".into(), format!("{}", v.round() as i32))),
        b'x' => number.parse::<f32>().ok().map(|v| ("zoom".into(), format!("{v}"))),
        _ => None,
    }
}

/// Expand format-specific tuning args.
///
/// After the format name, remaining hyphen-delimited args become per-codec
/// quality parameters. E.g., `webp-70` → `("webp.quality", "70")`.
fn expand_format_tuning(
    format: &str,
    args: &mut std::str::Split<'_, char>,
    pairs: &mut Vec<(String, String)>,
    warnings: &mut Vec<SrcsetWarning>,
) {
    for arg in args {
        if arg.is_empty() {
            continue;
        }

        // Lossless/lossy/keep
        if arg == "lossless" || arg == "l" {
            let key = format_lossless_key(format);
            pairs.push((key, "true".into()));
            continue;
        }
        if arg == "lossy" {
            let key = format_lossless_key(format);
            pairs.push((key, "false".into()));
            continue;
        }
        if arg == "keep" {
            let key = format_lossless_key(format);
            pairs.push((key, "keep".into()));
            continue;
        }

        // JPEG progressive/baseline
        if format == "jpeg" {
            if arg == "progressive" {
                pairs.push(("jpeg.progressive".into(), "true".into()));
                continue;
            }
            if arg == "baseline" {
                pairs.push(("jpeg.progressive".into(), "false".into()));
                continue;
            }
        }

        // Prefixed values: d[n]=distance, e[n]=effort, s[n]=speed, mq[n]=min_quality, q[n]=quality
        if let Some(rest) = arg.strip_prefix("mq") {
            if let Ok(v) = rest.parse::<f32>() {
                if format == "png" {
                    pairs.push(("png.min_quality".into(), format!("{v}")));
                } else {
                    warnings.push(SrcsetWarning {
                        message: format!("mq (min_quality) only valid for png, not {format}"),
                    });
                }
                continue;
            }
        }
        if let Some(rest) = arg.strip_prefix('d') {
            if let Ok(v) = rest.parse::<f32>() {
                if format == "jxl" {
                    pairs.push(("jxl.distance".into(), format!("{v}")));
                } else {
                    warnings.push(SrcsetWarning {
                        message: format!("d (distance) only valid for jxl, not {format}"),
                    });
                }
                continue;
            }
        }
        if let Some(rest) = arg.strip_prefix('e') {
            if let Ok(v) = rest.parse::<f32>() {
                if format == "jxl" {
                    let clamped = v.clamp(0.0, 255.0) as u8;
                    pairs.push(("jxl.effort".into(), format!("{clamped}")));
                } else {
                    warnings.push(SrcsetWarning {
                        message: format!("e (effort) only valid for jxl, not {format}"),
                    });
                }
                continue;
            }
        }
        if let Some(rest) = arg.strip_prefix('s') {
            if let Ok(v) = rest.parse::<f32>() {
                if format == "avif" {
                    let clamped = v.clamp(0.0, 255.0) as u8;
                    pairs.push(("avif.speed".into(), format!("{clamped}")));
                } else {
                    warnings.push(SrcsetWarning {
                        message: format!("s (speed) only valid for avif, not {format}"),
                    });
                }
                continue;
            }
        }
        if let Some(rest) = arg.strip_prefix('q') {
            if let Ok(v) = rest.parse::<f32>() {
                let key = format_quality_key(format);
                pairs.push((key, format!("{v}")));
                continue;
            }
        }

        // Bare number = quality
        if let Ok(v) = arg.parse::<f32>() {
            let key = format_quality_key(format);
            pairs.push((key, format!("{v}")));
            continue;
        }

        warnings
            .push(SrcsetWarning { message: format!("unrecognized {format} tuning arg: {arg}") });
    }
}

/// Expand qp command: `qp-good`, `qp-75`, `qp-dpr-2`.
fn expand_qp(
    args: &mut std::str::Split<'_, char>,
    pairs: &mut Vec<(String, String)>,
    warnings: &mut Vec<SrcsetWarning>,
) {
    if let Some(arg1) = args.next() {
        if arg1 == "dpr" || arg1 == "dppx" {
            if let Some(arg2) = args.next() {
                let number = arg2.strip_suffix('x').unwrap_or(arg2);
                if number.parse::<f32>().is_ok() {
                    pairs.push(("qp.dpr".into(), number.into()));
                } else {
                    warnings.push(SrcsetWarning {
                        message: format!("qp-dpr must be followed by a number, got: {arg2}"),
                    });
                }
            } else {
                warnings
                    .push(SrcsetWarning { message: "qp-dpr must be followed by a number".into() });
            }
        } else {
            // Named profile or numeric
            pairs.push(("qp".into(), arg1.into()));
        }
    } else {
        warnings.push(SrcsetWarning {
            message: "qp must be followed by a profile name or number".into(),
        });
    }
}

/// Return the per-format quality key.
fn format_quality_key(format: &str) -> String {
    match format {
        "jpeg" | "jpg" => "jpeg.quality".into(),
        "png" => "png.quality".into(),
        "webp" => "webp.quality".into(),
        "avif" => "avif.quality".into(),
        "jxl" => "jxl.quality".into(),
        "auto" => "quality".into(),
        _ => "quality".into(),
    }
}

/// Return the per-format lossless key.
fn format_lossless_key(format: &str) -> String {
    match format {
        "png" => "png.lossless".into(),
        "webp" => "webp.lossless".into(),
        "jxl" => "jxl.lossless".into(),
        _ => "lossless".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_format_quality() {
        let (pairs, warnings) = expand_srcset("webp-70");
        assert!(warnings.is_empty());
        assert!(pairs.contains(&("format".into(), "webp".into())));
        assert!(pairs.contains(&("webp.quality".into(), "70".into())));
    }

    #[test]
    fn size_and_mode() {
        let (pairs, warnings) = expand_srcset("100w,200h,fit-crop");
        assert!(warnings.is_empty());
        assert!(pairs.contains(&("w".into(), "100".into())));
        assert!(pairs.contains(&("h".into(), "200".into())));
        assert!(pairs.contains(&("mode".into(), "crop".into())));
    }

    #[test]
    fn zoom_factor() {
        let (pairs, warnings) = expand_srcset("2.5x");
        assert!(warnings.is_empty());
        assert!(pairs.contains(&("zoom".into(), "2.5".into())));
    }

    #[test]
    fn jxl_full_tuning() {
        let (pairs, warnings) = expand_srcset("jxl-q80-d1.5-e7");
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert!(pairs.contains(&("format".into(), "jxl".into())));
        assert!(pairs.contains(&("jxl.quality".into(), "80".into())));
        assert!(pairs.contains(&("jxl.distance".into(), "1.5".into())));
        assert!(pairs.contains(&("jxl.effort".into(), "7".into())));
    }

    #[test]
    fn avif_speed() {
        let (pairs, warnings) = expand_srcset("avif-q70-s6");
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert!(pairs.contains(&("avif.quality".into(), "70".into())));
        assert!(pairs.contains(&("avif.speed".into(), "6".into())));
    }

    #[test]
    fn crop_percent() {
        let (pairs, warnings) = expand_srcset("crop-20-30-80-90");
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert!(pairs.contains(&("crop".into(), "20,30,80,90".into())));
        assert!(pairs.contains(&("cropxunits".into(), "100".into())));
        assert!(pairs.contains(&("cropyunits".into(), "100".into())));
    }

    #[test]
    fn qp_profile() {
        let (pairs, warnings) = expand_srcset("qp-good");
        assert!(warnings.is_empty());
        assert!(pairs.contains(&("qp".into(), "good".into())));
    }

    #[test]
    fn qp_dpr() {
        let (pairs, warnings) = expand_srcset("qp-dpr-2");
        assert!(warnings.is_empty());
        assert!(pairs.contains(&("qp.dpr".into(), "2".into())));
    }

    #[test]
    fn sharpen() {
        let (pairs, warnings) = expand_srcset("sharp-15");
        assert!(warnings.is_empty());
        assert!(pairs.contains(&("f.sharpen".into(), "15".into())));
    }

    #[test]
    fn lossless_toplevel() {
        let (pairs, _) = expand_srcset("lossless");
        assert!(pairs.contains(&("format".into(), "auto".into())));
        assert!(pairs.contains(&("lossless".into(), "true".into())));
    }

    #[test]
    fn png_lossless_and_min_quality() {
        let (pairs, warnings) = expand_srcset("png-lossless-mq50");
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert!(pairs.contains(&("format".into(), "png".into())));
        assert!(pairs.contains(&("png.lossless".into(), "true".into())));
        assert!(pairs.contains(&("png.min_quality".into(), "50".into())));
    }

    #[test]
    fn jpeg_progressive() {
        let (pairs, warnings) = expand_srcset("jpeg-80-progressive");
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert!(pairs.contains(&("jpeg.quality".into(), "80".into())));
        assert!(pairs.contains(&("jpeg.progressive".into(), "true".into())));
    }

    #[test]
    fn combined_example() {
        let (pairs, warnings) = expand_srcset("webp-70,sharp-15,100w");
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert!(pairs.contains(&("format".into(), "webp".into())));
        assert!(pairs.contains(&("webp.quality".into(), "70".into())));
        assert!(pairs.contains(&("f.sharpen".into(), "15".into())));
        assert!(pairs.contains(&("w".into(), "100".into())));
    }

    #[test]
    fn multiple_formats_warns() {
        let (_, warnings) = expand_srcset("webp-70,jpeg-80");
        assert!(warnings.iter().any(|w| w.message.contains("multiple formats")));
    }

    #[test]
    fn unrecognized_command_warns() {
        let (_, warnings) = expand_srcset("banana");
        assert!(warnings.iter().any(|w| w.message.contains("unrecognized")));
    }

    #[test]
    fn upscale() {
        let (pairs, warnings) = expand_srcset("upscale");
        assert!(warnings.is_empty());
        assert!(pairs.contains(&("scale".into(), "both".into())));
    }
}
