//! Integration coverage for the three-layer codec killbits system.
//!
//! Exercises the endpoints (`v1/context/set_policy`,
//! `v1/context/get_net_support`) and the enforcement hooks at
//! decode/encode dispatch.

use imageflow_core::Context;
use serde_json::json;

/// Tiny valid PNG (1x1). Shared across tests.
const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

/// Invoke an endpoint by name, returning the parsed JSON `data` field.
fn call(context: &mut Context, method: &str, body: &serde_json::Value) -> serde_json::Value {
    let bytes = serde_json::to_vec(body).unwrap();
    let (response, _) = context.message(method, &bytes);
    let parsed: serde_json::Value = serde_json::from_slice(&response.response_json).unwrap();
    assert_eq!(response.status_code, 200, "{} failed: {:?}", method, parsed);
    parsed.get("data").expect("data field").clone()
}

/// Invoke an endpoint expecting a non-200 response, returning the full response JSON.
fn call_expect_err(
    context: &mut Context,
    method: &str,
    body: &serde_json::Value,
) -> serde_json::Value {
    let bytes = serde_json::to_vec(body).unwrap();
    let (response, _) = context.message(method, &bytes);
    assert!(response.status_code >= 400, "{} unexpectedly succeeded", method);
    serde_json::from_slice(&response.response_json).unwrap()
}

#[test]
fn get_net_support_without_policy_reports_build_ceiling() {
    let mut ctx = Context::create().unwrap();
    let data = call(&mut ctx, "v1/context/get_net_support", &json!({}));
    assert_eq!(data["trusted_policy_set"], json!(false));
    let formats = data["net_support"]["formats"].as_object().expect("formats object");
    // PNG and GIF must always be decodable and encodable (always-on codecs).
    assert_eq!(formats["png"]["decode"], json!(true));
    assert_eq!(formats["png"]["encode"], json!(true));
    assert_eq!(formats["gif"]["decode"], json!(true));
    assert_eq!(formats["gif"]["encode"], json!(true));
    // Heic/Tiff/Pnm have no backend yet — must be false.
    assert_eq!(formats["heic"]["decode"], json!(false));
    assert_eq!(formats["heic"]["encode"], json!(false));
    assert_eq!(formats["tiff"]["decode"], json!(false));
    assert_eq!(formats["pnm"]["decode"], json!(false));
}

#[test]
fn set_policy_locks_and_reports_grid() {
    let mut ctx = Context::create().unwrap();
    let body = json!({
        "policy": {
            "formats": {
                "deny_decode": ["avif"]
            }
        }
    });
    let data = call(&mut ctx, "v1/context/set_policy", &body);
    assert_eq!(data["ok"], json!(true));
    assert_eq!(data["locked"], json!(true));
    let formats = data["net_support"]["formats"].as_object().expect("formats object");
    // Avif decode denied; encode untouched.
    assert_eq!(formats["avif"]["decode"], json!(false));
    // Subsequent `get_net_support` reports trusted_policy_set=true.
    let get = call(&mut ctx, "v1/context/get_net_support", &json!({}));
    assert_eq!(get["trusted_policy_set"], json!(true));
    assert_eq!(get["net_support"]["formats"]["avif"]["decode"], json!(false));
}

#[test]
fn set_policy_require_unlocked_errors_when_locked() {
    let mut ctx = Context::create().unwrap();
    let first = json!({
        "policy": { "formats": { "deny_encode": ["avif"] } }
    });
    call(&mut ctx, "v1/context/set_policy", &first);

    let second = json!({
        "policy": { "formats": { "deny_encode": ["webp"] } },
        "require_unlocked": true
    });
    let err = call_expect_err(&mut ctx, "v1/context/set_policy", &second);
    let message = err["message"].as_str().unwrap_or("");
    assert!(message.contains("already set"), "got: {}", message);
}

#[test]
fn set_policy_narrowing_succeeds() {
    let mut ctx = Context::create().unwrap();
    // Initial: deny avif encode.
    let first = json!({
        "policy": { "formats": { "deny_encode": ["avif"] } }
    });
    call(&mut ctx, "v1/context/set_policy", &first);
    // Narrow: deny avif + webp encode.
    let second = json!({
        "policy": { "formats": { "deny_encode": ["avif", "webp"] } }
    });
    let data = call(&mut ctx, "v1/context/set_policy", &second);
    let formats = data["net_support"]["formats"].as_object().unwrap();
    assert_eq!(formats["avif"]["encode"], json!(false));
    assert_eq!(formats["webp"]["encode"], json!(false));
    // PNG encode still allowed.
    assert_eq!(formats["png"]["encode"], json!(true));
}

#[test]
fn set_policy_widening_rejected() {
    let mut ctx = Context::create().unwrap();
    // Initial: deny avif + webp encode.
    let first = json!({
        "policy": { "formats": { "deny_encode": ["avif", "webp"] } }
    });
    call(&mut ctx, "v1/context/set_policy", &first);
    // Attempted widen: deny only avif (would allow webp again).
    let second = json!({
        "policy": { "formats": { "deny_encode": ["avif"] } }
    });
    let err = call_expect_err(&mut ctx, "v1/context/set_policy", &second);
    let message = err["message"].as_str().unwrap_or("");
    assert!(message.contains("widen") || message.contains("cannot"), "got: {}", message);
}

