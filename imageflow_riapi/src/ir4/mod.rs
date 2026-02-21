use imageflow_helpers::preludes::from_std::*;
use imageflow_types as s;

mod encoder;
mod layout;
pub mod parsing;
mod schema;
mod srcset;

use crate::ir4::layout::*;
use crate::ir4::parsing::*;
use crate::sizing;
use crate::sizing::prelude::*;

pub use layout::ConstraintResults;

pub fn get_query_string_schema() -> Result<s::json_messages::QueryStringSchema, String> {
    schema::get_query_string_schema()
}

pub fn get_query_string_keys() -> Result<s::json_messages::QueryStringSchema, String> {
    schema::get_query_string_keys()
}

pub fn validate_query_string(
    query_string: String,
) -> Result<s::json_messages::QueryStringValidationResults, String> {
    let url = format!("http://localhost/image.jpg?{}", query_string);
    let a = url::Url::from_str(&url).unwrap();
    let (_i, warns) = parse_url(&a);

    Ok(s::json_messages::QueryStringValidationResults {
        issues: warns.into_iter().map(|w| w.to_query_string_validation_issue()).collect(),
    })
}

pub fn process_constraint(
    source_w: i32,
    source_h: i32,
    constraint: &imageflow_types::Constraint,
) -> sizing::Result<ConstraintResults> {
    layout::Ir4Layout::process_constraint(source_w, source_h, constraint)
}

#[derive(Debug, Clone)]
pub enum Ir4Command {
    Instructions(Box<Instructions>),
    Url(String),
    QueryString(String),
}

impl Ir4Command {
    pub fn parse(&self) -> sizing::Result<Ir4Result> {
        let (i, warn) = match *self {
            Ir4Command::Url(ref url) => parsing::parse_url(
                &::url::Url::from_str(url)
                    .expect("ImageResizer4 Url cannot be parsed into instructions: invalid URI"),
            ),
            Ir4Command::Instructions(ref i) => (**i, vec![]),
            Ir4Command::QueryString(ref s) => {
                let url = ::url::Url::from_str(&format!("https://fakeurl/img.jpg?{}", s))
                    .expect("Must be a valid querystring, excluding ?");
                parsing::parse_url(&url)
            }
        };
        Ok(Ir4Result { parse_warnings: warn, parsed: i, steps: None, canvas: None })
    }
}

/// Minimal translation into framewise (delay as much as possible)
pub struct Ir4Translate {
    pub i: Ir4Command,
    pub decode_id: Option<i32>,
    pub encode_id: Option<i32>,
    pub watermarks: Option<Vec<imageflow_types::Watermark>>,
}

// If using trim.threshold, delayed expansion is required.

#[derive(Debug, Clone)]
pub struct Ir4Result {
    pub parse_warnings: Vec<parsing::ParseWarning>,
    pub parsed: Instructions,
    pub steps: Option<Vec<s::Node>>,
    pub canvas: Option<AspectRatio>,
}

impl Ir4Translate {
    pub fn get_decode_node_without_commands(&self) -> Option<s::Node> {
        self.decode_id.map(|id| s::Node::Decode { io_id: id, commands: None })
    }

