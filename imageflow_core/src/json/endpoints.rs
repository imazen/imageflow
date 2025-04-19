use crate::Context;
use crate::internal_prelude::works_everywhere::*;
use crate::json::*;
use crate::parsing::GraphTranslator;
use crate::parsing::IoTranslator;
use imageflow_types::*;
use std::error;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use super::parse_json;
// --- OpenAPI Schema Generation Support ---

#[cfg(feature = "schema-export")]
use utoipa::{OpenApi, ToSchema, Modify};
#[cfg(feature = "schema-export")]
use serde::{Serialize, Deserialize};

#[cfg(not(feature = "schema-export"))]
pub fn get_openapi_schema_json() -> Result<String> {
    Err(nerror!(ErrorKind::MethodNotImplemented))
}

// Define a struct to implement the Modify trait for customizing schema names
#[cfg(feature = "schema-export")]
struct SchemaNamer;

#[cfg(feature = "schema-export")]
impl Modify for SchemaNamer {
    fn modify(&self, schema: &mut utoipa::openapi::schema::Schema) {
        // Attempt to remove the module path prefix like `imageflow_type` or `json_message`
        if let Some(title) = schema.title.as_mut() {
            if let Some(last_part) = title.rsplit("::").next() {
                *title = last_part.to_string();
            }
        }
    }
}


pub fn route_inner(context: &mut Context, method: &str, json: &[u8]) -> Result<JsonResponse> {
    match method {
        "v1/query_string/latest/get_schema" => {
            let input = parse_json::<s::GetQueryStringSchema>(json)?;
            let output = v1::get_query_string_schema(context, input)?;
            Ok(JsonResponse::ok(output))
        },
        "v1/query_string/latest/list_supported_keys" => {
            let input = parse_json::<s::ListQueryStringKeys>(json)?;
            let output = v1::list_query_string_keys(context, input)?;
            Ok(JsonResponse::ok(output))
        },
        "v1/query_string/latest/validate" => {
            let input = parse_json::<s::ValidateQueryString>(json)?;
            let output = v1::validate_query_string(context, input)?;
            Ok(JsonResponse::ok(output))
        },
        "v1/build" | "v0.1/build" => {
            let input = parse_json::<s::Build001>(json)?;
            let output = v1::build(context, input)?;
            Ok(JsonResponse::ok(output))
        },
        "v1/get_image_info" | "v0.1/get_image_info" => {
            let input = parse_json::<s::GetImageInfo001>(json)?;
            let output = v1::get_image_info(context, input)?;
            Ok(JsonResponse::ok(output))
        },
        "v1/get_scaled_image_info" => {
            let input = parse_json::<s::GetImageInfo001>(json)?;
            let output = v1::get_scaled_image_info(context, input)?;
            Ok(JsonResponse::ok(output))
        },
        "v1/tell_decoder" | "v0.1/tell_decoder" => {
            let input = parse_json::<s::TellDecoder001>(json)?;
            let output = v1::tell_decoder(context, input)?;
            Ok(JsonResponse::ok(output))
        },
        "v1/execute" | "v0.1/execute" => {
            let input = parse_json::<s::Execute001>(json)?;
            let output = v1::execute(context, input)?;
            Ok(JsonResponse::ok(output))
        },
        "v1/get_version_info" => {
            let input = parse_json::<s::GetVersionInfo>(json)?;
            let output = v1::get_version_info(context, input)?;
            Ok(JsonResponse::ok(output))
        },
        "brew_coffee" => Ok(JsonResponse::teapot()),
        _ => Err(nerror!(ErrorKind::InvalidMessageEndpoint))
    }
}

// --- V1 API Handlers and Schemas ---

pub(crate) mod v1 {
    use super::*;
    use imageflow_types::*;
    use crate::internal_prelude::works_everywhere::serde_json::Error;

    #[cfg(feature = "schema-export")]
    use utoipa::ToSchema;
    #[cfg(feature = "schema-export")]
    use serde::{Serialize, Deserialize};