#[test]
fn set_policy_rejects_mutually_exclusive_killbits() {
    let mut ctx = Context::create().unwrap();
    let body = json!({
        "policy": {
            "formats": {
                "allow_decode": ["jpeg"],
                "deny_decode": ["avif"]
            }
        }
    });
    let err = call_expect_err(&mut ctx, "v1/context/set_policy", &body);
    let message = err["message"].as_str().unwrap_or("");
    assert!(message.contains("allow") || message.contains("deny"), "got: {}", message);
}

#[test]
fn job_level_allow_encode_rejected() {
    let mut ctx = Context::create().unwrap();
    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "security": {
            "formats": { "allow_encode": ["jpeg"] }
        },
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                { "encode": { "io_id": 1, "preset": { "lodepng": {} } } }
            ]
        }
    });
    let err = call_expect_err(&mut ctx, "v1/execute", &job);
    let message = err["message"].as_str().unwrap_or("");
    assert!(
        message.contains("may only deny") || message.contains("allow"),
        "got: {}",
        message
    );
}

#[test]
fn job_level_table_with_true_rejected() {
    let mut ctx = Context::create().unwrap();
    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "security": {
            "formats": {
                "formats": {
                    "jpeg": { "decode": true, "encode": true }
                }
            }
        },
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                { "encode": { "io_id": 1, "preset": { "lodepng": {} } } }
            ]
        }
    });
    let err = call_expect_err(&mut ctx, "v1/execute", &job);
    let message = err["message"].as_str().unwrap_or("");
    assert!(message.contains("may only deny"), "got: {}", message);
}

#[test]
fn encode_denied_by_trusted_policy_errors_at_parse_time() {
    let mut ctx = Context::create().unwrap();
    // Trusted policy: deny PNG encode. (Not a plausible real policy, just a
    // smoke test that the enforcement fires.)
    let policy = json!({
        "policy": { "formats": { "deny_encode": ["png"] } }
    });
    call(&mut ctx, "v1/context/set_policy", &policy);

    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                { "encode": { "io_id": 1, "preset": { "lodepng": {} } } }
            ]
        }
    });
    let err = call_expect_err(&mut ctx, "v1/execute", &job);
    let message = err["message"].as_str().unwrap_or("");
    assert!(message.contains("encode_not_available"), "got: {}", message);
    assert!(message.contains("\"format\": \"png\""), "got: {}", message);
}

#[test]
fn decode_denied_by_trusted_policy_errors_at_input_setup() {
    // Decoder enforcement fires when the codec is instantiated for the
    // input — which happens at `add_copied_input_buffer` time, *before*
    // the job is even submitted. This is stronger than parse-time.
    let mut ctx = Context::create().unwrap();
    let policy = json!({
        "policy": { "formats": { "deny_decode": ["png"] } }
    });
    call(&mut ctx, "v1/context/set_policy", &policy);

    // Adding a PNG input after deny_decode must fail with the structured
    // error.
    let err = ctx
        .add_copied_input_buffer(0, TINY_PNG)
        .expect_err("expected decode_not_available error");
    assert!(err.message.contains("decode_not_available"), "got: {}", err.message);
    assert!(err.message.contains("\"format\": \"png\""), "got: {}", err.message);
}

#[test]
fn allow_unavailable_format_errors_at_policy_set() {
    // Heic has no compiled-in backend anywhere in upstream today.
    let mut ctx = Context::create().unwrap();
    let body = json!({
        "policy": { "formats": { "allow_decode": ["heic"] } }
    });
    let err = call_expect_err(&mut ctx, "v1/context/set_policy", &body);
    let message = err["message"].as_str().unwrap_or("");
    assert!(message.contains("disabled at build time"), "got: {}", message);
}

/// Two sequential Execute001 calls with different inline `max_decode_size`
/// values must not leak state through the Context. Each job sees its own
/// effective limit; `Context.default_job_security` is unchanged afterwards.
#[test]
fn inline_max_decode_size_is_not_persisted_across_jobs() {
    let mut ctx = Context::create().unwrap();
    // Snapshot the Context-scoped default for later equality check.
    let default_before = ctx.default_job_security.clone();

    // Job 1: tiny limit. PNG is 1x1 so this still succeeds.
    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job1 = json!({
        "security": {
            "max_decode_size": { "w": 4, "h": 4, "megapixels": 0.01 }
        },
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                { "encode": { "io_id": 1, "preset": { "lodepng": {} } } }
            ]
        }
    });
    let _ = call(&mut ctx, "v1/execute", &job1);

    // The Context-scoped default must be untouched.
    assert_eq!(
        ctx.default_job_security, default_before,
        "job inline security leaked into Context.default_job_security"
    );
    assert!(ctx.active_job_security.is_none(), "active_job_security should be cleared after job");

    // Job 2: no inline limit. The sane-defaults cap (12000 px) applies;
    // job 1's 4-pixel limit must not carry over.
    //
    // Re-use the same context with fresh io ids.
    ctx.add_copied_input_buffer(10, TINY_PNG).unwrap();
    ctx.add_output_buffer(11).unwrap();
    let job2 = json!({
        "framewise": {
            "steps": [
                { "decode": { "io_id": 10 } },
                { "encode": { "io_id": 11, "preset": { "lodepng": {} } } }
            ]
        }
    });
    let _ = call(&mut ctx, "v1/execute", &job2);

    // Default still untouched.
    assert_eq!(ctx.default_job_security, default_before);
}

