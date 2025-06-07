use crate::Context;
use crate::internal_prelude::works_everywhere::*;
use crate::json::*;
use crate::parsing::GraphTranslator;
use crate::parsing::IoTranslator;
use imageflow_types::*;
use twox_hash::xxhash3_128;
use std::error;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use serde::{Serialize, Deserialize};
use super::parse_json;
use std::fs::{self, ReadDir};
use std::io::{self, Read};
use std::path::PathBuf;
use std::collections::HashMap;

#[cfg(feature="json-schema")]
use schemars::{schema_for, JsonSchema};

#[cfg(test)]
extern crate include_dir;

#[cfg(feature = "schema-export")]
use utoipa::{OpenApi, ToSchema, Modify};

/// Define an endpoint response struct and its handler function with documentation
/// 
/// This macro reduces boilerplate for defining endpoints with standardized response types,
/// proper documentation, and error handling.
/// 
/// Usage:
/// ```
/// define_endpoint! {
///     name: BuildEndpoint,
///     path: "/v1/build",
///     request_type: Build001,
///     response_field: job_result,
///     response_type: JobResult,
///     handler_name: build,
///     description: "Build operation",
///     handler: |context, input| {
///         context.build_inner(input).map_err(|e| e.at(here!()))
///     }
/// }
/// ```
#[macro_export]
macro_rules! define_endpoint {
    (
        name: $name:ident,
        path: $path:expr,
        request_type: $req_type:ty,
        response_field: $resp_field:ident,
        response_type: $resp_type:ty,
        handler_name: $handler_name:ident,
        description: $desc:expr,
        handler: |$ctx:ident, $input:ident| $handler_body:expr
    ) => {
        // Define response struct
        #[cfg_attr(feature = "schema-export", derive(ToSchema))]
        #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct $name { pub $resp_field: $resp_type }

        // Define handler function with documentation
        #[cfg_attr(feature = "schema-export", utoipa::path(
            post,
            path = $path,
            request_body = $req_type,
            responses(
                (status = 200, description = concat!($desc, " successful"), body = JsonAnswer<$name>),
                (status = 500, description = concat!($desc, " failed"), body = JsonError)
            )
        ))]
        pub(super) fn $handler_name($ctx: &mut Context, $input: $req_type) -> Result<$name> {
            let result = $handler_body?;
            Ok($name { $resp_field: result })
        }
    };

    (
        name: $name:ident,
        path: $path:expr,
        request_type: $req_type:ty,
        handler_name: $handler_name:ident,
        empty_response: true,
        description: $desc:expr,
        handler: |$ctx:ident, $input:ident| $handler_body:expr
    ) => {
        // Define empty response struct
        #[cfg_attr(feature = "schema-export", derive(ToSchema))]
        #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct $name { }

        // Define handler function with documentation
        #[cfg_attr(feature = "schema-export", utoipa::path(
            post,
            path = $path,
            request_body = $req_type,
            responses(
                (status = 200, description = concat!($desc, " successful"), body = JsonAnswer<$name>),
                (status = 500, description = concat!($desc, " failed"), body = JsonError)
            )
        ))]
        pub(super) fn $handler_name($ctx: &mut Context, $input: $req_type) -> Result<$name> {
            $handler_body?;
            Ok($name {})
        }
    };

    (
        static_endpoint:
        name: $name:ident,
        path: $path:expr,
        request_type: $req_type:ty,
        response_field: $resp_field:ident,
        response_type: $resp_type:ty,
        handler_name: $handler_name:ident,
        description: $desc:expr,
        handler: || $handler_body:expr
    ) => {
        // Define response struct
        #[cfg_attr(feature = "schema-export", derive(ToSchema))]
        #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        pub struct $name { pub $resp_field: $resp_type }

        // Define static handler function
        #[cfg_attr(feature = "schema-export", utoipa::path(
            post,
            path = $path,
            request_body = $req_type,
            responses(
                (status = 200, description = concat!($desc, " successful"), body = JsonAnswer<$name>),
                (status = 500, description = concat!($desc, " failed"), body = JsonError)
            )
        ))]
        pub(super) fn $handler_name() -> Result<$name> {
            let result = $handler_body?;
            Ok($name { $resp_field: result })
        }
    };
}

