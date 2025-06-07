use crate::*;
use serde::{Deserialize, Serialize};
use std::any::TypeId;

// Re-export inventory's submit macro for use by the proc macro
pub use inventory::submit;

/// Represents HTTP methods supported by endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
    Trace,
}

/// Metadata for a registered API endpoint.
/// Instances of this struct will be collected by `inventory`.
#[derive(Debug, Clone)]
pub struct EndpointDefinition {
    /// The URL path (e.g., "/v1/build").
    pub path: &'static str,
    /// The HTTP method.
    pub method: HttpMethod,
    /// Optional tags for grouping in OpenAPI.
    pub tags: &'static [&'static str],
    /// Optional summary for the operation.
    pub summary: Option<&'static str>,
    /// Optional description for the operation.
    pub description: Option<&'static str>,
    /// Optional explicit operation ID.
    pub operation_id: Option<&'static str>,

    /// TypeId of the request struct (e.g., `TypeId::of::<Build001>()`).
    /// Used to look up the schema.
    pub request_type_id: Option<TypeId>,
    /// Static function to get the request schema name (if applicable).
    pub request_schema_name_fn: Option<fn() -> String>,
    /// Static function to get the request schema (if applicable).
    pub request_schema_fn: Option<fn() -> (String, utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>)>, 
    /// Is the request body required?
    pub request_required: bool,

    /// TypeId of the *success* response body struct (e.g., `TypeId::of::<BuildV1Response>()`).
    /// Assumes a 200/201/204 status code depending on method/return type.
    pub success_response_type_id: Option<TypeId>,
    /// Static function to get the success response schema name (if applicable).
    pub success_response_schema_name_fn: Option<fn() -> String>,
    /// Static function to get the success response schema (if applicable).
    pub success_response_schema_fn: Option<fn() -> (String, utoipa::openapi::RefOr<utoipa::openapi::schema::Schema>)>, 
    /// Description for the success response.
    pub success_description: &'static str,

    // TODO: Add support for multiple error responses (e.g., 400, 404, 500)
    // For now, assume a standard error response structure? Or require explicit definition?
    // pub error_responses: &'static [(u16, &'static str, TypeId)], // (status, desc, type_id)

    // --- Function Pointers (or alternative mechanism) for Dispatch ---
    // How to call the actual handler? This is tricky without generics or dynamic dispatch.
    // Option 1: Function pointer (requires specific signature)
    // pub handler_fn: fn(&mut imageflow_core::Context, request_json: &[u8]) -> imageflow_core::Result<imageflow_core::JsonResponse>,

    // Option 2: Store name and resolve later (less type-safe)
    // pub handler_name: &'static str,

    // Option 3: Type-erased wrapper? (Complex)

    // TODO: Decide on handler dispatch mechanism
}

// Make the definitions collectable by inventory.
inventory::collect!(EndpointDefinition);

// Helper function to simplify registration in generated code
pub fn submit_endpoint(endpoint: EndpointDefinition) {
    inventory::submit(endpoint);
}

// Function to get all registered endpoints
pub fn get_registered_endpoints() -> impl Iterator<Item = &'static EndpointDefinition> {
    inventory::iter::<EndpointDefinition>
} 