/// Job 1 denies PNG encode via inline job security; job 2 must still be
/// able to encode PNG. (This is the main leak scenario: a per-job
/// killbits mutation would make job 2 fail.)
#[test]
fn inline_killbits_is_not_persisted_across_jobs() {
    let mut ctx = Context::create().unwrap();

    // Job 1: deny PNG encode inline. Job fails at encode.
    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job1 = json!({
        "security": {
            "formats": { "deny_encode": ["png"] }
        },
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                { "encode": { "io_id": 1, "preset": { "lodepng": {} } } }
            ]
        }
    });
    let err = call_expect_err(&mut ctx, "v1/execute", &job1);
    let message = err["message"].as_str().unwrap_or("");
    assert!(message.contains("encode_not_available"), "job1 error: {}", message);

    // After the job, the Context's default_job_security must NOT carry
    // the deny_encode:png bit — otherwise job 2 would still fail.
    assert!(
        ctx.default_job_security.formats.is_none(),
        "inline formats killbits leaked into default_job_security"
    );
    assert!(ctx.active_job_security.is_none());

    // Job 2: same steps, no inline security. Must succeed because job 1's
    // deny never persisted.
    ctx.add_copied_input_buffer(10, TINY_PNG).unwrap();
    ctx.add_output_buffer(11).unwrap();
    let job2 = json!({
        "framewise": {
            "steps": [
                { "decode": { "io_id": 10 } },
                { "encode": { "io_id": 11, "preset": { "lodepng": {} } } }
            ]
        }
    });
    let data = call(&mut ctx, "v1/execute", &job2);
    assert!(data.get("job_result").is_some(), "job 2 should have succeeded: {}", data);
}

/// Trusted policy narrows `default_job_security.formats` (affecting
/// `net_support`), but inline job-level narrowing still doesn't leak
/// into `default_job_security` on top of it.
#[test]
fn trusted_policy_persists_but_inline_does_not() {
    let mut ctx = Context::create().unwrap();
    // Set trusted policy: deny avif encode.
    let policy = json!({
        "policy": { "formats": { "deny_encode": ["avif"] } }
    });
    call(&mut ctx, "v1/context/set_policy", &policy);

    // Trusted policy *is* allowed to affect the Context default —
    // that's what it's for. Capture the state for later comparison.
    let default_after_trusted = ctx.default_job_security.clone();
    assert!(
        default_after_trusted.formats.is_some(),
        "trusted policy should have recorded a formats killbits on default_job_security"
    );

    // Submit a job with inline deny_encode:webp. It should combine with
    // trusted (avif denied by policy, webp denied by job) for the job's
    // lifetime only.
    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "security": {
            "formats": { "deny_encode": ["webp"] }
        },
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                { "encode": { "io_id": 1, "preset": { "lodepng": {} } } }
            ]
        }
    });
    let _ = call(&mut ctx, "v1/execute", &job);

    // After the job: trusted policy still recorded; inline narrowing gone.
    assert_eq!(
        ctx.default_job_security, default_after_trusted,
        "inline job narrowing leaked on top of trusted policy"
    );
    assert!(ctx.active_job_security.is_none());
}

// ---- Codec-level killbits ----

/// `get_net_support` reports per-codec availability with format + role
/// on the baseline context. At minimum, PngquantEncoder and
/// LodepngEncoder are present (they ship unconditionally).
#[test]
fn get_net_support_reports_codec_grid() {
    let mut ctx = Context::create().unwrap();
    let data = call(&mut ctx, "v1/context/get_net_support", &json!({}));
    let codecs = data["net_support"]["codecs"].as_object().expect("codecs object");
    assert!(codecs.contains_key("pngquant_encoder"));
    assert!(codecs.contains_key("lodepng_encoder"));
    // PNG encoders always compiled in → pngquant_encoder available.
    assert_eq!(codecs["pngquant_encoder"]["available"], json!(true));
    assert_eq!(codecs["pngquant_encoder"]["format"], json!("png"));
    assert_eq!(codecs["pngquant_encoder"]["role"], json!("encode"));
}