/// Define an endpoint registry macro to help register and route endpoints
/// 
/// This macro takes a list of endpoint definitions and generates a lookup function
/// that matches endpoint paths to handler functions
#[macro_export]
macro_rules! endpoint_registry {
    (
        context_endpoints => [
            $(
                ( $path:expr, $path_alt:expr, $request_type:ty, $handler:ident ),
            )*
        ],
        static_endpoints => [
            $(
                ( $static_path:expr, $static_request_type:ty, $static_handler:ident ),
            )*
        ],
        conditional_static_endpoints => [
            $(
                $cfg:meta => ( $cfg_path:expr, $cfg_request_type:ty, $cfg_handler:ident ),
            )*
        ]
    ) => {
        /// Invoke an endpoint that requires a context
        pub fn invoke(context: &mut Context, method: &str, json: &[u8]) -> Result<JsonResponse> {
            if let Some(response) = try_invoke_static(method, json)? {
                return Ok(response);
            }
            
            match method {
                $(
                    $path | $path_alt => {
                        let input = parse_json::<$request_type>(json)?;
                        let output = $handler(context, input)?;
                        Ok(JsonResponse::ok(output))
                    },
                )*
                _ => Err(nerror!(ErrorKind::InvalidMessageEndpoint))
            }
        }

        /// Try to invoke a static endpoint (one that doesn't require a context)
        pub fn try_invoke_static(method: &str, json: &[u8]) -> Result<Option<JsonResponse>> {
            match method {
                $(
                    $static_path => {
                        let input = parse_json::<$static_request_type>(json)?;
                        let output = $static_handler()?;
                        Ok(Some(JsonResponse::ok(output)))
                    },
                )*
                $(
                    #[cfg($cfg)]
                    $cfg_path => {
                        let input = parse_json::<$cfg_request_type>(json)?;
                        let output = $cfg_handler()?;
                        Ok(Some(JsonResponse::ok(output)))
                    },
                )*
                "v1/schema/openapi/latest/get" => {
                    let output = get_openapi_schema_json()?;
                    Ok(Some(JsonResponse::ok(output)))
                },
                "v1/schema/riapi/latest/validate" => {
                    let input = parse_json::<s::ValidateQueryString>(json)?;
                    let output = validate_riapi_query_string(input)?;
                    Ok(Some(JsonResponse::ok(output)))
                },
                "v1/brew_coffee" => Ok(Some(JsonResponse::teapot())),
                _ => Ok(None)
            }
        }
    };
}

// Generate the endpoint registry
endpoint_registry! {
    context_endpoints => [
        ("v1/build", "v0.1/build", s::Build001, build),
        ("v1/get_image_info", "v0.1/get_image_info", s::GetImageInfo001, get_image_info),
        ("v1/get_scaled_image_info", "", s::GetImageInfo001, get_scaled_image_info),
        ("v1/tell_decoder", "v0.1/tell_decoder", s::TellDecoder001, tell_decoder),
        ("v1/execute", "v0.1/execute", s::Execute001, execute),
    ],
    static_endpoints => [
        ("v1/get_version_info", s::EmptyRequest, get_version_info),
        ("v1/schema/riapi/latest/get", s::EmptyRequest, get_riapi_schema),
        ("v1/schema/riapi/latest/list_keys", s::EmptyRequest, list_riapi_keys),
        ("v1/schema/list-schema-endpoints", s::EmptyRequest, list_schema_endpoints),
    ],
    conditional_static_endpoints => [
        feature = "json-schema" => ("v1/schema/json/latest/v1/all", s::EmptyRequest, get_json_schemas_v1),
    ]
}

// Define a struct to implement the Modify trait for customizing schema names
#[cfg(feature = "schema-export")]
struct SchemaNamer;

#[cfg(feature = "schema-export")]
impl Modify for SchemaNamer {
    fn modify(&self, schema: &mut utoipa::openapi::OpenApi) {
        // Attempt to remove the module path prefix like `imageflow_type` or `json_message`
        let mut title = schema.info.title.to_owned();
        // truncate the start of the string,
        if let Some(last_part) = title.rsplit("::").next() {
            title = last_part.to_string();
        }
        schema.info.title = title;
    }
}

