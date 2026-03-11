//! V2 API endpoint handlers.
//!
//! Routes `v2/build` and `v2/execute` through the `imageflow-graph` engine.
//! Uses `imageflow-commands` types (Step, Pipeline, BuildRequest, ExecuteRequest).
//!
//! ## Execution modes
//!
//! - **v1 as-is**: petgraph engine via `imageflow_types::Node`/`Framewise` (unchanged)
//! - **v2**: `imageflow-commands` types → `imageflow-graph` engine (this module)
//! - **transitional**: translates v1 types to v2, routes through imageflow-graph
//!
//! The transitional mode will be implemented as a separate endpoint that
//! converts `Execute001`/`Build001` → `ExecuteRequest`/`BuildRequest`.

use super::parse_json;
use crate::json::*;
use crate::Context;
use imageflow_commands as cmd;
use imageflow_graph as graph;
use serde::{Deserialize, Serialize};

// ─── Response types ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildV2Response {
    pub outputs: Vec<EncodeResultV2>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteV2Response {
    pub outputs: Vec<EncodeResultV2>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodeResultV2 {
    pub io_id: i32,
    pub format: String,
    pub w: u32,
    pub h: u32,
    pub bytes: u64,
}

impl From<graph::EncodeOutput> for EncodeResultV2 {
    fn from(o: graph::EncodeOutput) -> Self {
        Self { io_id: o.io_id, format: o.format, w: o.w, h: o.h, bytes: o.bytes }
    }
}

// ─── Routing ───────────────────────────────────────────────────────────

pub fn invoke(context: &mut Context, method: &str, json: &[u8]) -> Result<JsonResponse> {
    match method {
        "v2/build" => {
            let input = parse_json::<cmd::BuildRequest>(json)?;
            let output = build(context, input)?;
            Ok(JsonResponse::ok(output))
        }
        "v2/execute" => {
            let input = parse_json::<cmd::ExecuteRequest>(json)?;
            let output = execute(context, input)?;
            Ok(JsonResponse::ok(output))
        }
        _ => Err(nerror!(ErrorKind::InvalidMessageEndpoint)),
    }
}

// ─── Handlers ──────────────────────────────────────────────────────────

/// v2/build — Full pipeline: register I/O + build graph + execute.
fn build(context: &mut Context, request: cmd::BuildRequest) -> Result<BuildV2Response> {
    // 1. Register I/O sources
    register_io_v2(context, &request.io)?;

    // 2. Apply security limits
    if let Some(ref security) = request.security {
        apply_security_v2(context, security);
    }

    // 3. Build, compile, and execute the graph
    let result = execute_pipeline(context, &request.pipeline)?;

    Ok(BuildV2Response { outputs: result.outputs.into_iter().map(EncodeResultV2::from).collect() })
}

/// v2/execute — Execute pipeline (I/O already configured on context).
fn execute(context: &mut Context, request: cmd::ExecuteRequest) -> Result<ExecuteV2Response> {
    // Apply security limits
    if let Some(ref security) = request.security {
        apply_security_v2(context, security);
    }

    // Build, compile, and execute the graph
    let result = execute_pipeline(context, &request.pipeline)?;

    Ok(ExecuteV2Response {
        outputs: result.outputs.into_iter().map(EncodeResultV2::from).collect(),
    })
}

// ─── Graph pipeline ────────────────────────────────────────────────────

/// Build → compile → execute a pipeline through imageflow-graph.
fn execute_pipeline(
    context: &mut Context,
    pipeline: &cmd::Pipeline,
) -> Result<graph::ExecutionResult> {
    // Build the graph from the pipeline definition
    let builder = graph::GraphBuilder::from_pipeline(pipeline)
        .map_err(|e| nerror!(ErrorKind::InvalidOperation, "Failed to build graph: {}", e))?;

    // Compile (validate → estimate → expand → optimize)
    let compiled = graph::CompiledGraph::compile(builder)
        .map_err(|e| nerror!(ErrorKind::InvalidOperation, "Failed to compile graph: {}", e))?;

    // Execute with the v2 adapter bridging to existing codec infrastructure
    let mut engine = graph::GraphEngine::new(compiled);
    let mut adapter = V2ExecutionAdapter::new(context);

    engine
        .execute(&mut adapter)
        .map_err(|e| nerror!(ErrorKind::InternalError, "Graph execution failed: {}", e))
}

// ─── I/O registration ──────────────────────────────────────────────────

/// Register v2 IoObjects on the context.
///
/// Converts v2 `imageflow-commands` I/O types to v1 `imageflow_types` I/O types
/// and registers them via the existing IoTranslator infrastructure.
fn register_io_v2(context: &mut Context, io_objects: &[cmd::IoObject]) -> Result<()> {
    use imageflow_types as s;

    let v1_ios: Vec<s::IoObject> = io_objects
        .iter()
        .map(|io_obj| {
            let direction = match io_obj.direction {
                cmd::IoDirection::In => s::IoDirection::In,
                cmd::IoDirection::Out => s::IoDirection::Out,
            };

            let io_enum = match &io_obj.io {
                cmd::IoEnum::BytesHex(hex) => s::IoEnum::BytesHex(hex.clone()),
                cmd::IoEnum::Base64(b64) => s::IoEnum::Base64(b64.clone()),
                cmd::IoEnum::ByteArray(bytes) => s::IoEnum::ByteArray(bytes.clone()),
                cmd::IoEnum::Filename(path) => s::IoEnum::Filename(path.clone()),
                cmd::IoEnum::OutputBuffer => s::IoEnum::OutputBuffer,
                cmd::IoEnum::OutputBase64 => s::IoEnum::OutputBase64,
                cmd::IoEnum::Placeholder => s::IoEnum::Placeholder,
            };

            s::IoObject { io_id: io_obj.io_id, direction, io: io_enum }
        })
        .collect();

    crate::parsing::IoTranslator {}.add_all(context, v1_ios)
}

// ─── Security ──────────────────────────────────────────────────────────

/// Apply v2 SecurityLimits to context.
///
/// Maps v2 security types to existing context security configuration.
/// Currently a placeholder — security limits will be wired as the v2
/// engine matures and mirrors the v1 configure_security() method.
fn apply_security_v2(_context: &mut Context, _security: &cmd::SecurityLimits) {
    // TODO: Wire timeout, max sizes, codec config through to Context.
    // The v1 path uses context.configure_security(ExecutionSecurity),
    // which sets cancellation_token timeout, enabled_codecs, and
    // frame size limits. The v2 equivalent will be added when the
    // adapter handles actual execution.
}

// ─── Execution adapter ─────────────────────────────────────────────────

/// Bridges `imageflow-graph`'s `ExecutionContext` trait to the existing
/// imageflow_core codec and I/O infrastructure.
///
/// This is the central integration seam between the new graph engine and
/// the existing processing pipeline. It delegates decode/encode to the
/// Context's codec layer and processing operations to the existing
/// bitmap operations.
///
/// ## Current status
///
/// This is a skeleton adapter. Individual step handlers will be wired up
/// incrementally as the v2 engine matures:
/// 1. Decode/Encode — bridge to Context's codec infrastructure
/// 2. Geometry ops — bridge to existing resize/crop/rotate
/// 3. Color ops — bridge to existing color filter pipeline
/// 4. Composition — bridge to existing watermark/draw_image
struct V2ExecutionAdapter<'a> {
    context: &'a mut Context,
    next_handle: u64,
}