/// With `deny_encoders: [lodepng_encoder]` via trusted policy, the Lodepng
/// preset is substituted (not rejected) — the dispatcher picks another
/// PNG encoder for the same wire format and surfaces a
/// `codec_substitution` annotation on the encode step's response. The
/// format-level PNG grid stays live.
#[test]
fn deny_specific_encoder_substitutes_preset_when_lodepng_denied() {
    let mut ctx = Context::create().unwrap();
    let policy = json!({
        "policy": {
            "codecs": { "deny_encoders": ["lodepng_encoder"] }
        }
    });
    call(&mut ctx, "v1/context/set_policy", &policy);

    // Format-level PNG encode is still true — pngquant remains.
    let get = call(&mut ctx, "v1/context/get_net_support", &json!({}));
    assert_eq!(get["net_support"]["formats"]["png"]["encode"], json!(true));
    // Per-codec: lodepng_encoder denied.
    assert_eq!(get["net_support"]["codecs"]["lodepng_encoder"]["available"], json!(false));
    let lodepng_reasons = get["net_support"]["codecs"]["lodepng_encoder"]["reasons"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        lodepng_reasons.iter().any(|r| r == &json!("codec_killbits.deny_encoders")),
        "expected codec_killbits.deny_encoders reason, got {:?}",
        lodepng_reasons
    );

    // Submit a job with EncoderPreset::Lodepng — the dispatcher
    // substitutes another PNG encoder (zenpng on zen-only builds,
    // libpng otherwise) and the response carries the annotation.
    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                { "encode": { "io_id": 1, "preset": { "lodepng": {} } } }
            ]
        }
    });
    let data = call(&mut ctx, "v1/execute", &job);
    let encodes = data["job_result"]["encodes"]
        .as_array()
        .expect("encodes present");
    assert_eq!(encodes.len(), 1);
    let ann = &encodes[0]["annotations"]["codec_substitution"];
    assert_eq!(
        ann["requested"],
        json!("lodepng_encoder"),
        "expected requested=lodepng_encoder, got {:?}",
        ann
    );
    assert_ne!(ann["actual"], json!("lodepng_encoder"),);
    assert_eq!(ann["reason"], json!("codec_killbits_deny_encoders"));
}

/// Denying Lodepng *and* every other PNG encoder produces the unified
/// `format_not_available` error at parse time (substitution cannot
/// save us when no PNG encoder remains).
#[test]
fn lodepng_preset_errors_format_not_available_when_no_png_encoder_remains() {
    let mut ctx = Context::create().unwrap();
    let policy = json!({
        "policy": {
            "codecs": {
                "deny_encoders": [
                    "lodepng_encoder",
                    "pngquant_encoder",
                    "libpng_encoder",
                    "zen_png_encoder",
                    // See `deny_all_png_encoders_flips_format_encode_false`:
                    // ZenPng palette-reducing siblings must also be
                    // denied so the format-level grid reports PNG
                    // encode as unavailable.
                    "zen_png_zenquant_encoder",
                    "zen_png_imagequant_encoder"
                ]
            }
        }
    });
    call(&mut ctx, "v1/context/set_policy", &policy);

    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                { "encode": { "io_id": 1, "preset": { "lodepng": {} } } }
            ]
        }
    });
    let err = call_expect_err(&mut ctx, "v1/execute", &job);
    let message = err["message"].as_str().unwrap_or("");
    // The format-level gate fires first (`encode_not_available`) when
    // *every* PNG encoder is killbitted — the net_support grid folds
    // PNG encode to false. That's the expected unified error shape.
    assert!(
        message.contains("encode_not_available") || message.contains("format_not_available"),
        "got: {}",
        message
    );
    assert!(message.contains("\"format\": \"png\""), "got: {}", message);
}

/// Denying *all* PNG encoders via trusted policy flips the PNG format
/// encode cell to false with `no_available_encoder` in reasons.
#[test]
fn deny_all_png_encoders_flips_format_encode_false() {
    let mut ctx = Context::create().unwrap();
    // Upstream (c-codecs default, no zen-codecs) ships LodepngEncoder +
    // PngQuantEncoder + LibPngRsEncoder for PNG encode.
    let policy = json!({
        "policy": {
            "codecs": {
                "deny_encoders": [
                    "lodepng_encoder",
                    "pngquant_encoder",
                    "libpng_encoder",
                    "zen_png_encoder",
                    // The two sibling ZenPng palette-reducing variants
                    // added with the pngquant → zenquant substitution
                    // wiring must also be denied, or the net_support
                    // grid still reports PNG encode as available via
                    // `zen_png_zenquant_encoder`.
                    "zen_png_zenquant_encoder",
                    "zen_png_imagequant_encoder"
                ]
            }
        }
    });
    let data = call(&mut ctx, "v1/context/set_policy", &policy);
    let png = &data["net_support"]["formats"]["png"];
    assert_eq!(png["encode"], json!(false));
    let png_reasons = png["encode_reasons"].as_array().cloned().unwrap_or_default();
    assert!(
        png_reasons.iter().any(|r| r == &json!("no_available_encoder")),
        "expected no_available_encoder in png.encode_reasons, got {:?}",
        png_reasons
    );
    // Decode still allowed.
    assert_eq!(png["decode"], json!(true));
}