// Generic wrapper for successful JSON responses (matches Response001 structure)
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
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
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
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

// Define endpoints using our macro
define_endpoint! {
    name: BuildV1Response,
    path: "/v1/build",
    request_type: Build001,
    response_field: job_result,
    response_type: JobResult,
    handler_name: build,
    description: "Build operation",
    handler: |context, parsed| {
        context.build_inner(parsed).map_err(|e| e.at(here!()))
    }
}

define_endpoint! {
    name: GetImageInfoV1Response,
    path: "/v1/get_image_info",
    request_type: GetImageInfo001,
    response_field: image_info,
    response_type: ImageInfo,
    handler_name: get_image_info,
    description: "Image info retrieval",
    handler: |context, data| {
        context.get_unscaled_rotated_image_info(data.io_id).map_err(|e| e.at(here!()))
    }
}

define_endpoint! {
    name: GetScaledImageInfoV1Response,
    path: "/v1/get_scaled_image_info",
    request_type: GetImageInfo001,
    response_field: image_info,
    response_type: ImageInfo,
    handler_name: get_scaled_image_info,
    description: "Scaled image info retrieval",
    handler: |context, data| {
        context.get_scaled_rotated_image_info(data.io_id).map_err(|e| e.at(here!()))
    }
}

define_endpoint! {
    name: TellDecoderV1Response,
    path: "/v1/tell_decoder",
    request_type: TellDecoder001,
    handler_name: tell_decoder,
    empty_response: true,
    description: "Decoder hints application",
    handler: |context, data| {
        context.tell_decoder(data.io_id, data.command).map_err(|e| e.at(here!()))
    }
}

define_endpoint! {
    name: ExecuteV1Response,
    path: "/v1/execute",
    request_type: Execute001,
    response_field: job_result,
    response_type: JobResult,
    handler_name: execute,
    description: "Execution",
    handler: |context, parsed| {
        context.execute_inner(parsed).map_err(|e| e.at(here!()))
    }
}

define_endpoint! {
    static_endpoint:
    name: GetVersionInfoV1Response,
    path: "/v1/get_version_info",
    request_type: EmptyRequest,
    response_field: version_info,
    response_type: VersionInfo,
    handler_name: get_version_info,
    description: "Version info retrieval",
    handler: || {
        Context::get_version_info_static().map_err(|e| e.at(here!()))
    }
}

define_endpoint! {
    static_endpoint:
    name: GetRiapiSchemaV1Response,
    path: "/v1/schema/riapi/latest/get",
    request_type: EmptyRequest,
    response_field: schema,
    response_type: json_messages::QueryStringSchema,
    handler_name: get_riapi_schema,
    description: "RIAPI schema retrieval",
    handler: || {
        imageflow_riapi::ir4::get_query_string_schema().map_err(|e| nerror!(ErrorKind::InternalError, "{}", e))
    }
}

define_endpoint! {
    static_endpoint:
    name: ListRiapiKeysV1Response,
    path: "/v1/schema/riapi/latest/list_keys",
    request_type: EmptyRequest,
    response_field: schema,
    response_type: json_messages::QueryStringSchema,
    handler_name: list_riapi_keys,
    description: "Supported RIAPI keys listing",
    handler: || {
        imageflow_riapi::ir4::get_query_string_keys().map_err(|e| nerror!(ErrorKind::InternalError, "{}", e))
    }
}

// The validate_riapi_query_string function needs the utoipa::path attribute for OpenAPI docs
#[cfg_attr(feature = "schema-export", utoipa::path(
    post,
    path = "/v1/schema/riapi/latest/validate",
    request_body = ValidateQueryString,
    responses(
        (status = 200, description = "RIAPI query string validation results", body = JsonAnswer<ValidateRiapiQueryStringV1Response>),
        (status = 500, description = "Validation failed", body = JsonError)
    )
))]
pub(super) fn validate_riapi_query_string(data: ValidateQueryString) -> Result<ValidateRiapiQueryStringV1Response> {
    let results = imageflow_riapi::ir4::validate_query_string(data.query_string)
        .map_err(|e| nerror!(ErrorKind::InternalError, "{}", e))?;
    Ok(ValidateRiapiQueryStringV1Response { results })
}