    // Generic wrapper for successful JSON responses (matches Response001 structure)
    #[cfg_attr(feature = "schema-export", derive(Serialize, Deserialize, ToSchema))]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct JsonAnswer<T> {
        #[cfg_attr(feature = "schema-export", schema(example = 200))]
        pub code: i64,
        #[cfg_attr(feature = "schema-export", schema(example = true))]
        pub success: bool,
        pub message: Option<String>,
        pub data: T, // Specific payload for the endpoint
    }

    // Specific wrapper for error JSON responses (matches Response001 structure)
    #[cfg_attr(feature = "schema-export", derive(Serialize, Deserialize, ToSchema))]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct JsonError {
        #[cfg_attr(feature = "schema-export", schema(example = 500))]
        pub code: i64,
        #[cfg_attr(feature = "schema-export", schema(example = false))]
        pub success: bool,
        #[cfg_attr(feature = "schema-export", schema(example = "Detailed error message"))]
        pub message: Option<String>,
        // Errors have no data payload
        #[cfg_attr(feature = "schema-export", schema(nullable = true, value_type = Option<Object>))]
        pub data: Option<serde_json::Value>, // Use Option<Value> which serializes to null
    }

    // --- Specific Success Response Structs ---

    #[cfg_attr(feature = "schema-export", derive(Serialize, Deserialize, ToSchema))]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct BuildV1Response { pub job_result: JobResult }

    #[cfg_attr(feature = "schema-export", derive(Serialize, Deserialize, ToSchema))]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct GetImageInfoV1Response { pub image_info: ImageInfo }

    #[cfg_attr(feature = "schema-export", derive(Serialize, Deserialize, ToSchema))]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct GetScaledImageInfoV1Response { pub image_info: ImageInfo }

    #[cfg_attr(feature = "schema-export", derive(Serialize, Deserialize, ToSchema))]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct TellDecoderV1Response { }

    #[cfg_attr(feature = "schema-export", derive(Serialize, Deserialize, ToSchema))]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct ExecuteV1Response { pub job_result: JobResult }

    #[cfg_attr(feature = "schema-export", derive(Serialize, Deserialize, ToSchema))]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct GetVersionInfoV1Response { pub version_info: VersionInfo }

    #[cfg_attr(feature = "schema-export", derive(Serialize, Deserialize, ToSchema))]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct GetQueryStringSchemaV1Response { pub schema: json_messages::QueryStringSchema }

    #[cfg_attr(feature = "schema-export", derive(Serialize, Deserialize, ToSchema))]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct ListQueryStringKeysV1Response { pub schema: json_messages::QueryStringSchema } // Reuse schema type

    #[cfg_attr(feature = "schema-export", derive(Serialize, Deserialize, ToSchema))]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct ValidateQueryStringV1Response { pub results: json_messages::QueryStringValidationResults }

    // --- Handler Functions ---

    #[cfg_attr(feature = "schema-export", utoipa::path(
        post,
        path = "/v1/build",
        request_body = Build001,
        responses(
            (status = 200, description = "Build successful", body = JsonAnswer<BuildV1Response>),
            (status = 500, description = "Build failed", body = JsonError)
        )
    ))]
    pub(super) fn build(context: &mut Context, parsed: Build001) -> Result<BuildV1Response> {
        let job_result = context.build_inner(parsed).map_err(|e| e.at(here!()))?;
        Ok(BuildV1Response { job_result })
    }

    #[cfg_attr(feature = "schema-export", utoipa::path(
        post,
        path = "/v1/get_image_info",
        request_body = GetImageInfo001,
        responses(
            (status = 200, description = "Image info retrieved", body = JsonAnswer<GetImageInfoV1Response>),
            (status = 500, description = "Failed to get image info", body = JsonError)
        )
    ))]
    pub(super) fn get_image_info(context: &mut Context, data: GetImageInfo001) -> Result<GetImageInfoV1Response> {
         let image_info = context.get_unscaled_rotated_image_info(data.io_id).map_err(|e| e.at(here!()))?;
         Ok(GetImageInfoV1Response { image_info })
    }

     #[cfg_attr(feature = "schema-export", utoipa::path(
        post,
        path = "/v1/get_scaled_image_info",
        request_body = GetImageInfo001,
        responses(
            (status = 200, description = "Scaled image info retrieved", body = JsonAnswer<GetScaledImageInfoV1Response>),
            (status = 500, description = "Failed to get scaled image info", body = JsonError)
        )
    ))]
    pub(super) fn get_scaled_image_info(context: &mut Context, data: GetImageInfo001) -> Result<GetScaledImageInfoV1Response> {
        let image_info = context.get_scaled_rotated_image_info(data.io_id).map_err(|e| e.at(here!()))?;
        Ok(GetScaledImageInfoV1Response { image_info })
    }

    #[cfg_attr(feature = "schema-export", utoipa::path(
        post,
        path = "/v1/tell_decoder",
        request_body = TellDecoder001,
        responses(
            (status = 200, description = "Decoder hints applied", body = JsonAnswer<TellDecoderV1Response>), // No data payload
            (status = 500, description = "Failed to apply decoder hints", body = JsonError)
        )
    ))]
    pub(super) fn tell_decoder(context: &mut Context, data: TellDecoder001) -> Result<TellDecoderV1Response> {
        context.tell_decoder(data.io_id, data.command).map_err(|e| e.at(here!()))?;
        Ok(TellDecoderV1Response {})
    }

    #[cfg_attr(feature = "schema-export", utoipa::path(
        post,
        path = "/v1/execute",
        request_body = Execute001,
        responses(
            (status = 200, description = "Execution successful", body = JsonAnswer<ExecuteV1Response>),
            (status = 500, description = "Execution failed", body = JsonError)
        )
    ))]
    pub(super) fn execute(context: &mut Context, parsed: Execute001) -> Result<ExecuteV1Response> {
        let job_result = context.execute_inner(parsed).map_err(|e| e.at(here!()))?;
        Ok(ExecuteV1Response { job_result })
    }

    #[cfg_attr(feature = "schema-export", utoipa::path(
        post,
        path = "/v1/get_version_info",
        request_body = GetVersionInfo,
        responses(
            (status = 200, description = "Version info retrieved", body = JsonAnswer<GetVersionInfoV1Response>),
            (status = 500, description = "Failed to get version info", body = JsonError)
        )
    ))]
    pub(super) fn get_version_info(context: &mut Context, _data: GetVersionInfo) -> Result<GetVersionInfoV1Response> {
        let version_info = context.get_version_info().map_err(|e| e.at(here!()))?;
        Ok(GetVersionInfoV1Response { version_info })
    }

    #[cfg_attr(feature = "schema-export", utoipa::path(
        post,
        path = "/v1/query_string/latest/get_schema",
        request_body = GetQueryStringSchema,
        responses(
            (status = 200, description = "Query string schema retrieved", body = JsonAnswer<GetQueryStringSchemaV1Response>),
            (status = 500, description = "Failed to get query string schema", body = JsonError)
        )
    ))]
    pub(crate) fn get_query_string_schema(_context: &mut Context, _data: GetQueryStringSchema) -> Result<GetQueryStringSchemaV1Response> {
        let schema = imageflow_riapi::ir4::get_query_string_schema().map_err(|e| nerror!(ErrorKind::InternalError, "{}", e))?;
        Ok(GetQueryStringSchemaV1Response { schema })
    }

    #[cfg_attr(feature = "schema-export", utoipa::path(
        post,
        path = "/v1/query_string/latest/list_supported_keys",
        request_body = ListQueryStringKeys,
        responses(
            (status = 200, description = "Supported keys listed", body = JsonAnswer<ListQueryStringKeysV1Response>),
            (status = 500, description = "Failed to list keys", body = JsonError)
        )
    ))]
    pub(super) fn list_query_string_keys(_context: &mut Context, _data: ListQueryStringKeys) -> Result<ListQueryStringKeysV1Response> {
        let schema = imageflow_riapi::ir4::get_query_string_keys().map_err(|e| nerror!(ErrorKind::InternalError, "{}", e))?;
        Ok(ListQueryStringKeysV1Response { schema })
    }

    #[cfg_attr(feature = "schema-export", utoipa::path(
        post,
        path = "/v1/query_string/latest/validate",
        request_body = ValidateQueryString,
        responses(
            (status = 200, description = "Query string validation results", body = JsonAnswer<ValidateQueryStringV1Response>),
            (status = 500, description = "Validation failed", body = JsonError)
        )
    ))]
    pub(super) fn validate_query_string(_context: &mut Context, data: ValidateQueryString) -> Result<ValidateQueryStringV1Response> {
        let results = imageflow_riapi::ir4::validate_query_string(data.query_string)
            .map_err(|e| nerror!(ErrorKind::InternalError, "{}", e))?;
        Ok(ValidateQueryStringV1Response { results })
    }

    // --- Main OpenAPI Documentation Struct ---
    #[cfg(feature = "schema-export")]
    #[derive(OpenApi)]
    #[openapi(
        paths(
            handle_build,
            handle_get_image_info,
            handle_get_scaled_image_info,
            handle_tell_decoder,
            handle_execute,
            handle_get_version_info,
            handle_get_query_string_schema,
            handle_list_query_string_keys,
            handle_validate_query_string,
        ),
        components(
            schemas(
                // Generic Response Wrappers
                JsonAnswer<BuildV1Response>, BuildV1Response,
                JsonAnswer<GetImageInfoV1Response>, GetImageInfoV1Response,
                JsonAnswer<GetScaledImageInfoV1Response>, GetScaledImageInfoV1Response,
                JsonAnswer<TellDecoderV1Response>, TellDecoderV1Response,
                JsonAnswer<ExecuteV1Response>, ExecuteV1Response,
                JsonAnswer<GetVersionInfoV1Response>, GetVersionInfoV1Response,
                JsonAnswer<GetQueryStringSchemaV1Response>, GetQueryStringSchemaV1Response,
                JsonAnswer<ListQueryStringKeysV1Response>, ListQueryStringKeysV1Response,
                JsonAnswer<ValidateQueryStringV1Response>, ValidateQueryStringV1Response,
                JsonError,

                // Core Request/Response types from imageflow_types (referenced by handlers/structs above)
                Response001, // Still useful to include the base structure
                ResponsePayload,
                ImageInfo,
                JobResult, EncodeResult, DecodeResult, ResultBytes, BuildPerformance, FramePerformance, NodePerf,
                VersionInfo,
                json_messageQueryStringSchema, json_messageQueryStringSchemaKey, json_messageQueryStringDescription,
                json_messageQueryStringSchemaValue, json_messageQueryStringSchemaValueValidation, json_messageQueryStringSchemaValueRange,
                json_messageQueryStringSchemaExample, json_messageQueryStringSchemaKeyGroup, json_messageQueryStringSchemaMarkdownPage,
                json_messageQueryStringValidationResults, json_messageQueryStringValidationIssue, json_messageQueryStringValidationIssueKind,
                Build001, Build001Config, Build001GraphRecording, ExecutionSecurity, FrameSizeLimit,
                IoObject, IoDirection, IoEnum,
                Framewise, Graph, Node, Edge, EdgeKind,
                Constraint, ConstraintMode, ConstraintGravity, Color, ColorSrgb,
                ResampleHints, Filter, ScalingFloatspace, ResampleWhen, SharpenWhen,
                Watermark, WatermarkConstraintBox, WatermarkConstraintMode,
                CompositingMode, RoundCornersMode, CommandStringKind, PixelFormat,
                ColorFilterSrgb,
                EncoderPreset, QualityProfile, BoolKeep, AllowedFormats, EncoderHints,
                JpegEncoderHints, JpegEncoderStyle, PngEncoderHints, PngEncoderStyle, PngBitDepth,
                WebpEncoderHints, GifEncoderHints,
                GetImageInfo001,
                TellDecoder001, DecoderCommand, JpegIDCTDownscaleHints, WebPDecoderHints,
                Execute001,
                GetVersionInfo,
                GetQueryStringSchema,
                ListQueryStringKeys,
                ValidateQueryString,
            )
        ),
        modifiers(&super::SchemaNamer),
        tags(
            (name = "Imageflow V1", description = "Imageflow JSON API operations (V1)")
        ),
        info(
            title = "Imageflow JSON API",
            version = "v1",
            // description = Some(include_str!("../../docs/src/json/api_description.md")), // TODO: Create this file
            contact(
                name = "Imazen",
                url = "https://imazen.io",
                email = "support@imazen.io"
            ),
            license(
                name = "Apache 2.0",
                url = "https://www.apache.org/licenses/LICENSE-2.0.html"
            )
        ),
        servers(
            (url = "/", description = "Relative path for FFI/tool interaction (simulated server)")
        )
    )]
    pub struct ApiDoc;
}

