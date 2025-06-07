#![allow(unused_imports)] // Temporary while moving code
use serde::{Serialize, Deserialize};

#[cfg(feature = "json-schema")]
use schemars::JsonSchema;

#[cfg(feature = "schema-export")]
use utoipa::ToSchema;

// Define placeholder derive macro and trait if features are off
#[cfg(not(feature = "json-schema"))]
#[macro_export]
macro_rules! JsonSchema { () => {}; }

#[cfg(not(feature = "json-schema"))]
pub trait JsonSchema {}

#[cfg(not(feature = "json-schema"))]
impl<T> JsonSchema for T {}

// Moved content starts here 

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct QueryStringValidationResults{
    pub issues: Vec<QueryStringValidationIssue>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct QueryStringValidationIssue{
    pub message: String,
    pub key: String,
    pub value: String,
    pub kind: QueryStringValidationIssueKind,
}
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum QueryStringValidationIssueKind{
    DuplicateKeyError,
    UnrecognizedKey,
    IgnoredKey,
    InvalidValueError,
    DeprecatedValueWarning,
    DeprecatedKeyWarning,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct QueryStringSchema{
    pub key_names: Vec<String>,
    // pub keys: Vec<QueryStringSchemaKey>,
    // pub groups: Vec<QueryStringSchemaKeyGroup>,
    // pub markdown_pages: Vec<QueryStringSchemaMarkdownPage>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct QueryStringSchemaMarkdownPage{
    pub slug: String,
    pub title: String,
    pub markdown: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum QueryStringDescription{
    #[serde(rename="markdown")]
    Markdown(String),
    #[serde(rename="text")]
    Text(String),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct QueryStringSchemaKeyGroup{
    pub id: String,
    pub name: String,
    pub description: QueryStringDescription,
    pub generated_markdown: Option<String>, // gener
    pub keys: Vec<String>,
    pub examples: Option<Vec<QueryStringSchemaExample>>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct QueryStringSchemaKey{
    pub key: String,
    pub aliases: Option<Vec<String>>,
    pub description: QueryStringDescription,
    pub ignored_reason: Option<String>,
    pub deprecation_message: Option<String>,
    pub interacts_with: Option<Vec<String>>,
    pub related_keys: Option<Vec<String>>,
    pub conflicts_with_keys: Option<Vec<String>>,
    pub allowed_values: Vec<QueryStringSchemaValue>,
    pub examples: Option<Vec<QueryStringSchemaExample>>,
    pub generated_markdown: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct QueryStringSchemaValue{
    pub example_value: Option<String>,
    pub value_syntax: Option<String>,
    pub data_validation: Option<QueryStringSchemaValueValidation>,
    pub description: QueryStringDescription,
    pub is_default: Option<bool>,
    pub ignored_reason: Option<String>,
    pub deprecation_message: Option<String>,
    pub examples: Option<Vec<QueryStringSchemaExample>>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub enum QueryStringSchemaValueValidation{
    // float, integer (both with optionalrange)
    // enum of strings
    // boolean
    // list of floats, specific count, optional range
    #[serde(rename="enum")]
    Enum { options: Vec<String>, case_sensitive: Option<bool> },
    // list of floats, specific count, optional range
    #[serde(rename="numeric_list")]
    NumberList { count: Option<usize>, ranges: Option<Vec<QueryStringSchemaValueRange>> },
    /// boolean values like 1, 0, true, false
    #[serde(rename="bool")]
    Bool,

    #[serde(rename="number")]
    Number(QueryStringSchemaValueRange),

    #[serde(rename="regex")]
    Regex { pattern: String },

    #[serde(rename="equals")]
    Equals { value: String, case_sensitive: Option<bool> },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct QueryStringSchemaValueRange{
    pub min: Option<f32>,
    pub max: Option<f32>,
    pub step_hint: Option<f32>,
    pub integer: Option<bool>,
}


#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
pub struct QueryStringSchemaExample{
    pub querystring: String,
    pub html_fragment: Option<String>,
    pub description: QueryStringDescription,
    pub generated_markdown: Option<String>,
}

// Need to import types used by the structs above if they are defined elsewhere
// Example: Assuming Response001, ResponsePayload etc. are defined in the parent (lib.rs)
// use crate::{Response001, ResponsePayload, ImageInfo, JobResult, EncodeResult, DecodeResult, ResultBytes, BuildPerformance, FramePerformance, NodePerf, VersionInfo, Build001, Build001Config, Build001GraphRecording, ExecutionSecurity, FrameSizeLimit, IoObject, IoDirection, IoEnum, Framewise, Graph, Node, Edge, EdgeKind, Constraint, ConstraintMode, ConstraintGravity, Color, ColorSrgb, ResampleHints, Filter, ScalingFloatspace, ResampleWhen, SharpenWhen, Watermark, WatermarkConstraintBox, WatermarkConstraintMode, CompositingMode, RoundCornersMode, CommandStringKind, PixelFormat, ColorFilterSrgb, EncoderPreset, QualityProfile, BoolKeep, AllowedFormats, EncoderHints, JpegEncoderHints, JpegEncoderStyle, PngEncoderHints, PngEncoderStyle, PngBitDepth, WebpEncoderHints, GifEncoderHints, GetImageInfo001, TellDecoder001, DecoderCommand, JpegIDCTDownscaleHints, WebPDecoderHints, Execute001, ValidateQueryString, EmptyRequest}; 