#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidateRiapiQueryStringV1Response { pub results: json_messages::QueryStringValidationResults }

#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListSchemaEndpointsResponse { pub endpoints: Vec<String> }

#[cfg(feature="json-schema")]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndpointSchemaPair {
    #[cfg_attr(feature = "schema-export", schema(value_type = Object))]
    pub input_schema: schemars::Schema,
    #[cfg_attr(feature = "schema-export", schema(value_type = Object))]
    pub output_schema: schemars::Schema,
}

#[cfg(feature="json-schema")]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AllJsonSchemasV1 {
    #[serde(rename = "v1/build")] pub build: EndpointSchemaPair,
    #[serde(rename = "v1/execute")] pub execute: EndpointSchemaPair,
    #[serde(rename = "v1/tell_decoder")] pub tell_decoder: EndpointSchemaPair,
    #[serde(rename = "v1/get_image_info")] pub get_image_info: EndpointSchemaPair,
    #[serde(rename = "v1/get_scaled_image_info")] pub get_scaled_image_info: EndpointSchemaPair,
}

#[cfg(feature="json-schema")]
#[cfg_attr(feature = "schema-export", derive(ToSchema))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GetJsonSchemasV1Response { pub schemas: AllJsonSchemasV1 }

// --- Main OpenAPI Documentation Struct ---
#[cfg(feature = "schema-export")]
#[derive(::utoipa::OpenApi)]
#[openapi(
    paths(
        build,
        get_image_info,
        get_scaled_image_info,
        tell_decoder,
        execute,
        get_version_info,
        get_riapi_schema,
        list_riapi_keys,
        validate_riapi_query_string,
        get_openapi_schema_json,
        list_schema_endpoints,
        #[cfg(feature = "json-schema")]
        get_json_schemas_v1,
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
            JsonAnswer<GetRiapiSchemaV1Response>, GetRiapiSchemaV1Response,
            JsonAnswer<ListRiapiKeysV1Response>, ListRiapiKeysV1Response,
            JsonAnswer<ValidateRiapiQueryStringV1Response>, ValidateRiapiQueryStringV1Response,
            JsonAnswer<ListSchemaEndpointsResponse>, ListSchemaEndpointsResponse,
            JsonAnswer<GetJsonSchemasV1Response>, GetJsonSchemasV1Response, 
            EndpointSchemaPair, AllJsonSchemasV1,
            JsonError,

            // Core Request/Response types from imageflow_types (referenced by handlers/structs above)
            Response001, // Still useful to include the base structure
            ResponsePayload,
            ImageInfo,
            JobResult, EncodeResult, DecodeResult, ResultBytes, BuildPerformance, FramePerformance, NodePerf,
            VersionInfo,
            json_messages::QueryStringSchema, json_messages::QueryStringSchemaKey, json_messages::QueryStringDescription,
            json_messages::QueryStringSchemaValue, json_messages::QueryStringSchemaValueValidation, json_messages::QueryStringSchemaValueRange,
            json_messages::QueryStringSchemaExample, json_messages::QueryStringSchemaKeyGroup, json_messages::QueryStringSchemaMarkdownPage,
            json_messages::QueryStringValidationResults, json_messages::QueryStringValidationIssue, json_messages::QueryStringValidationIssueKind,
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
            ValidateQueryString,
            EmptyRequest,
        )
    ),
    modifiers(&SchemaNamer),
    tags(
        (name = "Imageflow V1", description = "Imageflow JSON API operations (V1)")
    ),
    info(
        title = "libimageflow JSON API",
        version = "v1",
        // description = Some(include_str!("../../docs/src/json/api_description.md")), // TODO: Create this file
        contact(
            name = "Imazen",
            url = "https://imazen.io",
            email = "support@imazen.io"
        ),
        license(
            name = "AGPL 3.0 / Commercial",
            url = "https://imazen.io/pricing"
        )
    ),
    servers(
        (url = "/", description = "Relative path for FFI/tool interaction (simulated server)")
    )
)]
pub struct ApiDoc;


