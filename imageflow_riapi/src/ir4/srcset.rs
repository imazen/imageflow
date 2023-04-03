// &srcset syntax is a comma-delimited short-form ideal for using in srcset attributes
// https://github.com/imazen/imageflow/issues/629
// Examples:
// * &srcset=webp-70,sharp-15,100w
// * &srcset=jpeg-80,2x,100w,sharpen-20
// * &srcset=png-90,2.5x,100w,100h,fit-crop
// * &srcset=png-lossless
// * &srcset=gif,crop-20-30-90-100,2.5x,100w,100h
// * &srcset=webp-l,2.5x,100w,100h,fit-crop
// * &srcset=webp-lossless,2.5x,100w,100h,upscale
// if the format -value is not specified, the defaults will be used. webp will be lossy, and png will be lossy if png.quality is not specified or png.lossless=false

// Some unexpected changes from the original spec: the default mode is max, not pad, and the default cropxunits/cropyunits is percent, not pixels.

// This function mutates a Instructions::new() to add the values from the srcset string, overriding existing values, and returns Vec<ParseWarning>

// The srcset string is a comma-delimited list of commands. Each command is a hyphen-delimited list of values. The first value is the command name, and the rest are arguments.

use super::parsing::{FitMode, ScaleMode, OutputFormat, ParseWarning, Instructions};