impl<'a> V2ExecutionAdapter<'a> {
    fn new(context: &'a mut Context) -> Self {
        Self { context, next_handle: 1 }
    }

    #[allow(dead_code)]
    fn alloc_handle(&mut self) -> graph::FrameHandle {
        let h = graph::FrameHandle(self.next_handle);
        self.next_handle += 1;
        h
    }
}

impl graph::ExecutionContext for V2ExecutionAdapter<'_> {
    fn execute_step(
        &mut self,
        node: graph::NodeIndex,
        step: &imageflow_commands::Step,
        _inputs: &[graph::FrameHandle],
        _canvas: Option<graph::FrameHandle>,
    ) -> graph::Result<graph::StepOutput> {
        // TODO: Wire up individual step handlers as the v2 engine matures.
        // Each match arm will delegate to existing Context methods.
        let step_name = match step {
            imageflow_commands::Step::Decode(_) => "decode",
            imageflow_commands::Step::Encode(_) => "encode",
            imageflow_commands::Step::Constrain(_) => "constrain",
            imageflow_commands::Step::Resize(_) => "resize",
            imageflow_commands::Step::Crop(_) => "crop",
            imageflow_commands::Step::FlipH => "flip_h",
            imageflow_commands::Step::FlipV => "flip_v",
            imageflow_commands::Step::Rotate90 => "rotate_90",
            imageflow_commands::Step::Rotate180 => "rotate_180",
            imageflow_commands::Step::Rotate270 => "rotate_270",
            imageflow_commands::Step::Transpose => "transpose",
            _ => "unknown",
        };

        Err(graph::GraphError::UnsupportedOperation {
            node,
            operation: format!(
                "v2 step '{}' not yet implemented — \
                 the v2 execution adapter is a skeleton",
                step_name,
            ),
        })
    }

    fn execute_internal(
        &mut self,
        node: graph::NodeIndex,
        params: &graph::InternalParams,
        _inputs: &[graph::FrameHandle],
    ) -> graph::Result<graph::StepOutput> {
        let op_name = match params {
            graph::InternalParams::Noop => "noop",
            graph::InternalParams::Resize { .. } => "internal_resize",
            graph::InternalParams::Crop { .. } => "internal_crop",
            graph::InternalParams::Pad { .. } => "internal_pad",
        };

        Err(graph::GraphError::UnsupportedOperation {
            node,
            operation: format!(
                "v2 internal op '{}' not yet implemented — \
                 the v2 execution adapter is a skeleton",
                op_name,
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal JPEG: 1x1 pixel, valid enough to register as input.
    const TINY_JPEG_HEX: &str = "FFD8FFE000104A46494600010101004800480000\
        FFDB004300FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF\
        FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF\
        FFC2000B080001000101011100FFC40014100100000000000000000000000000000000\
        FFDA0008010100013F10";

    fn build_request_json(steps_json: &str) -> String {
        format!(
            r#"{{
                "io": [
                    {{ "io_id": 0, "direction": "in", "bytes_hex": "{}" }},
                    {{ "io_id": 1, "direction": "out", "output_buffer": null }}
                ],
                "pipeline": {{ "steps": {} }},
                "security": null
            }}"#,
            TINY_JPEG_HEX, steps_json
        )
    }

    #[test]
    fn v2_build_parses_and_routes() {
        let json = build_request_json(
            r#"[
                { "decode": { "io_id": 0 } },
                { "constrain": { "mode": "fit", "w": 100, "h": 100 } },
                { "encode": { "io_id": 1, "format": "jpeg" } }
            ]"#,
        );

        let mut ctx = Context::create().unwrap();
        let response = crate::json::invoke(&mut ctx, "v2/build", json.as_bytes());

        // Should fail at execution (adapter not implemented), not at parsing/routing/compilation
        let err = response.unwrap_err();
        let msg = format!("{}", err);
        assert!(
            msg.contains("not yet implemented"),
            "Expected 'not yet implemented' error, got: {}",
            msg
        );
    }

    #[test]
    fn v2_execute_parses_and_routes() {
        let json = r#"{
            "pipeline": {
                "steps": [
                    { "flip_h": null },
                    { "flip_v": null }
                ]
            }
        }"#;

        let mut ctx = Context::create().unwrap();
        let response = crate::json::invoke(&mut ctx, "v2/execute", json.as_bytes());

        // Should fail at execution, not at routing/parsing
        let err = response.unwrap_err();
        let msg = format!("{}", err);
        assert!(
            msg.contains("not yet implemented"),
            "Expected 'not yet implemented' error, got: {}",
            msg
        );
    }

    #[test]
    fn v2_unknown_endpoint_returns_error() {
        let mut ctx = Context::create().unwrap();
        let response = crate::json::invoke(&mut ctx, "v2/nonexistent", b"{}");
        assert!(response.is_err());
    }

    #[test]
    fn v2_build_invalid_json_returns_parse_error() {
        let mut ctx = Context::create().unwrap();
        let response = crate::json::invoke(&mut ctx, "v2/build", b"not json");
        assert!(response.is_err());
    }

    #[test]
    fn v1_routing_unaffected() {
        // Verify v1 endpoints still work after adding v2 routing
        let mut ctx = Context::create().unwrap();
        let response = crate::json::invoke(&mut ctx, "v1/get_version_info", b"{}");
        let resp = response.unwrap();
        assert!(resp.status_2xx());
    }
}