// static bool if schema-export is enabled
#[cfg(feature = "schema-export")]
pub static OPENAPI_SCHEMA_ENABLED: bool = true;

#[cfg(not(feature = "schema-export"))]
pub static OPENAPI_SCHEMA_ENABLED: bool = false;

// static bool if schema-export is enabled
use std::sync::Mutex;
static OPENAPI_SCHEMA_JSON: Mutex<String> = Mutex::new(String::new());

#[cfg(feature = "schema-export")]
pub fn get_openapi_schema_json_inner() -> Result<String> {
    generate_openapi_schema_json_cached()
}
#[cfg(not(feature = "schema-export"))]
pub fn get_openapi_schema_json_inner() -> Result<String> {
    load_embedded_openapi_schema_json()
}

#[cfg(feature = "schema-export")]
pub fn generate_openapi_schema_json_cached() -> Result<String> {
    let mut schema = OPENAPI_SCHEMA_JSON.lock().unwrap();
    if schema.is_empty() {
        *schema = generate_openapi_schema_json()?;
    }
    Ok(schema.clone())
}
#[cfg(feature = "schema-export")]
pub fn generate_openapi_schema_json() -> Result<String> {
    //Use an atomic or something to cache the result
    use ApiDoc;
    ApiDoc::openapi().to_pretty_json().map_err( |e| nerror!(ErrorKind::InternalError, "{}", e))    
}


const OPENAPI_SCHEMA_V1_JSON_NAME: &str = "openapi_schema_v1.json";
const OPENAPI_SCHEMA_V1_JSON: &str = include_str!("openapi_schema_v1.json");
const OPENAPI_SCHEMA_V1_JSON_HASH: &str = include_str!("openapi_schema_v1.json.hash");
const OPENAPI_SCHEMA_V1_JSON_HASH_NAME: &str = "openapi_schema_v1.json.hash";

#[cfg(not(feature = "schema-export"))]
pub fn load_embedded_openapi_schema_json() -> Result<String> {
    Ok(OPENAPI_SCHEMA_V1_JSON.to_string())
}


#[cfg(test)]
static SCHEMA_SET_1: include_dir::Dir = include_dir::include_dir!("$CARGO_MANIFEST_DIR/src/json");
#[cfg(test)]
static SCHEMA_SET_2: include_dir::Dir = include_dir::include_dir!("$CARGO_MANIFEST_DIR/../imageflow_types/src");

#[cfg(test)]
fn hash_files_relevant_to_schema() -> String {
    let mut hasher = xxhash3_128::Hasher::new();
    // Also include this file in the hash
    hasher.write(include_str!("v1.rs").as_bytes()); 
    for file in SCHEMA_SET_1.find("**/*.rs").unwrap() {
        hasher.write(file.as_file().unwrap().contents());
    }
    for file in SCHEMA_SET_2.find("**/*.rs").unwrap() {
        hasher.write(file.as_file().unwrap().contents());
    }
    format!("{:x}", hasher.finish_128())
}