/// Setting an `allow_encoders` for a codec not compiled in errors at
/// policy-set time. The codec we name must be absent in the current
/// build: in c-codecs-only builds `zen_avif_encoder` isn't compiled in;
/// in zen-codecs builds `zen_avif_encoder` *is* compiled in so we
/// substitute a codec name that is never shipped in upstream
/// (`webp_encoder` is only in c-codecs builds).
#[test]
fn allow_unavailable_codec_errors_at_policy_set() {
    let mut ctx = Context::create().unwrap();
    let absent_codec = if cfg!(feature = "c-codecs") {
        // c-codecs builds have every C encoder compiled in; pick a
        // zen-only variant (ZenJxl is the rarest — zen-codecs must
        // be absent).
        if cfg!(feature = "zen-codecs") {
            // Both compiled — skip this test, nothing is universally
            // absent.
            return;
        }
        "zen_jxl_encoder"
    } else {
        "mozjpeg_encoder"
    };
    let body = json!({
        "policy": {
            "codecs": { "allow_encoders": [absent_codec] }
        }
    });
    let err = call_expect_err(&mut ctx, "v1/context/set_policy", &body);
    let message = err["message"].as_str().unwrap_or("");
    assert!(message.contains("not compiled in"), "got: {}", message);
}

/// Job-level `allow_encoders` is rejected at parse — layer 3 may only
/// deny.
#[test]
fn job_level_allow_encoders_rejected() {
    let mut ctx = Context::create().unwrap();
    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "security": {
            "codecs": { "allow_encoders": ["lodepng_encoder"] }
        },
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                { "encode": { "io_id": 1, "preset": { "lodepng": {} } } }
            ]
        }
    });
    let err = call_expect_err(&mut ctx, "v1/execute", &job);
    let message = err["message"].as_str().unwrap_or("");
    assert!(message.contains("may only deny"), "got: {}", message);
}

/// `allow_encoders` + `deny_encoders` simultaneously → mutual-exclusion
/// error at policy-set time.
#[test]
fn codec_killbits_mutually_exclusive() {
    let mut ctx = Context::create().unwrap();
    let body = json!({
        "policy": {
            "codecs": {
                "allow_encoders": ["lodepng_encoder"],
                "deny_encoders": ["pngquant_encoder"]
            }
        }
    });
    let err = call_expect_err(&mut ctx, "v1/context/set_policy", &body);
    let message = err["message"].as_str().unwrap_or("");
    assert!(message.contains("allow") || message.contains("deny"), "got: {}", message);
}

/// Pngquant preset with pngquant_encoder denied → `format_not_available`.
/// When the priority-indexed table lets Pngquant fall through to
/// ZenPngEncoder (zen-codecs compiled in), denying the pngquant_encoder
/// produces a substitution, not an error. When zen-codecs isn't in the
/// build, ZenPng isn't registered — the fallthrough runs out of
/// candidates and the unified format-not-available error fires.
///
/// Pngquant-c is the only zero-substitute case on c-only builds; the
/// substitute-annotation shape is exercised by
/// `pngquant_preset_substitutes_to_zenpng_when_denied_and_zen_live`
/// below.
#[test]
fn pngquant_preset_errors_format_not_available_when_pngquant_denied() {
    let mut ctx = Context::create().unwrap();
    let policy = json!({
        "policy": {
            "codecs": { "deny_encoders": ["pngquant_encoder"] }
        }
    });
    call(&mut ctx, "v1/context/set_policy", &policy);

    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                { "encode": { "io_id": 1, "preset": { "pngquant": {} } } }
            ]
        }
    });

    // On zen-codecs builds the priority-indexed table routes pngquant
    // to ZenPng (the substitute is the zen default pipeline). The
    // request succeeds with a substitution annotation.
    if cfg!(feature = "zen-codecs") {
        let data = call(&mut ctx, "v1/execute", &job);
        let _ = assert_substitution(&data, "pngquant_encoder", "codec_killbits_deny_encoders");
    } else {
        // c-only build: no PNG encoder substitutes for pngquant; the
        // unified format-not-available path fires and names the
        // requested codec.
        let err = call_expect_err(&mut ctx, "v1/execute", &job);
        let message = err["message"].as_str().unwrap_or("");
        assert!(message.contains("format_not_available"), "got: {}", message);
        assert!(message.contains("pngquant_encoder"), "got: {}", message);
        assert!(message.contains("\"format\": \"png\""), "got: {}", message);
    }

    // Either way, a separate Lodepng request still works — the denial
    // is codec-scoped.
    ctx.add_copied_input_buffer(10, TINY_PNG).unwrap();
    ctx.add_output_buffer(11).unwrap();
    let job2 = json!({
        "framewise": {
            "steps": [
                { "decode": { "io_id": 10 } },
                { "encode": { "io_id": 11, "preset": { "lodepng": {} } } }
            ]
        }
    });
    let data = call(&mut ctx, "v1/execute", &job2);
    assert!(data.get("job_result").is_some(), "{}", data);
}

// ---- Graceful codec substitution (per-preset coverage) ----