// Public function to generate the OpenAPI schema JSON
#[cfg(feature = "schema-export")]
pub fn get_openapi_schema_json() -> Result<String, serde_json::Error> {
    use v1::ApiDoc;
    ApiDoc::openapi().to_pretty_json()
}





fn get_create_doc_dir() -> std::path::PathBuf {
    let path = ::imageflow_types::version::crate_parent_folder().join(Path::new("target/doc"));
    let _ = std::fs::create_dir_all(&path);
    // Error { repr: Os { code: 17, message: "File exists" } }
    // The above can happen, despite the docs.
    path
}
#[test]
fn write_context_doc() {
    let path = get_create_doc_dir().join(Path::new("context_json_api.txt"));
    File::create(&path).unwrap().write_all(document_message().as_bytes()).unwrap();
}



fn document_message() -> String {
    let mut s = String::new();
    s.reserve(8000);
    s += "# JSON API - Context\n\n";
    s += "imageflow_context responds to these message methods\n\n";
    s += "## v1/build \n";
    s += "Example message body:\n";
    s += &serde_json::to_string_pretty(&Build001::example_with_steps()).unwrap();
    s += "\n\nExample response:\n";
    s += &serde_json::to_string_pretty(&Response001::example_job_result_encoded(2,
                                                                                   200,
                                                                                   200,
                                                                                   "image/png",
                                                                                   "png"))
        .unwrap();
    s += "## v1/get_image_info \n";
    s += "Example message body:\n";
    s += &serde_json::to_string_pretty(&GetImageInfo001::example_get_image_info()).unwrap();
    s += "\nExample response:\n";
    s += &serde_json::to_string_pretty(&Response001::example_image_info()).unwrap();
    s += "\n\n";


    s += "## v1/tell_decoder \n";
    s += "Example message body:\n";
    s += &serde_json::to_string_pretty(&TellDecoder001::example_hints()).unwrap();
    s += "\nExample response:\n";
    s += &serde_json::to_string_pretty(&Response001::example_ok()).unwrap();
    s += "\n\n";

    s += "## v1/execute \n";
    s += "Example message body (with graph):\n";
    s += &serde_json::to_string_pretty(&Execute001::example_graph()).unwrap();
    s += "Example message body (with linear steps):\n";
    s += &serde_json::to_string_pretty(&Execute001::example_steps()).unwrap();
    s += "\nExample response:\n";
    s += &serde_json::to_string_pretty(&Response001::example_job_result_encoded(2,
                                                                                   200,
                                                                                   200,
                                                                                   "image/jpg",
                                                                                   "jpg"))
        .unwrap();
    s += "\nExample failure response:\n";
    s += &serde_json::to_string_pretty(&Response001::example_error()).unwrap();
    s += "\n\n";

    s
}