#[test]
fn hash_files_relevant_to_schema_and_compare() -> io::Result<()> {
    let embedded_hash = OPENAPI_SCHEMA_V1_JSON_HASH;
    let current_hash = hash_files_relevant_to_schema();
    let hash_name = OPENAPI_SCHEMA_V1_JSON_HASH_NAME;
    let schema_name = OPENAPI_SCHEMA_V1_JSON_NAME;

    if current_hash.as_str() != embedded_hash {
        if cfg!(feature = "schema-export") {
            eprintln!("Schema-relevant files changed. Regenerating OpenAPI schema and hash.");

            let new_schema_json = generate_openapi_schema_json()
                .expect("Failed to generate OpenAPI schema JSON");

                
            let cargo_manifest_dir = env!("CARGO_MANIFEST_DIR");
            let current_dir = Path::new(cargo_manifest_dir).join("src/json/endpoints");
    
            let hash_file_path = current_dir.join(OPENAPI_SCHEMA_V1_JSON_HASH_NAME);
            let schema_file_path = current_dir.join(OPENAPI_SCHEMA_V1_JSON_NAME);
            // if current_dir doesn't exist, fail with a message
            if !current_dir.exists() {
                panic!("Current directory does not exist: {}", current_dir.display());
            }

            
            fs::write(&schema_file_path, &new_schema_json)?;
            eprintln!("Wrote updated schema to: {}", schema_file_path.display());

            fs::write(&hash_file_path, &current_hash)?;
            eprintln!("Wrote updated hash to: {}", hash_file_path.display());

            eprintln!(
                "OpenAPI schema ({}) and hash ({}) \
                 were updated because relevant source files changed. Please review and commit \
                 the changes.",
                 schema_name, hash_name
            );
        } else {
            panic!(
                "OpenAPI schema definition is outdated. Please run `cargo test --features schema-export`
                 to regenerate the schema and hash file, then commit the changes to both '{}' and '{}'.",
                 hash_name, schema_name
            );
        }
    } else {
        println!("OpenAPI schema hash matches.");
    }

    Ok(())
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

#[cfg_attr(feature = "schema-export", utoipa::path(
    get,
    path = "/v1/schema/openapi/latest/get",
    responses(
        (status = 200, description = "OpenAPI schema retrieved", body = String)
    )
))]
pub fn get_openapi_schema_json() -> Result<String> {
    get_openapi_schema_json_inner()
}

#[cfg_attr(feature = "schema-export", utoipa::path(
    post,
    path = "/v1/schema/list-schema-endpoints",
    request_body = EmptyRequest,
    responses(
        (status = 200, description = "List of available schema endpoints", body = JsonAnswer<ListSchemaEndpointsResponse>),
        (status = 500, description = "Failed to list schema endpoints", body = JsonError)
    )
))]
pub(super) fn list_schema_endpoints() -> Result<ListSchemaEndpointsResponse> {
    let mut endpoints = vec![
        "/v1/schema/riapi/latest/get".to_string(),
        "/v1/schema/riapi/latest/list_keys".to_string(),
        "/v1/schema/riapi/latest/validate".to_string(),
        "/v1/schema/openapi/latest/get".to_string(),
        "/v1/schema/list-schema-endpoints".to_string(),
    ];
    if cfg!(feature = "json-schema") {
        endpoints.push("/v1/schema/json/latest/v1/all".to_string());
    }
    endpoints.sort();
    Ok(ListSchemaEndpointsResponse { endpoints })
}

#[cfg(feature="json-schema")]
#[cfg_attr(feature = "schema-export", utoipa::path(
    post,
    path = "/v1/schema/json/latest/v1/all",
    request_body = EmptyRequest,
    responses(
        (status = 200, description = "Combined JSON schemas for V1 endpoints", body = JsonAnswer<GetJsonSchemasV1Response>),
        (status = 500, description = "Failed to generate JSON schemas", body = JsonError)
    )
))]
pub(super) fn get_json_schemas_v1() -> Result<GetJsonSchemasV1Response> {
    let schemas = AllJsonSchemasV1 {
        build: EndpointSchemaPair {
            input_schema: schema_for!(s::Build001),
            output_schema: schema_for!(BuildV1Response),
        },
        execute: EndpointSchemaPair {
            input_schema: schema_for!(s::Execute001),
            output_schema: schema_for!(ExecuteV1Response),
        },
        tell_decoder: EndpointSchemaPair {
            input_schema: schema_for!(s::TellDecoder001),
            output_schema: schema_for!(TellDecoderV1Response),
        },
        get_image_info: EndpointSchemaPair {
            input_schema: schema_for!(s::GetImageInfo001),
            output_schema: schema_for!(GetImageInfoV1Response),
        },
        get_scaled_image_info: EndpointSchemaPair {
            input_schema: schema_for!(s::GetImageInfo001),
            output_schema: schema_for!(GetScaledImageInfoV1Response),
        },
    };
    Ok(GetJsonSchemasV1Response { schemas })
}