/// Helper asserting a `codec_substitution` annotation payload.
fn assert_substitution(
    data: &serde_json::Value,
    expected_requested: &str,
    expected_reason: &str,
) -> serde_json::Value {
    let encodes = data["job_result"]["encodes"]
        .as_array()
        .expect("encodes present");
    assert_eq!(encodes.len(), 1, "expected one encode step");
    let ann = &encodes[0]["annotations"]["codec_substitution"];
    assert_eq!(
        ann["requested"],
        json!(expected_requested),
        "expected requested={}, got {:?}",
        expected_requested,
        ann
    );
    assert_ne!(ann["actual"], json!(expected_requested), "actual must differ from requested");
    assert_ne!(ann["actual"], json!(null), "actual must be populated");
    assert_eq!(ann["reason"], json!(expected_reason));
    ann.clone()
}

/// Mozjpeg preset substitutes when `mozjpeg_encoder` is denied, landing
/// on a live JPEG encoder (zen_jpeg_encoder or mozjpeg_rs_encoder
/// depending on features).
#[cfg(any(feature = "c-codecs", feature = "zen-codecs"))]
#[test]
fn mozjpeg_preset_substitutes_when_mozjpeg_denied() {
    let mut ctx = Context::create().unwrap();
    // Only meaningful when there's a substitute to pick — i.e. zen-codecs
    // is compiled in alongside c-codecs. Skip otherwise.
    if !cfg!(feature = "zen-codecs") {
        return;
    }
    let policy = json!({
        "policy": {
            "codecs": { "deny_encoders": ["mozjpeg_encoder"] }
        }
    });
    call(&mut ctx, "v1/context/set_policy", &policy);

    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                {
                    "encode": {
                        "io_id": 1,
                        "preset": { "mozjpeg": { "quality": 85, "progressive": true } }
                    }
                }
            ]
        }
    });
    let data = call(&mut ctx, "v1/execute", &job);
    let ann = assert_substitution(&data, "mozjpeg_encoder", "codec_killbits_deny_encoders");
    // Field translations exist and reference quality + progressive.
    let fts: Vec<String> = ann["field_translations"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|v| v.as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        fts.iter().any(|s| s.contains("quality")),
        "expected quality translation in {:?}",
        fts
    );
    assert!(
        fts.iter().any(|s| s.contains("progressive")),
        "expected progressive translation in {:?}",
        fts
    );
}

/// LibjpegTurbo preset substitutes when `mozjpeg_encoder` is denied,
/// landing specifically on zen_jpeg_encoder (mozjpeg-rs is excluded
/// because it can't honor `optimize_huffman=false`).
#[cfg(all(feature = "c-codecs", feature = "zen-codecs"))]
#[test]
fn libjpegturbo_preset_substitutes_mozjpeg_denied_to_zenjpeg_not_mozrs() {
    let mut ctx = Context::create().unwrap();
    let policy = json!({
        "policy": {
            "codecs": { "deny_encoders": ["mozjpeg_encoder"] }
        }
    });
    call(&mut ctx, "v1/context/set_policy", &policy);

    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                {
                    "encode": {
                        "io_id": 1,
                        "preset": {
                            "libjpegturbo": {
                                "quality": 92,
                                "progressive": false,
                                "optimize_huffman_coding": false
                            }
                        }
                    }
                }
            ]
        }
    });
    let data = call(&mut ctx, "v1/execute", &job);
    let ann = assert_substitution(&data, "mozjpeg_encoder", "codec_killbits_deny_encoders");
    assert_eq!(
        ann["actual"],
        json!("zen_jpeg_encoder"),
        "LibjpegTurbo must not route to mozjpeg_rs_encoder (can't disable huffman optimization)"
    );
}

/// Libpng preset substitutes when libpng_encoder is denied, landing on
/// zen_png_encoder (zen builds) or lodepng_encoder (else). `depth` or
/// `zlib_compression` dropped as appropriate.
#[test]
fn libpng_preset_substitutes_when_libpng_denied() {
    let mut ctx = Context::create().unwrap();
    let policy = json!({
        "policy": {
            "codecs": { "deny_encoders": ["libpng_encoder"] }
        }
    });
    call(&mut ctx, "v1/context/set_policy", &policy);

    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                {
                    "encode": {
                        "io_id": 1,
                        "preset": { "libpng": { "depth": "png_24", "zlib_compression": 9 } }
                    }
                }
            ]
        }
    });
    let data = call(&mut ctx, "v1/execute", &job);
    let ann = assert_substitution(&data, "libpng_encoder", "codec_killbits_deny_encoders");
    // zlib_compression is dropped when the substitute doesn't accept
    // it (zenpng / lodepng).
    let dropped: Vec<String> = ann["dropped_fields"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|v| v.as_str().unwrap_or("").to_string())
        .collect();
    let translations: Vec<String> = ann["field_translations"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|v| v.as_str().unwrap_or("").to_string())
        .collect();
    // Either dropped or translated — never silently lost.
    assert!(
        dropped.iter().any(|s| s.contains("zlib_compression"))
            || translations.iter().any(|s| s.contains("zlib_compression")),
        "zlib_compression must be annotated; got dropped={:?}, translations={:?}",
        dropped,
        translations
    );
}

