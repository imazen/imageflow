// &srcset syntax is a comma-delimited short-form ideal for using in srcset attributes
// https://github.com/imazen/imageflow/issues/629
// Examples:
// * &srcset=webp-70,sharp-15,100w
// * &srcset=jpeg-80,2x,100w,sharpen-20
// * &srcset=png-90,2.5x,100w,100h,fit-crop
// * &srcset=png-lossless
// * &srcset=gif,crop-20-30-90-100,2.5x,100w,100h
// * &srcset=webp-lossless,2.5x,100w,100h,upscale
// if the format -value is not specified, the defaults will be used. webp will be lossy, and png will be lossy if png.quality is not specified or png.lossless=false

// Some unexpected changes from the original spec: the default mode is max, not pad, and the default cropxunits/cropyunits is percent, not pixels.

// This function mutates a Instructions::new() to add the values from the srcset string, overriding existing values, and returns Vec<ParseWarning>

// The srcset string is a comma-delimited list of commands. Each command is a hyphen-delimited list of values. The first value is the command name, and the rest are arguments.

use super::parsing::{FitMode, Instructions, OutputFormat, ParseWarning, ScaleMode};
use imageflow_types::BoolKeep;
use imageflow_types::QualityProfile;
use std::str::FromStr;

fn srcset_syntax_message_for(format: OutputFormat) -> &'static str {
    match format {
        OutputFormat::Webp => "srcset=webp-[quality|lossless|keep|s[number]|q[number]]",
        OutputFormat::Jxl => "srcset=jxl-[quality|lossless|keep|d[number]|e[number]|q[number]]",
        OutputFormat::Jpeg => "srcset=jpeg-[quality]",
        OutputFormat::Png => "srcset=png-[quality|lossless]",
        OutputFormat::Avif => "srcset=avif-[quality|s[number]|q[number]]",
        OutputFormat::Gif => "srcset=gif",
        OutputFormat::Auto => "srcset=auto",
        _ => "srcset=[format]-[quality|lossless|keep|s[number]|q[number]|d[number]]",
    }
}
fn parse_format_tuning(
    format: OutputFormat,
    lossless: Option<BoolKeep>,
    srcset: &str,
    iter: &mut std::str::Split<'_, &str>,
    i: &mut Instructions,
    warnings: &mut Vec<ParseWarning>,
) {
    // if any is 'l' or 'lossless', set lossless
    // if any is a regular number or q[number], set quality
    // if any is 'keep', set keep lossless
    // if any is 'd[number]', e[], s[], mq[], set distance (jxl), effort, speed, min-quality (png)
    let mut set_lossless = lossless;
    let mut quality = None;
    let mut count = 0;
    for arg in iter {
        count += 1;
        if arg == "lossless" || arg == "l" {
            set_lossless = Some(BoolKeep::True);
        } else if arg == "lossy" {
            set_lossless = Some(BoolKeep::False);
        } else if arg == "keep" {
            set_lossless = Some(BoolKeep::Keep);
        } else if arg == "progressive" && format == OutputFormat::Jpeg {
            i.jpeg_progressive = Some(true);
        } else if arg == "baseline" && format == OutputFormat::Jpeg {
            i.jpeg_progressive = Some(false);
        } else if arg.starts_with("d")
            || arg.starts_with("s")
            || arg.starts_with("mq")
            || arg.starts_with("q")
            || arg.starts_with("e")
        {
            let substr = arg.strip_prefix("mq").unwrap_or(&arg[1..]);
            if let Ok(s) = substr.parse::<f32>() {
                match arg.get(0..1).unwrap() {
                    "d" => {
                        if format == OutputFormat::Jxl {
                            i.jxl_distance = Some(s);
                        } else {
                            warnings.push(ParseWarning::ValueInvalid((
                                srcset_syntax_message_for(format),
                                arg.to_string(),
                            )));
                        }
                    }
                    "e" => {
                        if format == OutputFormat::Jxl {
                            i.jxl_effort = Some(s.clamp(0.0, 255.0) as u8);
                        } else {
                            warnings.push(ParseWarning::ValueInvalid((
                                srcset_syntax_message_for(format),
                                arg.to_string(),
                            )));
                        }
                    }
                    "m" => {
                        if format == OutputFormat::Png {
                            i.png_min_quality = Some(s.clamp(0.0, 100.0) as u8);
                        } else {
                            warnings.push(ParseWarning::ValueInvalid((
                                srcset_syntax_message_for(format),
                                arg.to_string(),
                            )));
                        }
                    }
                    "s" => {
                        if format == OutputFormat::Avif {
                            i.avif_speed = Some(s.clamp(0.0, 255.0) as u8);
                        } else {
                            warnings.push(ParseWarning::ValueInvalid((
                                srcset_syntax_message_for(format),
                                arg.to_string(),
                            )));
                        }
                    }
                    "q" => {
                        quality = Some(s);
                    }
                    _ => unreachable!(),
                }
            } else {
                warnings.push(ParseWarning::ValueInvalid((
                    srcset_syntax_message_for(format),
                    arg.to_string(),
                )));
            }
        } else if let Ok(v) = arg.parse::<f32>() {
            quality = Some(v);
        } else {
            warnings.push(ParseWarning::ValueInvalid((
                srcset_syntax_message_for(format),
                arg.to_string(),
            )));
        }
    }
    // Set the global lossless regardless of where lossless/lossy appears in the syntax, if format=auto
    if let Some(lossless) = set_lossless {
        if format == OutputFormat::Auto {
            i.lossless = Some(lossless);
        }
    }
    // Now, set i.format, the appropriate quality and lossless values
    let max_count;
    match format {
        OutputFormat::Webp => {
            i.format = Some(OutputFormat::Webp);
            if let Some(quality) = quality {
                i.webp_quality = Some(quality);
            }
            if let Some(lossless) = set_lossless {
                i.webp_lossless = Some(lossless);
            }
            max_count = 2; //lossless|keep,quality
        }
        OutputFormat::Jxl => {
            i.format = Some(OutputFormat::Jxl);
            if let Some(quality) = quality {
                i.jxl_quality = Some(quality);
            }
            if let Some(lossless) = set_lossless {
                i.jxl_lossless = Some(lossless);
            }
            max_count = 4; //lossless|keep,quality,distance,effort
        }
        OutputFormat::Avif => {
            i.format = Some(OutputFormat::Avif);
            if let Some(quality) = quality {
                i.avif_quality = Some(quality);
            }
            max_count = 2; //quality,speed
        }
        OutputFormat::Jpeg => {
            i.format = Some(OutputFormat::Jpeg);
            if let Some(quality) = quality {
                i.jpeg_quality = Some(quality as i32);
            }
            max_count = 2; //quality,progressive|baseline
        }
        OutputFormat::Png => {
            i.format = Some(OutputFormat::Png);
            if let Some(quality) = quality {
                i.png_quality = Some(quality as u8);
            }
            if let Some(lossless) = set_lossless {
                i.png_lossless = Some(lossless);
            }
            max_count = 3; //quality,lossless,min_quality
        }
        OutputFormat::Auto => {
            i.format = Some(OutputFormat::Auto);
            max_count = 1; //lossless
        }
        other => {
            i.format = Some(other);
            max_count = 0;
        }
    }
    if count > max_count {
        warnings.push(ParseWarning::ValueInvalid((
            srcset_syntax_message_for(format),
            format!("too many arguments:{}", srcset),
        )));
    }
}

