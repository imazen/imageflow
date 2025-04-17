use imageflow_types::json_messages::*;
// use imageflow_helpers::preludes::from_std::*;
// Reference @riapi.mdc, parsing.rs, encoder.rs, the docs, and srcset.rs. Read all of imageflow_types::json_messages::*
// Create builder functions to make structuring the schema as easy as possible
// Group keys the way they're grouped in docs/src/querystring/*, and bring in all the knoweldge from the docs
// Our end goal is to regenerate better docs from this schema.

// pub static IR4_KEYS: [&'static str;100] = [
//     "mode", "anchor", "flip", "sflip", "scale", "cache", "process",
//     "quality", "jpeg.quality", "zoom", "crop", "cropxunits", "cropyunits",
//     "w", "h", "width", "height", "maxwidth", "maxheight", "format", "thumbnail",
//      "autorotate", "srotate", "rotate", "ignoreicc", "ignore_icc_errors", //really? : "precise_scaling_ratio",
//     "stretch", "webp.lossless", "webp.quality", "watermark_red_dot",
//     "frame", "page", "subsampling", "colors", "f.sharpen", "f.sharpen_when", "down.colorspace",
//     "404", "bgcolor", "paddingcolor", "bordercolor", "preset", "floatspace",
//     "jpeg_idct_downscale_linear", "watermark", "s.invert", "s.sepia", "s.grayscale", "s.alpha",
//     "s.brightness", "s.contrast", "s.saturation",  "trim.threshold", "trim.percentpadding",
//     "a.blur", "a.sharpen", "a.removenoise", "a.balancewhite", "dither","jpeg.progressive",
//     "jpeg.turbo", "encoder", "decoder", "builder", "s.roundcorners", "paddingwidth",
//     "paddingheight", "margin", "borderwidth", "decoder.min_precise_scaling_ratio",
//     "png.quality","png.min_quality", "png.quantization_speed", "png.libpng", "png.max_deflate",
//     "png.lossless", "up.filter", "down.filter", "dpr", "dppx", "up.colorspace", "srcset", "short","accept.webp",
//     "accept.avif","accept.jxl", "accept.color_profiles", "c", "c.gravity", "qp", "qp.dpr", "qp.dppx",
//     "avif.speed", "avif.quality", "jxl.effort", "jxl.distance", "jxl.quality", "jxl.lossless", "jpeg.li", "lossless"];

pub fn get_query_string_schema() -> Result<QueryStringSchema, String> {
    Ok(QueryStringSchema {
        key_names: crate::ir4::parsing::IR4_KEYS.iter().map(|s| s.to_string()).collect(),
        // keys: vec![],
        // groups: vec![],
        // markdown_pages: vec![],
    })
}

pub fn get_query_string_keys() -> Result<QueryStringSchema, String> {
    Ok(QueryStringSchema {
        key_names: crate::ir4::parsing::IR4_KEYS.iter().map(|s| s.to_string()).collect(),
        // keys: vec![],
        // groups: vec![],
        // markdown_pages: vec![],
    })
}