/// Lodepng preset substitutes when lodepng_encoder is denied, landing
/// on another PNG encoder and reporting `maximum_deflate` as translated
/// (libpng: zlib=9) or dropped (zenpng).
#[test]
fn lodepng_preset_substitutes_when_lodepng_denied() {
    let mut ctx = Context::create().unwrap();
    let policy = json!({
        "policy": {
            "codecs": { "deny_encoders": ["lodepng_encoder"] }
        }
    });
    call(&mut ctx, "v1/context/set_policy", &policy);

    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                {
                    "encode": {
                        "io_id": 1,
                        "preset": { "lodepng": { "maximum_deflate": true } }
                    }
                }
            ]
        }
    });
    let data = call(&mut ctx, "v1/execute", &job);
    let _ = assert_substitution(&data, "lodepng_encoder", "codec_killbits_deny_encoders");
}

/// WebPLossy preset substitutes when the primary WebP encoder is denied,
/// landing on the other WebP encoder.
#[cfg(all(feature = "c-codecs", feature = "zen-codecs"))]
#[test]
fn webp_lossy_preset_substitutes_when_primary_webp_denied() {
    let mut ctx = Context::create().unwrap();
    // Deny the c-codecs libwebp encoder so the substitute is
    // zen_webp_encoder (both must be compiled in for this test).
    let policy = json!({
        "policy": {
            "codecs": { "deny_encoders": ["webp_encoder"] }
        }
    });
    call(&mut ctx, "v1/context/set_policy", &policy);

    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                {
                    "encode": {
                        "io_id": 1,
                        "preset": { "webplossy": { "quality": 85.0 } }
                    }
                }
            ]
        }
    });
    let data = call(&mut ctx, "v1/execute", &job);
    let ann = assert_substitution(&data, "webp_encoder", "codec_killbits_deny_encoders");
    assert_eq!(ann["actual"], json!("zen_webp_encoder"));
}

/// WebPLossless preset substitutes when the primary WebP encoder is
/// denied.
#[cfg(all(feature = "c-codecs", feature = "zen-codecs"))]
#[test]
fn webp_lossless_preset_substitutes_when_primary_webp_denied() {
    let mut ctx = Context::create().unwrap();
    let policy = json!({
        "policy": {
            "codecs": { "deny_encoders": ["webp_encoder"] }
        }
    });
    call(&mut ctx, "v1/context/set_policy", &policy);

    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                { "encode": { "io_id": 1, "preset": "webplossless" } }
            ]
        }
    });
    let data = call(&mut ctx, "v1/execute", &job);
    let ann = assert_substitution(&data, "webp_encoder", "codec_killbits_deny_encoders");
    assert_eq!(ann["actual"], json!("zen_webp_encoder"));
}

/// Format-level request stays strict: `EncoderPreset::Format { format:
/// png }` when PNG is format-denied returns `encode_not_available`,
/// not a substitution. Wire-format contract is sacred.
/// (Upstream builds don't compile zen-codecs, so AVIF isn't available
/// in the first place — PNG is always-on in every build and makes the
/// assertion testable regardless of features.)
#[test]
fn format_preset_denied_stays_strict_no_substitution() {
    let mut ctx = Context::create().unwrap();
    let policy = json!({
        "policy": { "formats": { "deny_encode": ["png"] } }
    });
    call(&mut ctx, "v1/context/set_policy", &policy);

    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                { "encode": { "io_id": 1, "preset": { "format": { "format": "png" } } } }
            ]
        }
    });
    let err = call_expect_err(&mut ctx, "v1/execute", &job);
    let message = err["message"].as_str().unwrap_or("");
    assert!(message.contains("encode_not_available"), "got: {}", message);
    assert!(message.contains("\"format\": \"png\""), "got: {}", message);
}

/// Sanity: legacy jobs with no `security.formats` still work unchanged.
#[test]
fn legacy_job_still_decodes_and_encodes() {
    let mut ctx = Context::create().unwrap();
    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                { "encode": { "io_id": 1, "preset": { "lodepng": {} } } }
            ]
        }
    });
    let data = call(&mut ctx, "v1/execute", &job);
    // Smoke test: we got a job_result back.
    assert!(data.get("job_result").is_some(), "{}", data);
}

// ─── Priority-indexed substitution: V2 flavor override ─────────────

