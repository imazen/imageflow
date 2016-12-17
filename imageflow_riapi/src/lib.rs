extern crate imageflow_types;
extern crate imageflow_helpers;
//use imageflow_helpers as hlp;
//use imageflow_types as s;
use imageflow_helpers::preludes::from_std::*;
extern crate url;
//use url::Url;

#[feature(fixed_size_array)]
mod sizing;
mod layout;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}

static IR4_KEYS: [&'static str;58] = ["mode", "anchor", "flip", "sflip", "scale", "cache", "process", "frame", "page", "quality", "subsampling", "colors", "zoom",
"crop", "cropxunits", "cropyunits", "w", "h", "width", "height", "maxwidth", "maxheight", "format", "thumbnail",
"precise_scaling_ratio", "autorotate", "srotate", "rotate", "ignoreicc", "404", "bgcolor", "paddingcolor", "bordercolor", "preset", "floatspace", "jpeg_idct_downscale_linear", "watermark",
"s.invert", "s.sepia", "s.grayscale", "s.alpha", "s.brightness", "s.contrast", "s.saturation", "trim.threshold", "trim.percentpadding", "a.blur", "a.sharpen", "a.removenoise", "dither",
"encoder", "decoder", "builder", "s.roundcorners.", "paddingwidth", "paddingheight", "margin", "borderwidth"];

static IF_KEYS: [&'static str;8] = ["w","h", "width", "height", "mode", "quality", "format", "autorotate"];

//pub fn parse_url(url: Url) -> s::Framewise{
//    url.query_pairs().filter(|(k,v)| IR4_keys.contains(k.to_lowercase()))
//
//    // Let various strategies attempt to handle all the querypairs that intersect with IR4/IF
//    // Leftovers cause a warning
//
//
//    //scale=up should cause a warning
//}



enum ConstraintMode{
    Max,
    Crop,
    Pad
}
enum Scaling{
    Down,
    Both,
    Canvas
}
enum AspectHandling{
    CropToRatioWhenSmaller,
    PadToRatioWhenSmaller,
    RevertToOrigin,
}
struct ConstraintParams{
    w: Option<u32>,
    h: Option<u32>,
    scale: Scaling,
    aspect_handling: AspectHandling,
    mode: ConstraintMode
}
