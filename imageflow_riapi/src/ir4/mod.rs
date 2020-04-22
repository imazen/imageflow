use imageflow_helpers::preludes::from_std::*;
use imageflow_types as s;

pub mod parsing;
mod layout;

use crate::sizing;
use crate::sizing::prelude::*;
use crate::ir4::parsing::*;
use crate::ir4::layout::*;

pub use layout::ConstraintResults;

pub fn process_constraint(source_w: i32, source_h: i32, constraint: &imageflow_types::Constraint) -> sizing::Result<ConstraintResults>{
    layout::Ir4Layout::process_constraint(source_w,source_h, constraint)
}

pub enum Ir4Command{
    Instructions(Box<Instructions>),
    Url(String),
    QueryString(String)
}

impl Ir4Command{
    pub fn parse(&self) -> sizing::Result<Ir4Result> {
        let (i, warn) = match *self {
            Ir4Command::Url(ref url) => parsing::parse_url(&::url::Url::from_str(url).expect("ImageResizer4 Url cannot be parsed into instructions: invalid URI")),
            Ir4Command::Instructions(ref i) => (**i, vec![]),
            Ir4Command::QueryString(ref s) => {
                let url = ::url::Url::from_str(&format!("https://fakeurl/img.jpg?{}", s)).expect("Must be a valid querystring, excluding ?");
                parsing::parse_url(&url)
            }
        };
        Ok(Ir4Result{
            parse_warnings: warn,
            parsed: i,
            steps: None,
            canvas: None
        })
    }

}

/// Minimal translation into framewise (delay as much as possible)
pub struct Ir4Translate{
    pub i: Ir4Command,
    pub decode_id: Option<i32>,
    pub encode_id: Option<i32>,
}

// If using trim.threshold, delayed expansion is required.


pub struct Ir4Result{
    pub parse_warnings: Vec<parsing::ParseWarning>,
    pub parsed: Instructions,
    pub steps: Option<Vec<s::Node>>,
    pub canvas: Option<AspectRatio>
}

impl Ir4Translate{



    pub fn get_decode_node(&self) -> Option<s::Node>{
        if let Some(id) = self.decode_id {
            Some(s::Node::Decode { io_id: id, commands: None })
        }else{
            None
        }
    }

    pub fn translate(&self) -> sizing::Result<Ir4Result> {
        let mut r = self.i.parse()?;
        let mut b = crate::ir4::layout::FramewiseBuilder::new();
        //Expand decoder early if trimming
        let delayed_id = if r.parsed.trim_whitespace_threshold.is_some() {
            if let Some(n) = self.get_decode_node() {
                b.add(n);
            }
            None
        } else {
            self.decode_id
        };
        // Add CropWhitespace
        if r.parsed.trim_whitespace_threshold.is_some(){
            b.add(s::Node::CropWhitespace {
                threshold: cmp::max(0,r.parsed.trim_whitespace_threshold.unwrap()) as u32,
                percent_padding: r.parsed.trim_whitespace_padding_percent.unwrap_or(0f64) as f32
            });
        }

        //delete whitespace from instructions
        let mut without_trimming: Instructions = r.parsed;
        without_trimming.trim_whitespace_padding_percent = None;
        without_trimming.trim_whitespace_threshold = None;

        b.add(s::Node::CommandString {
            kind: s::CommandStringKind::ImageResizer4,
            value: without_trimming.to_string(),
            decode: delayed_id,
            encode: self.encode_id
        });

        r.steps = Some(b.into_steps());
        Ok(r)
    }
}

pub struct Ir4SourceFrameInfo{
    pub w: i32,
    pub h: i32,
    pub fmt: s::PixelFormat,
    pub original_mime: Option<String>,
}

impl Ir4SourceFrameInfo{

    fn get_format_from_mime(&self) -> Option<OutputFormat>{
        self.original_mime.as_ref().and_then(|f|
                    match f.as_str(){
                        "image/jpeg" => Some(OutputFormat::Jpeg),
                        "image/png" => Some(OutputFormat::Png),
                        "image/gif" => Some(OutputFormat::Gif),
                        "image/webp" => Some(OutputFormat::Webp),
                        _ => None
                    })
    }
    fn get_format_from_frame(&self) -> OutputFormat{
        match self.fmt{
            s::PixelFormat::Bgr24 | s::PixelFormat::Bgr32 => OutputFormat::Jpeg,
            _ => OutputFormat::Png
        }
    }
}