// #[test]
fn test_handler() {

    let input_io = IoObject {
        io_id: 0,
        direction: IoDirection::In,

        io: IoEnum::BytesHex("FFD8FFE000104A46494600010101004800480000FFDB004300FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFC2000B080001000101011100FFC40014100100000000000000000000000000000000FFDA0008010100013F10".to_owned())
    };

    let output_io = IoObject {
        io_id: 1,
        direction: IoDirection::Out,

        io: IoEnum::OutputBuffer,
    };

    let mut steps = vec![];
    steps.push(Node::Decode {
        io_id: 0,
        commands: None,
    });
    steps.push(Node::Resample2D {
        w: 20,
        h: 30,
        hints: None,
    });
    steps.push(Node::FlipV);
    steps.push(Node::FlipH);
    steps.push(Node::Rotate90);
    steps.push(Node::Rotate180);
    steps.push(Node::Rotate270);
    steps.push(Node::Transpose);
    steps.push(Node::ExpandCanvas {
        top: 2,
        left: 3,
        bottom: 4,
        right: 5,
        color: Color::Srgb(ColorSrgb::Hex("aeae22".to_owned())),
    });
    steps.push(Node::FillRect {
        x1: 0,
        x2: 10,
        y1: 0,
        y2: 10,
        color: Color::Srgb(ColorSrgb::Hex("ffee00".to_owned())),
    });
    steps.push(Node::Encode {
        io_id: 1,
        preset: EncoderPreset::LibjpegTurbo { quality: Some(90), optimize_huffman_coding: None, progressive: None, matte: None },
    });

    let build = Build001 {
        builder_config: Some(Build001Config {
            graph_recording: None,
            security:None,
//            process_all_gif_frames: Some(false),
//            enable_jpeg_block_scaling: Some(false)
        }),
        io: vec![input_io, output_io],
        framewise: Framewise::Steps(steps),
    };
    // This test is outdated as build_1 is deprecated in favor of handle_build/build_1_raw
    // let response = Context::create().unwrap().build_1(build);
}

#[test]
fn test_get_version_info(){
    let response = Context::create().unwrap().get_version_info().unwrap();

    assert!(response.build_date.len() > 0);
    assert!(response.git_describe_always.len() > 0);
    assert!(response.last_git_commit.len() > 0);
    assert!(response.long_version_string.len() > 0);
}