pub fn apply_srcset_string(i: &mut Instructions, srcset: &str, warnings: &mut Vec<ParseWarning>) {
    if srcset.is_empty() {
        return;
    }
    // split srcset into commands by comma delimiter and iterate.
    let mut modes_specified = 0;
    let mut formats_specified = 0;

    for command_untrimmed in srcset.to_ascii_lowercase().split(",") {
        let command = command_untrimmed.trim();
        // split command into arguments by hyphen delimiter and iterate
        let mut args = command.split("-");
        if let Some(command_name) = args.next() {
            // if the command is webp, jpeg, gif, png, jxl, avif, send the rest of the args to parse_format_tuning
            // if the command is crop, and additional parameters are specified, there must be 4 and they must parse as floats or log a warning
            // if the command is a float followed by 'x', set i.zoom
            // if the command is a float followed by 'w', set i.width
            // if the command is a float followed by 'h', set i.height
            // if the command is 'fit-crop'  set mode=crop
            // if the command is 'fit-pad' set mode=pad
            // if the command is 'fit-max' set mode=pad
            // otherwise set mode=max
            // if the command is 'upscale', set i.scale = both
            i.mode = Some(FitMode::Max);

            let format = match OutputFormat::from_str(command_name) {
                Ok(format) => Some(format),
                Err(_) if command_name.eq_ignore_ascii_case("lossy") => {
                    i.lossless = Some(BoolKeep::False);
                    Some(OutputFormat::Auto)
                }
                Err(_) if command_name.eq_ignore_ascii_case("lossless") => {
                    i.lossless = Some(BoolKeep::True);
                    Some(OutputFormat::Auto)
                }
                Err(_) => None,
            };

            match command_name {
                _ if format.is_some() => {
                    parse_format_tuning(
                        format.unwrap(),
                        i.lossless,
                        srcset,
                        &mut args,
                        i,
                        warnings,
                    );
                    formats_specified += 1;
                }

                "qp" => {
                    const QP_SYNTAX: &str = "qp-[lowest|low|medium|good|high|highest|lossless|number] or qp-dpr-[number]";
                    // dpr/dppx indicates the 3rd is a number, might have trailing x
                    // otherwise, try
                    if let Some(arg1) = args.next() {
                        if arg1 == "dpr" || arg1 == "dppx" {
                            // dpr/dppx indicates the 3rd is a number, might have trailing x
                            if let Some(arg2) = args.next() {
                                let number_text = arg2.strip_suffix('x').unwrap_or(arg2);
                                if let Ok(v) = number_text.parse::<f32>() {
                                    i.qp_dpr = Some(f32::max(0.0, v));
                                } else {
                                    warnings.push(ParseWarning::ValueInvalid((QP_SYNTAX, format!("qp-dpr- must be followed by a valid number, got {} instead", number_text))));
                                }
                            } else {
                                warnings.push(ParseWarning::ValueInvalid((
                                    QP_SYNTAX,
                                    format!("qp-dpr- must be followed by a number: {}", srcset),
                                )));
                            }
                        } else if let Some(profile) = QualityProfile::parse(arg1) {
                            i.qp = Some(profile);
                        } else {
                            warnings.push(ParseWarning::ValueInvalid((
                                QP_SYNTAX,
                                format!("qp not followed by a profile name or dpr: {}", srcset),
                            )));
                        }
                    } else {
                        warnings.push(ParseWarning::ValueInvalid((
                            QP_SYNTAX,
                            format!("qp not followed by a profile name or dpr: {}", srcset),
                        )));
                    }
                }
                "crop" => {
                    let crop_x1 = args.next();
                    let crop_y1 = args.next();
                    let crop_x2 = args.next();
                    let crop_y2 = args.next();
                    if let (Some(cx1), Some(cy1), Some(cx2), Some(cy2)) =
                        (crop_x1, crop_y1, crop_x2, crop_y2)
                    {
                        i.mode = Some(FitMode::Max);
                        if let Ok(crop_x1) = cx1.parse::<f64>() {
                            if let Ok(crop_y1) = cy1.parse::<f64>() {
                                if let Ok(crop_x2) = cx2.parse::<f64>() {
                                    if let Ok(crop_y2) = cy2.parse::<f64>() {
                                        i.crop = Some([crop_x1, crop_y1, crop_x2, crop_y2]);
                                        i.cropxunits = Some(100.0);
                                        i.cropyunits = Some(100.0);
                                        i.mode = Some(FitMode::Max);
                                    } else {
                                        warnings.push(ParseWarning::ValueInvalid((
                                            "srcset=crop-x1,y1,x2,[y2]",
                                            cy2.to_owned(),
                                        )));
                                    }
                                } else {
                                    warnings.push(ParseWarning::ValueInvalid((
                                        "srcset=crop-x1,y1,[x2],y2",
                                        cx2.to_string(),
                                    )));
                                }
                            } else {
                                warnings.push(ParseWarning::ValueInvalid((
                                    "srcset=crop-x1,[y1],x2,y2",
                                    cy1.to_string(),
                                )));
                            }
                        } else {
                            warnings.push(ParseWarning::ValueInvalid((
                                "srcset=crop-[x1],y1,x2,y2",
                                cx1.to_string(),
                            )));
                        }
                    } else {
                        warnings.push(ParseWarning::ValueInvalid((
                            "srcset=crop-x1,y1,x2,y2",
                            "crop requires 4 parameters".to_string(),
                        )));
                    }
                }
                "fit" => {
                    if let Some(fit) = args.next() {
                        // can be 'pad', 'crop', 'max', 'distort'

                        match fit {
                            "pad" => {
                                i.mode = Some(FitMode::Pad);
                                modes_specified += 1;
                            }
                            "crop" | "cover" => {
                                i.mode = Some(FitMode::Crop); // CSS cover up-scales, we don't
                                modes_specified += 1;
                            }
                            "max" | "scale" | "contain" => {
                                i.mode = Some(FitMode::Max); // CSS contain up-scales, we don't
                                modes_specified += 1;
                            }
                            "distort" | "fill" => {
                                i.mode = Some(FitMode::Stretch);
                                modes_specified += 1;
                            }
                            _ => {
                                warnings.push(ParseWarning::ValueInvalid((
                                    "srcset=fit-[pad|crop|max|distort]",
                                    srcset.to_string(),
                                )));
                            }
                        }
                    } else {
                        warnings.push(ParseWarning::ValueInvalid((
                            "srcset=fit-[pad|crop|max|distort]",
                            "missing fit mode".to_string(),
                        )));
                    }
                }
                "upscale" => {
                    i.scale = Some(ScaleMode::Both);
                }
                "sharp" | "sharpen" => {
                    // parse sharpen
                    if let Some(sharpen) = args.next() {
                        //TODO: support sharpen=auto, that scales with qp-dpr/dpr/dppx
                        if let Ok(sharpen) = sharpen.parse::<f32>() {
                            i.f_sharpen = Some(sharpen);
                        } else {
                            warnings.push(ParseWarning::ValueInvalid((
                                "srcset=sharpen-",
                                sharpen.to_string(),
                            )));
                        }
                    } else {
                        warnings.push(ParseWarning::ValueInvalid((
                            "srcset=sharpen-[0-100]",
                            "missing sharpen value".to_string(),
                        )));
                    }
                }
                other => {
                    // if it ends in w, h, or x, it's a size
                    // parse it
                    if other.ends_with('h') || other.ends_with('x') || other.ends_with('w') {
                        let float_value = other[..other.len() - 1].parse::<f32>();
                        if let Ok(float_value) = float_value {
                            if other.ends_with('h') {
                                i.h = Some(float_value.round() as i32);
                            } else if other.ends_with('x') {
                                i.zoom = Some(float_value);
                            } else if other.ends_with('w') {
                                i.w = Some(float_value.round() as i32);
                            }
                        } else {
                            warnings.push(ParseWarning::ValueInvalid((
                                "srcset=[value][w|h|x]",
                                other.to_string(),
                            )));
                        }
                    } else {
                        // collect
                        warnings.push(ParseWarning::ValueInvalid((
                            "srcset=command,command,[unrecognized]",
                            format!("unrecognized command: {} from srcset={}", other, srcset),
                        )));
                    }
                }
            }
        }
    }

    if modes_specified > 1 {
        warnings.push(ParseWarning::ValueInvalid((
            "srcset",
            format!("multiple modes specified: {}", srcset),
        )));
    }
    if formats_specified > 1 {
        warnings.push(ParseWarning::ValueInvalid((
            "srcset",
            format!("multiple formats specified: {}", srcset),
        )));
    }
    // ok
}
