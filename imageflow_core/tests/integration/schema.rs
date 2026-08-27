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
    let json_bytes = json_response.response_json;

    // Parse the response wrapper as JSON
    let response_json: serde_json::Value = serde_json::from_slice(&json_bytes).unwrap();
    assert!(response_json.is_object(), "Response should be a JSON object");

    // The actual schema is in the 'data' field of the response, as a JSON-encoded string.
    let data_field = response_json.get("data").expect("Response JSON should have a 'data' field");
    let schema_string =
        data_field.as_str().expect("'data' field should be a string containing the schema");
    let schema_json: serde_json::Value = serde_json::from_str(schema_string)
        .expect("Failed to parse schema string from 'data' field");

    // Verify the schema contains expected top-level keys
    assert!(schema_json.is_object(), "Schema should be a JSON object");

    // Check for some expected schema properties
    let schema_obj = schema_json.as_object().unwrap();

    let mut root_children_str = String::new();
    for key in schema_obj.keys() {
        root_children_str += key;
        root_children_str += ", ";
    }

    assert!(
        schema_obj.contains_key("openapi"),
        "Schema should contain openapi key, only contains: {:?}",
        root_children_str
    );
    assert!(
        schema_obj.contains_key("components"),
        "Schema should contain components key, only contains: {:?}",
        root_children_str
    );
    assert!(
        schema_obj.contains_key("paths"),
        "Schema should contain paths key, only contains: {:?}",
        root_children_str
    );

    // Verify the schema is not empty
    assert!(!schema_obj.is_empty(), "Schema should not be empty");

    // Clean up
    context.destroy().unwrap();
}

/// Issue #700: `v1/schema/formats/v1/decodable` lists what this context can
/// decode and which implementation is preferred, reflecting runtime state.
#[test]
fn test_decodable_formats_endpoint_reflects_enabled_codecs() {
    use imageflow_core::NamedDecoders;
    use imageflow_types::json_messages::DecodableFormat;

    fn call(context: &mut Context) -> Vec<DecodableFormat> {
        let (response, result) = context.message("v1/schema/formats/v1/decodable", b"{}");
        assert!(result.is_ok(), "endpoint should succeed: {:?}", result.err());
        assert_eq!(response.status_code, 200);
        let json: serde_json::Value = serde_json::from_slice(&response.response_json).unwrap();
        assert_eq!(json["success"], serde_json::Value::Bool(true));
        serde_json::from_value(json["data"]["formats"].clone()).unwrap()
    }

    let mut context = Context::create().unwrap();
    let formats = call(&mut context);

    let names: Vec<&str> = formats.iter().map(|f| f.format.as_str()).collect();
    for expected in ["jpeg", "png", "gif", "bmp"] {
        assert!(names.contains(&expected), "expected {expected} in {names:?}");
    }
    assert_eq!(names.len(), names.iter().collect::<std::collections::HashSet<_>>().len());

    for f in &formats {
        assert!(!f.decoders.is_empty(), "{} lists no decoders", f.format);
        // Exactly one preferred decoder per format, and it is the first listed.
        assert_eq!(f.decoders.iter().filter(|d| d.preferred).count(), 1, "{:?}", f);
        assert!(f.decoders[0].preferred, "{:?}", f);
        for d in &f.decoders {
            assert!(d.backend == "v2" || d.backend == "zen", "{:?}", d);
            assert!(!d.name.is_empty());
        }
    }
    let bmp = formats.iter().find(|f| f.format == "bmp").unwrap();
    assert_eq!(bmp.decoders[0].name, "zenbitmaps");
    assert_eq!(bmp.decoders[0].backend, "zen");
    #[cfg(feature = "c-codecs")]
    {
        let jpeg = formats.iter().find(|f| f.format == "jpeg").unwrap();
        assert_eq!(jpeg.decoders[0].name, "mozjpeg");
        assert_eq!(jpeg.decoders[0].backend, "v2");
    }

    // Runtime state, not compile-time flags: disabling a decoder removes it.
    context.enabled_codecs.disable_decoder(NamedDecoders::ZenBmpDecoder);
    let after = call(&mut context);
    assert!(
        !after.iter().any(|f| f.format == "bmp"),
        "bmp should disappear once its only decoder is disabled: {after:?}"
    );
    assert_eq!(after.len(), formats.len() - 1);

    let (response, _) = context.message("v1/schema/list-schema-endpoints", b"{}");
    let json: serde_json::Value = serde_json::from_slice(&response.response_json).unwrap();
    let endpoints = json["data"]["endpoints"].as_array().unwrap();
    assert!(endpoints.iter().any(|e| e == "/v1/schema/formats/v1/decodable"), "{endpoints:?}");

    context.destroy().unwrap();
}

/// Issue #699: `v1/schema/riapi/v1/keys` returns the understood keys per backend
/// with a version identifier, sorted, de-duplicated, and counted.
#[test]
fn test_riapi_keys_by_backend_endpoint() {
    use imageflow_types::json_messages::RiapiKeysBackend;

    let mut context = Context::create().unwrap();
    let (response, result) = context.message("v1/schema/riapi/v1/keys", b"{}");
    assert!(result.is_ok(), "{:?}", result.err());
    assert_eq!(response.status_code, 200);
    let json: serde_json::Value = serde_json::from_slice(&response.response_json).unwrap();
    let backends: Vec<RiapiKeysBackend> =
        serde_json::from_value(json["data"]["backends"].clone()).unwrap();

    assert_eq!(backends.len(), 1, "only the v2 backend lives in this repo: {backends:?}");
    let v2 = &backends[0];
    assert_eq!(v2.backend, "v2");
    assert!(!v2.version.is_empty());
    assert_eq!(v2.count as usize, v2.keys.len());
    let mut sorted = v2.keys.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(v2.keys, sorted, "keys must be sorted and de-duplicated");

    // Same key set the flat legacy endpoint reports.
    let (legacy, _) = context.message("v1/schema/riapi/latest/list_keys", b"{}");
    let legacy: serde_json::Value = serde_json::from_slice(&legacy.response_json).unwrap();
    let mut legacy_keys: Vec<String> =
        serde_json::from_value(legacy["data"]["schema"]["key_names"].clone()).unwrap();
    legacy_keys.sort();
    legacy_keys.dedup();
    assert_eq!(v2.keys, legacy_keys);
    for k in ["w", "h", "mode", "c.gravity", "qp"] {
        assert!(v2.keys.iter().any(|x| x == k), "missing {k}");
    }

    let (response, _) = context.message("v1/schema/list-schema-endpoints", b"{}");
    let json: serde_json::Value = serde_json::from_slice(&response.response_json).unwrap();
    let endpoints = json["data"]["endpoints"].as_array().unwrap();
    assert!(endpoints.iter().any(|e| e == "/v1/schema/riapi/v1/keys"), "{endpoints:?}");
    context.destroy().unwrap();
}
