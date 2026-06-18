//! End-to-end coverage for `v1/static/info`: call the endpoint, parse
//! the response, and sanity-check its shape.

use imageflow_core::Context;
use serde_json::json;

/// Invoke an endpoint by name, returning the parsed JSON `data` field.
fn call(context: &mut Context, method: &str, body: &serde_json::Value) -> serde_json::Value {
    let bytes = serde_json::to_vec(body).unwrap();
    let (response, _) = context.message(method, &bytes);
    let parsed: serde_json::Value = serde_json::from_slice(&response.response_json).unwrap();
    assert_eq!(response.status_code, 200, "{} failed: {:?}", method, parsed);
    parsed.get("data").expect("data field").clone()
}

#[test]
fn static_info_returns_200_and_expected_shape() {
    let mut ctx = Context::create().unwrap();
    let data = call(&mut ctx, "v1/static/info", &json!({}));
    let info = data.get("static_info").expect("static_info payload");

    // Top-level keys exist.
    assert!(info.get("imageflow_version").and_then(|v| v.as_str()).is_some());
    let build = info.get("build").expect("build info");
    assert!(build.get("features").and_then(|v| v.as_array()).is_some());
    assert!(build.get("codec_priority_default").and_then(|v| v.as_str()).is_some());

    // Formats: at least JPEG / PNG / GIF / WebP present.
    let formats = info
        .get("formats_available")
        .and_then(|v| v.as_object())
        .expect("formats_available");
    for fmt in ["jpeg", "png", "gif", "webp", "avif", "jxl"] {
        assert!(formats.contains_key(fmt), "format {fmt} missing");
    }
    // PNG is always encode+decode enabled.
    let png = &formats["png"];
    assert_eq!(png["encode"], json!(true));
    assert_eq!(png["decode"], json!(true));

    // Codecs: at least one entry.
    let codecs = info.get("codecs").and_then(|v| v.as_object()).expect("codecs");
    assert!(!codecs.is_empty(), "codecs table must not be empty");
    for (name, entry) in codecs.iter() {
        assert!(!name.is_empty(), "codec name must not be empty");
        let role = entry.get("role").and_then(|v| v.as_str()).unwrap_or("");
        assert!(
            role == "encode" || role == "decode",
            "codec {name} has unexpected role {role}"
        );
    }

    // RIAPI schema: has some keys and ignores unknown ones.
    let riapi = info.get("riapi").expect("riapi");
    assert_eq!(riapi["ignores_unknown_keys"], json!(true));
    let keys = riapi.get("keys").and_then(|v| v.as_array()).expect("keys array");
    assert!(keys.len() > 50, "RIAPI must report more than 50 keys");
    // `accept.webp` must be present with the right Accept header origin.
    let accept_webp = keys
        .iter()
        .find(|k| k.get("name").and_then(|v| v.as_str()) == Some("accept.webp"))
        .expect("accept.webp key");
    assert_eq!(accept_webp["accept_header_origin"], json!("image/webp"));

    // Server recommendations: Accept translation is populated.
    let recs = info
        .get("server_recommendations")
        .expect("server_recommendations");
    let translation = recs
        .get("accept_header_translation")
        .and_then(|v| v.as_object())
        .expect("accept_header_translation");
    assert_eq!(translation["image/webp"], json!("accept.webp=1"));
    assert_eq!(translation["image/avif"], json!("accept.avif=1"));
    assert_eq!(translation["image/jxl"], json!("accept.jxl=1"));
}

#[test]
fn static_info_is_independent_of_context() {
    // Two fresh Contexts must produce the exact same bytes — the
    // endpoint is process-static.
    let mut ctx_a = Context::create().unwrap();
    let mut ctx_b = Context::create().unwrap();
    let (resp_a, _) = ctx_a.message("v1/static/info", b"{}");
    let (resp_b, _) = ctx_b.message("v1/static/info", b"{}");
    assert_eq!(resp_a.status_code, 200);
    assert_eq!(resp_b.status_code, 200);
    assert_eq!(
        resp_a.response_json.as_ref(),
        resp_b.response_json.as_ref(),
        "v1/static/info must be identical across Contexts"
    );
}

#[test]
fn list_schema_endpoints_includes_static_info() {
    let mut ctx = Context::create().unwrap();
    let data = call(&mut ctx, "v1/schema/list-schema-endpoints", &json!({}));
    let endpoints = data
        .get("endpoints")
        .and_then(|v| v.as_array())
        .expect("endpoints list");
    let names: Vec<&str> = endpoints.iter().filter_map(|v| v.as_str()).collect();
    assert!(
        names.contains(&"/v1/static/info"),
        "/v1/static/info missing from list_schema_endpoints: {names:?}"
    );
}