    pub fn translate(&self) -> sizing::Result<Ir4Result> {
        let mut r = self.i.parse()?;
        let mut b = crate::ir4::layout::FramewiseBuilder::new();
        //Expand decoder early if trimming
        let delayed_id = if r.parsed.trim_whitespace_threshold.is_some() {
            if let Some(n) = self.get_decode_node_without_commands() {
                b.add(n);
            }
            None
        } else {
            self.decode_id
        };
        // Add CropWhitespace
        if let Some(threshold) = r.parsed.trim_whitespace_threshold {
            b.add(s::Node::CropWhitespace {
                threshold: cmp::max(0, threshold) as u32,
                percent_padding: r.parsed.trim_whitespace_padding_percent.unwrap_or(0f32),
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
            encode: self.encode_id,
            watermarks: self.watermarks.clone(),
        });

        r.steps = Some(b.into_steps());
        Ok(r)
    }
}

#[derive(Debug, Clone)]
pub struct Ir4SourceFrameInfo {
    pub w: i32,
    pub h: i32,
    pub fmt: s::PixelFormat,
    pub original_mime: Option<String>,
    pub lossless: bool,
}

/// Cannot expand decoder. use `Ir4Translate` for that.
#[derive(Debug, Clone)]
pub struct Ir4Expand {
    pub i: Ir4Command,
    pub source: Ir4SourceFrameInfo,
    /// The actual, not-pre-shrunk image width. May differ from the bitmap size during IDCT scaling
    pub reference_width: i32,
    /// The actual, not-pre-shrunk image height. May differ from the bitmap size during IDCT scaling
    pub reference_height: i32,
    pub encode_id: Option<i32>,
    pub watermarks: Option<Vec<imageflow_types::Watermark>>,
}

impl Ir4Expand {
    pub fn get_preshrink_ratio(&self) -> sizing::Result<f64> {
        let i = self.i.parse()?.parsed;

        let layout = self.get_layout(&i)?;
        let (from, to): (AspectRatio, AspectRatio) = layout.get_downscaling()?;

        let downscale_ratio =
            (f64::from(from.w) / f64::from(to.w)).min(f64::from(from.h) / f64::from(to.w));

        Ok(i.min_precise_scaling_ratio.unwrap_or(2.1f32) as f64 / downscale_ratio)
    }

    pub fn get_decode_commands(&self) -> sizing::Result<Option<Vec<s::DecoderCommand>>> {
        //TODO: consider smallvec or generalizing decoder hints
        let i = self.i.parse()?.parsed;

        // Default to gamma correct
        let gamma_correct = i.down_colorspace != Some(ScalingColorspace::Srgb);

        let preshrink_ratio = self.get_preshrink_ratio()?;

        let scaled_width = (f64::from(self.source.w) * preshrink_ratio).floor() as i64;
        let scaled_height = (f64::from(self.source.h) * preshrink_ratio).floor() as i64;
        let mut vec = Vec::with_capacity(4);

        if i.ignoreicc == Some(true) {
            vec.push(s::DecoderCommand::DiscardColorProfile);
        }
        if i.ignore_icc_errors == Some(true) {
            vec.push(s::DecoderCommand::IgnoreColorProfileErrors);
        }

        if preshrink_ratio < 1f64 {
            vec.push(s::DecoderCommand::JpegDownscaleHints(s::JpegIDCTDownscaleHints {
                scale_luma_spatially: Some(gamma_correct),
                gamma_correct_for_srgb_during_spatial_luma_scaling: Some(gamma_correct),
                width: scaled_width,
                height: scaled_height,
            }));
            if !gamma_correct {
                vec.push(s::DecoderCommand::WebPDecoderHints(s::WebPDecoderHints {
                    width: scaled_width as i32,
                    height: scaled_height as i32,
                }));
            }
        }
        if vec.is_empty() {
            Ok(None)
        } else {
            Ok(Some(vec))
        }
    }

    pub fn get_canvas_size(&self) -> sizing::Result<AspectRatio> {
        let i = self.i.parse()?.parsed;
        let (_, layout) = self.get_layout(&i).unwrap().get_crop_and_layout().unwrap();
        Ok(layout.get_box(BoxTarget::CurrentCanvas))
    }

    pub fn get_layout(&self, i: &Instructions) -> sizing::Result<Ir4Layout> {
        if i.trim_whitespace_threshold.is_some() {
            return Err(sizing::LayoutError::ContentDependent);
        }
        Ok(layout::Ir4Layout::new(
            *i,
            self.source.w,
            self.source.h,
            self.reference_width,
            self.reference_height,
        ))
    }

    pub fn expand_steps(&self) -> sizing::Result<Ir4Result> {
        let mut r = self.i.parse()?;

        let layout = self.get_layout(&r.parsed)?;

        let mut b = FramewiseBuilder::new();
        r.canvas = Some(layout.add_steps(&mut b, &self.watermarks)?.canvas);

        if let Some(n) = self.get_encoder_node(&r.parsed) {
            b.add(n);
        }
        r.steps = Some(b.into_steps());
        Ok(r)
    }

    pub fn get_encoder_node(&self, i: &Instructions) -> Option<s::Node> {
        if let Some(id) = self.encode_id {
            let preset = crate::ir4::encoder::calculate_encoder_preset(i);
            Some(s::Node::Encode { io_id: id, preset })
        } else {
            None
        }
    }
}
