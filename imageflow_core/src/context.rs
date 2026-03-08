//! Context — the main entry point for imageflow operations.
//!
//! A Context manages I/O buffers and processes JSON API requests.
//! Thread-safe: multiple threads can call `send_json` concurrently.

use crate::error::FlowError;
use crate::io::IoStore;
use crate::pipeline;
use imageflow_types::{BuildRequest, ExecuteRequest, Response, ResponseData, VersionInfo};
use std::sync::{Arc, RwLock};

/// The main imageflow context. Owns I/O buffers and processes requests.
pub struct Context {
    inner: RwLock<ContextInner>,
}

struct ContextInner {
    io: IoStore,
    #[allow(dead_code)]
    error: Option<FlowError>,
}

/// JSON response returned from `send_json`.
pub struct JsonResponse {
    pub status_code: i32,
    pub response_json: Vec<u8>,
}

impl Context {
    /// Create a new context.
    pub fn new() -> Self {
        Context { inner: RwLock::new(ContextInner { io: IoStore::new(), error: None }) }
    }

    /// Add an input buffer.
    pub fn add_input_buffer(&self, io_id: i32, data: &[u8]) -> Result<(), FlowError> {
        let mut inner =
            self.inner.write().map_err(|_| FlowError::Internal("lock poisoned".into()))?;
        inner.io.add_input(io_id, Arc::from(data));
        Ok(())
    }

    /// Add an output buffer slot.
    pub fn add_output_buffer(&self, io_id: i32) -> Result<(), FlowError> {
        let mut inner =
            self.inner.write().map_err(|_| FlowError::Internal("lock poisoned".into()))?;
        inner.io.add_output(io_id);
        Ok(())
    }

    /// Get the output buffer for a given io_id.
    pub fn get_output_buffer(&self, io_id: i32) -> Result<Vec<u8>, FlowError> {
        let inner = self.inner.read().map_err(|_| FlowError::Internal("lock poisoned".into()))?;
        Ok(inner.io.get_output(io_id)?.to_vec())
    }

    /// Send a JSON message and get a JSON response.
    pub fn send_json(&self, method: &str, json: &[u8]) -> JsonResponse {
        let result = self.handle_method(method, json);
        match result {
            Ok(response) => {
                let json_bytes = serde_json::to_vec(&response).unwrap_or_else(|e| {
                    format!(
                        r#"{{"code":500,"success":false,"message":"serialization error: {e}"}}"#
                    )
                    .into_bytes()
                });
                JsonResponse { status_code: response.code as i32, response_json: json_bytes }
            }
            Err(e) => {
                let status = e.http_status();
                let response = Response {
                    code: status,
                    success: false,
                    data: None,
                    message: Some(e.to_string()),
                };
                let json_bytes = serde_json::to_vec(&response).unwrap_or_else(|_| {
                    format!(r#"{{"code":{status},"success":false,"message":"error"}}"#).into_bytes()
                });
                JsonResponse { status_code: status as i32, response_json: json_bytes }
            }
        }
    }

    fn handle_method(&self, method: &str, json: &[u8]) -> Result<Response, FlowError> {
        match method {
            "v2/build" | "v1/build" => self.handle_build(json),
            "v2/execute" | "v1/execute" => self.handle_execute(json),
            "v2/get_image_info" | "v1/get_image_info" => self.handle_get_image_info(json),
            "v2/get_version_info" | "v1/get_version_info" => Ok(Response {
                code: 200,
                success: true,
                data: Some(ResponseData::VersionInfo(VersionInfo {
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    git_commit: None,
                    codecs: vec![
                        "jpeg".into(),
                        "png".into(),
                        "webp".into(),
                        "gif".into(),
                        "avif".into(),
                        "jxl".into(),
                    ],
                })),
                message: None,
            }),
            _ => Err(FlowError::InvalidPipeline(format!("unknown method: {method}"))),
        }
    }

    fn handle_build(&self, json: &[u8]) -> Result<Response, FlowError> {
        let request: BuildRequest = serde_json::from_slice(json)?;

        // Set up I/O from the request
        {
            let mut inner =
                self.inner.write().map_err(|_| FlowError::Internal("lock poisoned".into()))?;
            for io_obj in &request.io {
                match io_obj.direction {
                    imageflow_types::IoDirection::In => {
                        let data = resolve_io_data(&io_obj.io)?;
                        inner.io.add_input(io_obj.io_id, Arc::from(data));
                    }
                    imageflow_types::IoDirection::Out => {
                        inner.io.add_output(io_obj.io_id);
                    }
                }
            }
        }

        // Execute the pipeline
        let result = {
            let mut inner =
                self.inner.write().map_err(|_| FlowError::Internal("lock poisoned".into()))?;
            pipeline::execute(&mut inner.io, &request.pipeline, &request.security)?
        };

        Ok(Response {
            code: 200,
            success: true,
            data: Some(ResponseData::BuildResult(result)),
            message: None,
        })
    }

    fn handle_execute(&self, json: &[u8]) -> Result<Response, FlowError> {
        let request: ExecuteRequest = serde_json::from_slice(json)?;

        let result = {
            let mut inner =
                self.inner.write().map_err(|_| FlowError::Internal("lock poisoned".into()))?;
            pipeline::execute(&mut inner.io, &request.pipeline, &request.security)?
        };

        Ok(Response {
            code: 200,
            success: true,
            data: Some(ResponseData::BuildResult(result)),
            message: None,
        })
    }

    fn handle_get_image_info(&self, json: &[u8]) -> Result<Response, FlowError> {
        #[derive(serde::Deserialize)]
        struct InfoRequest {
            io_id: i32,
        }
        let req: InfoRequest = serde_json::from_slice(json)?;

        let inner = self.inner.read().map_err(|_| FlowError::Internal("lock poisoned".into()))?;
        let data = inner.io.get_input(req.io_id)?;

        let info = pipeline::probe_image(data)?;

        Ok(Response {
            code: 200,
            success: true,
            data: Some(ResponseData::ImageInfo(info)),
            message: None,
        })
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve IoEnum to bytes.
fn resolve_io_data(io: &imageflow_types::IoEnum) -> Result<Vec<u8>, FlowError> {
    match io {
        imageflow_types::IoEnum::ByteArray(data) => Ok(data.clone()),
        imageflow_types::IoEnum::Base64(b64) => {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| FlowError::InvalidPipeline(format!("invalid base64: {e}")))
        }
        imageflow_types::IoEnum::BytesHex(hex) => {
            let bytes: Result<Vec<u8>, _> =
                (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16)).collect();
            bytes.map_err(|e| FlowError::InvalidPipeline(format!("invalid hex: {e}")))
        }
        imageflow_types::IoEnum::Filename(path) => {
            std::fs::read(path).map_err(|e| FlowError::Codec(format!("failed to read {path}: {e}")))
        }
        imageflow_types::IoEnum::Placeholder => {
            Err(FlowError::InvalidPipeline("placeholder I/O not replaced before execution".into()))
        }
        imageflow_types::IoEnum::OutputBuffer | imageflow_types::IoEnum::OutputBase64 => {
            Err(FlowError::InvalidPipeline("cannot read from output I/O".into()))
        }
    }
}
