// We need to make sure the schema is updated by calling that endpoint.
// Call the endpoint and make sure the schema is updated.
use imageflow_core::Context;
#[test]
fn test_schema_endpoint() {
    let mut context = Context::create().unwrap();

    // Call the schema endpoint to get the current schema
    let (json_response, result) = context.message("v1/schema/openapi/latest/get", &[]);
    assert!(result.is_ok(), "Schema endpoint should not return an error");

    let status_code = json_response.status_code;
    assert_eq!(status_code, 200, "Schema endpoint should return 200 OK");
    let json_bytes  = json_response.response_json;

    // Parse the response wrapper as JSON
    let response_json: serde_json::Value = serde_json::from_slice(&json_bytes).unwrap();
    assert!(response_json.is_object(), "Response should be a JSON object");

    // The actual schema is in the 'data' field of the response, as a JSON-encoded string.
    let data_field = response_json.get("data").expect("Response JSON should have a 'data' field");
    let schema_string = data_field.as_str().expect("'data' field should be a string containing the schema");
    let schema_json: serde_json::Value = serde_json::from_str(schema_string).expect("Failed to parse schema string from 'data' field");

    // Verify the schema contains expected top-level keys
    assert!(schema_json.is_object(), "Schema should be a JSON object");

    // Check for some expected schema properties
    let schema_obj = schema_json.as_object().unwrap();

    let mut root_children_str = String::new();
    for key in schema_obj.keys() {
        root_children_str += key;
        root_children_str += ", ";
    }

    assert!(schema_obj.contains_key("openapi"), "Schema should contain openapi key, only contains: {:?}", root_children_str);
    assert!(schema_obj.contains_key("components"), "Schema should contain components key, only contains: {:?}", root_children_str);
    assert!(schema_obj.contains_key("paths"), "Schema should contain paths key, only contains: {:?}", root_children_str);

    // Verify the schema is not empty
    assert!(!schema_obj.is_empty(), "Schema should not be empty");

    // Clean up
    context.destroy().unwrap();
}