pub fn apply_srcset_string (i: &mut Instructions, srcset: &str, warnings: &mut Vec<ParseWarning>) {

    if srcset.is_empty() {
        return;
    }
    // split srcset into commands by comma delimiter and iterate.
    let mut modes_specified = 0;
    let mut formats_specified = 0;

    for command_untrimmed in srcset.split(",") {
        let command = command_untrimmed.trim();
        // split command into arguments by hyphen delimiter and iterate
        let mut args = command.split("-");
        if let Some(command_name) = args.next() {

            // if the command is webp, jpeg, or png, add to warnings if the parameter does not parse as a float
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

            match command_name {
                "webp" => {
                    i.format = Some(OutputFormat::Webp);
                    if let Some(quality) = args.next() {
                        if quality == "lossless" || quality == "l" {
                            i.webp_lossless = Some(true);
                        }else if let Ok(quality) = quality.parse::<f64>() {
                            i.webp_quality = Some(quality);
                        }else{
                            warnings.push(ParseWarning::ValueInvalid(("srcset=webp-[quality]", quality.to_string())));
                        }
                    }
                    if let Some(excess) = args.next(){
                        warnings.push(ParseWarning::ValueInvalid(("srcset=webp-[quality]", "excess parameter: ".to_string() + excess)));
                    }
                    formats_specified += 1;
                },
                "jpeg" => {
                    i.format = Some(OutputFormat::Jpeg);
                    if let Some(quality) = args.next() {
                        if let Ok(quality) = quality.parse::<i32>() {
                            i.quality = Some(quality);
                        }else{
                            warnings.push(ParseWarning::ValueInvalid(("jpeg", quality.to_string())));
                        }
                        if let Some(excess) = args.next(){
                            warnings.push(ParseWarning::ValueInvalid(("srcset=jpeg-[quality]", "excess parameter: ".to_string() + excess)));
                        }
                    }
                    formats_specified += 1;
                }

                "png" => {
                    i.format = Some(OutputFormat::Png);
                    if let Some(quality) = args.next() {
                        if quality == "lossless" || quality == "l"{
                            i.png_lossless = Some(true);
                        } else if let Ok(quality) = quality.parse::<u8>() {
                            i.png_quality = Some(quality);
                        }else{
                            warnings.push(ParseWarning::ValueInvalid(("png", quality.to_string())));
                        }
                        if let Some(excess) = args.next(){
                            warnings.push(ParseWarning::ValueInvalid(("srcset=jpeg-[quality]", "excess parameter: ".to_string() + excess)));
                        }
                    }else{
                        i.png_lossless = Some(true);
                    }
                    formats_specified += 1;
                },
                "gif" => {
                    i.format = Some(OutputFormat::Gif);
                    if let Some(excess) = args.next(){
                        warnings.push(ParseWarning::ValueInvalid(("srcset=gif", "excess parameter: ".to_string() + excess)));
                    }
                    formats_specified += 1;
                },
                "crop" => {
                    let crop_x1 = args.next();
                    let crop_y1 = args.next();
                    let crop_x2 = args.next();
                    let crop_y2 = args.next();
                    if crop_x1.is_some() && crop_y1.is_some() && crop_x2.is_some() && crop_y2.is_some() {

                        i.mode = Some(FitMode::Max);
                        if let Ok(crop_x1) = crop_x1.unwrap().parse::<f64>() {
                            if let Ok(crop_y1) = crop_y1.unwrap().parse::<f64>() {
                                if let Ok(crop_x2) = crop_x2.unwrap().parse::<f64>() {
                                    if let Ok(crop_y2) = crop_y2.unwrap().parse::<f64>() {
                                        i.crop = Some([crop_x1, crop_y1, crop_x2, crop_y2]);
                                        i.cropxunits = Some(100.0);
                                        i.cropyunits = Some(100.0);
                                        i.mode= Some(FitMode::Max);
                                    }else{
                                        warnings.push(ParseWarning::ValueInvalid(("srcset=crop-x1,y1,x2,[y2]", crop_y2.unwrap().to_owned())));
                                    }

                                }else{
                                    warnings.push(ParseWarning::ValueInvalid(("srcset=crop-x1,y1,[x2],y2", crop_x2.unwrap().to_string())));
                                }
                            }else{
                                warnings.push(ParseWarning::ValueInvalid(("srcset=crop-x1,[y1],x2,y2", crop_y1.unwrap().to_string())));
                            }
                        }else{
                            warnings.push(ParseWarning::ValueInvalid(("srcset=crop-[x1],y1,x2,y2", crop_x1.unwrap().to_string())));
                        }
                    }else{
                        warnings.push(ParseWarning::ValueInvalid(("srcset=crop-x1,y1,x2,y2", "crop requires 4 parameters".to_string())));
                    }

                },
                "fit" => {
                    if let Some(fit) = args.next() {
                        // can be 'pad', 'crop', 'max', 'distort'
                        match fit {
                            "pad" => {
                                i.mode = Some(FitMode::Pad);
                                modes_specified += 1;
                            },
                            "crop" => {
                                i.mode = Some(FitMode::Crop);
                                modes_specified += 1;
                            },
                            "max" => {
                                i.mode = Some(FitMode::Max);
                                modes_specified += 1;
                            },
                            "distort" => {
                                i.mode = Some(FitMode::Stretch);
                                modes_specified += 1;
                            },
                            _ => {
                                warnings.push(ParseWarning::ValueInvalid(("srcset=fit-[pad|crop|max|distort]", srcset.to_string())));
                            }
                        }
                    }else{
                        warnings.push(ParseWarning::ValueInvalid(("srcset=fit-[pad|crop|max|distort]", "missing fit mode".to_string())));
                    }
                },
                "upscale" => {
                    i.scale = Some(ScaleMode::Both);
                },
                "sharp" | "sharpen" => {
                    // parse sharpen
                    if let Some(sharpen) = args.next() {
                        if let Ok(sharpen) = sharpen.parse::<f64>() {
                            i.f_sharpen = Some(sharpen);
                        }else{
                            warnings.push(ParseWarning::ValueInvalid(("srcset=sharpen-", sharpen.to_string())));
                        }
                    }else{
                        warnings.push(ParseWarning::ValueInvalid(("srcset=sharpen-[0-100]", "missing sharpen value".to_string())));
                    }
                }
                other => {
                    // if it ends in w, h, or x, it's a size
                    // parse it
                    if other.ends_with('h') || other.ends_with('x') || other.ends_with('w') {
                        let float_value = other[..other.len()-1].parse::<f64>();
                        if let Ok(float_value) = float_value {
                            if other.ends_with('h') {
                                i.h = Some(float_value.round() as i32);
                            }else if other.ends_with('x') {
                                i.zoom = Some(float_value);
                            }else if other.ends_with('w') {
                                i.w = Some(float_value.round() as i32);
                            }
                        }else{
                            warnings.push(ParseWarning::ValueInvalid(("srcset=[value][w|h|x]", other.to_string())));
                        }
                    }else{
                        // collect
                        warnings.push(ParseWarning::ValueInvalid(("srcset=command,command,[unrecognized]", format!("unrecognized command: {} from srcset={}", other, srcset))));
                    }
                }
            }
        }
    }

    if modes_specified > 1 {
        warnings.push(ParseWarning::ValueInvalid(("srcset", format!("multiple modes specified: {}", srcset))));
    }
    if formats_specified > 1 {
        warnings.push(ParseWarning::ValueInvalid(("srcset", format!("multiple formats specified: {}", srcset))));
    }
        // ok
}

