// Verify that the JSON API endpoints respond correctly.
use imageflow_core::Context;

#[test]
fn test_version_info_endpoint() {
    let context = Context::new();

    let json_response = context.send_json("v2/get_version_info", b"{}");

    assert_eq!(json_response.status_code, 200, "Version info endpoint should return 200 OK");

    let response_json: serde_json::Value =
        serde_json::from_slice(&json_response.response_json).unwrap();
    assert!(response_json["success"].as_bool().unwrap_or(false), "Response should indicate success");

    // Check that version_info data is present
    let data = response_json.get("data").expect("Response should have a 'data' field");
    let version_info = data.get("version_info").expect("Data should contain 'version_info'");

    assert!(
        version_info.get("version").is_some(),
        "version_info should contain a 'version' field"
    );
    assert!(
        version_info.get("codecs").is_some(),
        "version_info should contain a 'codecs' field"
    );

    let codecs = version_info["codecs"].as_array().expect("codecs should be an array");
    assert!(!codecs.is_empty(), "codecs list should not be empty");
}

#[test]
fn test_unknown_endpoint_returns_error() {
    let context = Context::new();

    let json_response = context.send_json("v1/nonexistent", b"{}");

    // Should return an error status for unknown methods
    assert_ne!(json_response.status_code, 200, "Unknown endpoint should not return 200");

    let response_json: serde_json::Value =
        serde_json::from_slice(&json_response.response_json).unwrap();
    assert!(
        !response_json["success"].as_bool().unwrap_or(true),
        "Unknown endpoint should not report success"
    );
}