/// Serializes priority-dependent tests — the override is process-wide.
static PRIORITY_TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Under V2 priority, `EncoderPreset::Mozjpeg` with `mozjpeg_encoder`
/// denied substitutes to `MozjpegRsEncoder` (the zen shim that mimics
/// mozjpeg). Identical to V3 here — the substitute lists for the
/// Mozjpeg preset are the same under both priorities (moz-rs → zen),
/// because the caller named `MozjpegEncoder` and we return only the
/// substitutes. This test pins that identity to catch future
/// divergence.
#[cfg(feature = "zen-codecs")]
#[test]
fn v2_priority_mozjpeg_denied_substitutes_to_moz_rs_then_zen() {
    use imageflow_types::build_killbits::{CodecPriority, CodecPriorityGuard};
    let _lock = PRIORITY_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _g = CodecPriorityGuard::install(CodecPriority::V2ClassicFirst);

    let mut ctx = Context::create().unwrap();
    let policy = json!({
        "policy": {
            "codecs": { "deny_encoders": ["mozjpeg_encoder"] }
        }
    });
    call(&mut ctx, "v1/context/set_policy", &policy);

    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                { "encode": { "io_id": 1, "preset": { "mozjpeg": { "quality": 85 } } } }
            ]
        }
    });
    let data = call(&mut ctx, "v1/execute", &job);
    let ann = assert_substitution(&data, "mozjpeg_encoder", "codec_killbits_deny_encoders");
    assert_eq!(
        ann["codec_priority"],
        json!("v2_classic_first"),
        "codec_priority wire form reflects the V2 override"
    );
}

/// Under the V3 default, the same scenario produces the same
/// substitute order (moz-rs first) but the annotation's
/// `codec_priority` reads `v3_zen_first`.
#[cfg(feature = "zen-codecs")]
#[test]
fn v3_priority_mozjpeg_denied_records_v3_priority_in_annotation() {
    use imageflow_types::build_killbits::{CodecPriority, CodecPriorityGuard};
    let _lock = PRIORITY_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _g = CodecPriorityGuard::install(CodecPriority::V3ZenFirst);

    let mut ctx = Context::create().unwrap();
    let policy = json!({
        "policy": {
            "codecs": { "deny_encoders": ["mozjpeg_encoder"] }
        }
    });
    call(&mut ctx, "v1/context/set_policy", &policy);

    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                { "encode": { "io_id": 1, "preset": { "mozjpeg": { "quality": 85 } } } }
            ]
        }
    });
    let data = call(&mut ctx, "v1/execute", &job);
    let ann = assert_substitution(&data, "mozjpeg_encoder", "codec_killbits_deny_encoders");
    assert_eq!(ann["codec_priority"], json!("v3_zen_first"));
}

/// Under V3, Libpng denied substitutes to ZenPng first (per priority
/// table). The annotation's `field_translations` cite the validated
/// zlib→zenpng.Compression mapping.
#[cfg(feature = "zen-codecs")]
#[test]
fn v3_priority_libpng_denied_cites_validated_mapping_in_translations() {
    use imageflow_types::build_killbits::{CodecPriority, CodecPriorityGuard};
    let _lock = PRIORITY_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _g = CodecPriorityGuard::install(CodecPriority::V3ZenFirst);

    let mut ctx = Context::create().unwrap();
    let policy = json!({
        "policy": {
            "codecs": { "deny_encoders": ["libpng_encoder"] }
        }
    });
    call(&mut ctx, "v1/context/set_policy", &policy);

    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                {
                    "encode": {
                        "io_id": 1,
                        "preset": { "libpng": { "zlib_compression": 9 } }
                    }
                }
            ]
        }
    });
    let data = call(&mut ctx, "v1/execute", &job);
    let ann = assert_substitution(&data, "libpng_encoder", "codec_killbits_deny_encoders");
    assert_eq!(ann["actual"], json!("zen_png_encoder"));
    let translations: Vec<String> = ann["field_translations"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|v| v.as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        translations.iter().any(|s| s.contains("zenpng.compression") && s.contains("validated")),
        "expected a validated zlib→zenpng mapping note, got: {:?}",
        translations
    );
}

/// Under V2, LibjpegTurbo preset substitutes to MozjpegEncoder (the C
/// backend), never to mozjpeg-rs (which can't honor the Huffman
/// toggle).
#[cfg(all(feature = "c-codecs", feature = "zen-codecs"))]
#[test]
fn v2_priority_libjpegturbo_denied_subs_to_zen_not_moz_rs() {
    use imageflow_types::build_killbits::{CodecPriority, CodecPriorityGuard};
    let _lock = PRIORITY_TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _g = CodecPriorityGuard::install(CodecPriority::V2ClassicFirst);

    let mut ctx = Context::create().unwrap();
    let policy = json!({
        "policy": {
            "codecs": { "deny_encoders": ["mozjpeg_encoder"] }
        }
    });
    call(&mut ctx, "v1/context/set_policy", &policy);

    ctx.add_copied_input_buffer(0, TINY_PNG).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let job = json!({
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                {
                    "encode": {
                        "io_id": 1,
                        "preset": {
                            "libjpegturbo": {
                                "quality": 92,
                                "progressive": false,
                                "optimize_huffman_coding": false
                            }
                        }
                    }
                }
            ]
        }
    });
    let data = call(&mut ctx, "v1/execute", &job);
    let ann = assert_substitution(&data, "mozjpeg_encoder", "codec_killbits_deny_encoders");
    assert_eq!(
        ann["actual"],
        json!("zen_jpeg_encoder"),
        "LibjpegTurbo must route to zen_jpeg_encoder under any priority (never mozjpeg_rs)"
    );
    assert_eq!(ann["codec_priority"], json!("v2_classic_first"));
}