/// Cannot expand decoder. use `Ir4Translate` for that.
pub struct Ir4Expand{
    pub i: Ir4Command,
    pub source: Ir4SourceFrameInfo,
    pub encode_id: Option<i32>,

}

impl Ir4Expand{

    pub fn get_decode_commands(&self) -> sizing::Result<Option<Vec<s::DecoderCommand>>> { //TODO: consider smallvec or generalizing decoder hints
        let i = self.i.parse()?.parsed;

        // Default to gamma correct
        let gamma_correct = i.down_colorspace != Some(ScalingColorspace::Srgb);

        let layout = self.get_layout(&i)?;
        let (from, to): (AspectRatio, AspectRatio) = layout.get_downscaling()?;

        let downscale_ratio = (f64::from(from.w) / f64::from(to.w)).min(f64::from(from.h) / f64::from(to.w));

        let preshrink_ratio = i.min_precise_scaling_ratio.unwrap_or(2.1f64) / downscale_ratio;

        let scaled_width = (f64::from(self.source.w) * preshrink_ratio).floor() as i64;
        let scaled_height = (f64::from(self.source.h) * preshrink_ratio).floor() as i64;
        if preshrink_ratio < 1f64 {
            let mut vec = Vec::with_capacity(2);
            vec.push(s::DecoderCommand::JpegDownscaleHints(s::JpegIDCTDownscaleHints {
                scale_luma_spatially: Some(gamma_correct),
                gamma_correct_for_srgb_during_spatial_luma_scaling: Some(gamma_correct),
                width: scaled_width,
                height: scaled_height
            }));
            if !gamma_correct{
                vec.push(s::DecoderCommand::WebPDecoderHints(s::WebPDecoderHints{
                    width: scaled_width as i32,
                    height: scaled_height as i32,
                }));
            }
            Ok(Some(vec))
        } else {
            Ok(None)
        }
    }

    pub fn get_canvas_size(&self) -> sizing::Result<AspectRatio>{
        let i = self.i.parse()?.parsed;
        let (_, layout) = self.get_layout(&i).unwrap().get_crop_and_layout().unwrap();
        Ok(layout.get_box(BoxTarget::CurrentCanvas))
    }

    pub fn get_layout(&self, i: &Instructions) -> sizing::Result<Ir4Layout> {
        if i.trim_whitespace_threshold.is_some() {
            return Err(sizing::LayoutError::ContentDependent);
        }
        Ok(layout::Ir4Layout::new(*i, self.source.w, self.source.h))
    }

    pub fn expand_steps(&self) -> sizing::Result<Ir4Result> {
        let mut r = self.i.parse()?;

        let layout = self.get_layout(&r.parsed)?;

        let mut b = FramewiseBuilder::new();
        r.canvas = Some(layout.add_steps(&mut b)?.canvas);

        if let Some(n) = self.get_encoder_node(&r.parsed) {
            b.add(n);
        }
        r.steps = Some(b.into_steps());
        Ok(r)

    }

    pub fn get_encoder_node(&self, i: &Instructions) -> Option<s::Node>{

        if let Some(id) = self.encode_id {

            let format = i.format.or_else(|| self.source.get_format_from_mime())
                .unwrap_or_else(|| self.source.get_format_from_frame());

            let encoder = match format {
                OutputFormat::Gif => s::EncoderPreset::Gif,
                OutputFormat::Jpeg => s::EncoderPreset::Mozjpeg {
                    quality: Some(i.quality.unwrap_or(90) as u8),
                    progressive: i.jpeg_progressive
                },
                // TODO: introduce support for 24-bit png and self.i.bgcolor_srgb (matte)
                OutputFormat::Png  => s::EncoderPreset::Libpng {
                    depth: Some(if i.bgcolor_srgb.is_some() { s::PngBitDepth::Png24 } else { s::PngBitDepth::Png32 }),
                    zlib_compression: None,
                    matte: i.bgcolor_srgb.map(|sr| s::Color::Srgb(s::ColorSrgb::Hex(sr.to_rrggbbaa_string())))
                },
                OutputFormat::Webp if i.webp_lossless == Some(true) => s::EncoderPreset::WebPLossless,
                OutputFormat::Webp => s::EncoderPreset::WebPLossy {
                    quality: i.webp_quality.unwrap_or(90f64) as f32
                },
            };
            Some(s::Node::Encode { io_id: id, preset: encoder })
        }else{
            None
        }
    }
}

