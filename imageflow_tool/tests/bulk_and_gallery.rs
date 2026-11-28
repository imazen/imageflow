//! Integration tests for bulk responsive processing, gallery building, XSS/schema hardening,
//! incremental caching, and the security-hardened interactive gallery designer server.

#![cfg(feature = "modern-cli")]

use imageflow_tool_lib::bulk::{
    AspectRatio, BULK_SCHEMA_VERSION, BulkConfig, BulkLimits, FailurePolicy, FitMode,
    GalleryConfig, GalleryLayout, GalleryTheme, OutputFormat, OutputPathLayout,
    QualityFallbackPolicy, QualityTarget, ResponsivePreset, WorkspaceAdmission, plan_breakpoints,
    resolve_variant_coding, run_bulk,
};
use imageflow_tool_lib::gallery::{
    build_gallery, init_gallery_config,
    server::{DesignerServerOptions, serve_gallery_designer},
};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

fn temp_test_dir(tag: &str) -> PathBuf {
    let base = std::env::var("HOME").unwrap_or_else(|_| ".".into()) + "/tmp/imageflow-gallery-tests";
    let dir = PathBuf::from(base).join(format!("{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create test dir");
    dir
}

fn make_gradient_png(w: u32, h: u32, seed: u8) -> Vec<u8> {
    let mut ctx = imageflow_core::Context::create().unwrap();
    ctx.add_output_buffer(1).unwrap();
    let req = serde_json::json!({
        "inputs": {},
        "outputs": { "out": 1 },
        "pipeline": {
            "steps": [
                { "canvas": { "width": w, "height": h, "fill": [seed.wrapping_add(40), seed.wrapping_add(110), 200u8.wrapping_sub(seed), 255] } },
                { "encode": { "output": "out", "format": "png", "coding": "lossless" } }
            ]
        }
    });
    ctx.message("v2/execute", req.to_string().as_bytes()).1.unwrap();
    ctx.take_output_buffer(1).unwrap()
}

fn make_textured_jpeg(w: u32, h: u32, seed: u8) -> Vec<u8> {
    // Build a deterministic gradient PNG via imageflow_core -> JPEG via v2/execute.
    let mut raw_png = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut raw_png, w, h);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().unwrap();
        let mut data = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            let yf = y as f32 / (h.max(1) as f32);
            for x in 0..w {
                let xf = x as f32 / (w.max(1) as f32);
                let wave = ((xf * 9.0 + yf * 5.0 + f32::from(seed)).sin() * 0.5 + 0.5) * 150.0;
                let r = (45.0 + 165.0 * xf).clamp(0.0, 255.0) as u8;
                let g = (40.0 + 160.0 * yf).clamp(0.0, 255.0) as u8;
                let b = (35.0 + wave).clamp(0.0, 255.0) as u8;
                data.extend_from_slice(&[r, g, b]);
            }
        }
        writer.write_image_data(&data).unwrap();
    }
    let mut ctx = imageflow_core::Context::create().unwrap();
    ctx.add_copied_input_buffer(0, &raw_png).unwrap();
    ctx.add_output_buffer(1).unwrap();
    let req = br#"{"inputs":{"src":0},"outputs":{"out":1},"pipeline":{"steps":[{"decode":{"input":"src"}},{"encode":{"output":"out","format":"jpeg","coding":{"calibrated_quality":92.0}}}]}}"#;
    ctx.message("v2/execute", req).1.unwrap();
    ctx.take_output_buffer(1).unwrap()
}

#[test]
fn planner_enforces_no_upscale_and_route_aware_verify_fallback() {
    let mut preset = ResponsivePreset::web_responsive();
    preset.widths_px = vec![300, 800, 1600];
    preset.allow_upscale = false;
    preset.aspect_ratio = Some(AspectRatio::new(16, 9).unwrap());
    preset.fit_mode = FitMode::FitCrop;

    // 640x480 source: 800 and 1600 must clamp to <= 640x360 without upscaling either axis!
    let bps = plan_breakpoints(640, 480, &preset);
    assert_eq!(bps.len(), 2, "800 and 1600 must deduplicate after no-upscale clamp: {bps:?}");
    assert_eq!((bps[0].width, bps[0].height), (300, 169));
    assert_eq!((bps[1].width, bps[1].height), (640, 360));
    assert!(bps.iter().all(|b| b.width <= 640 && b.height <= 480));

    // Route-aware coding truth table:
    let qt = QualityTarget::VerifyZensimA { target: 85.0 };
    let jpg = resolve_variant_coding(
        OutputFormat::Jpeg,
        &qt,
        QualityFallbackPolicy::StrictVerify,
        WorkspaceAdmission::Strict,
    )
    .unwrap();
    assert!(jpg.fallback_reason.is_none());

    // WebP Verify requires BestEffort; StrictVerify under Strict refuses up front!
    assert!(
        resolve_variant_coding(
            OutputFormat::Webp,
            &qt,
            QualityFallbackPolicy::StrictVerify,
            WorkspaceAdmission::Strict
        )
        .is_err()
    );
    let webp_be = resolve_variant_coding(
        OutputFormat::Webp,
        &qt,
        QualityFallbackPolicy::StrictVerify,
        WorkspaceAdmission::BestEffort,
    )
    .unwrap();
    assert!(webp_be.fallback_reason.is_none());

    // AVIF falls back transparently under Calibrated and refuses up front under StrictVerify!
    assert!(
        resolve_variant_coding(
            OutputFormat::Avif,
            &qt,
            QualityFallbackPolicy::StrictVerify,
            WorkspaceAdmission::BestEffort
        )
        .is_err()
    );
    let avif_cal = resolve_variant_coding(
        OutputFormat::Avif,
        &qt,
        QualityFallbackPolicy::Calibrated,
        WorkspaceAdmission::BestEffort,
    )
    .unwrap();
    assert_eq!(
        avif_cal.fallback_reason.as_deref(),
        Some("avif_encoder_verify_unavailable")
    );

    // AimForZensimA is supported on BOTH JPEG and WebP even WITHOUT a registered Verify IQA scorer!
    let aim = QualityTarget::AimForZensimA { target: 84.0 };
    let webp_aim = imageflow_tool_lib::bulk::planner::resolve_variant_coding_with_caps(
        OutputFormat::Webp,
        &aim,
        QualityFallbackPolicy::StrictVerify,
        WorkspaceAdmission::Strict,
        false,
    )
    .unwrap();
    assert!(webp_aim.fallback_reason.is_none());

    // AspectRatio > 100 in JSON must be rejected by Rust validate() (parity with C#).
    let bad_ar_json = r#"{"schema":"imageflow-gallery-v1","title":"Bad","design":{"grid_aspect_ratio":{"width":120,"height":63}}}"#;
    let bad_cfg: GalleryConfig = serde_json::from_str(bad_ar_json).unwrap();
    assert!(bad_cfg.validate().is_err());

    // Hex color CSS injection must be rejected by GalleryConfig::validate!
    let mut gcfg = GalleryConfig::default();
    gcfg.design.accent_color_hex = Some("#fff; background:url(javascript:alert(1))".into());
    assert!(gcfg.validate().is_err());
}

#[test]
fn bulk_single_decode_fanout_incremental_cache_and_static_gallery_xss_safety() {
    let root = temp_test_dir("bulk_and_gallery");
    let in_dir = root.join("photos");
    let out_dir = root.join("bulk_out");
    let site_dir = root.join("site_out");
    std::fs::create_dir_all(&in_dir).unwrap();

    std::fs::write(in_dir.join("01_sunset.jpg"), make_textured_jpeg(160, 120, 1)).unwrap();
    std::fs::write(in_dir.join("02_forest.png"), make_gradient_png(120, 160, 2)).unwrap();

    let mut preset = ResponsivePreset::web_responsive();
    preset.widths_px = vec![64, 128];
    preset.formats = vec![OutputFormat::Avif, OutputFormat::Webp, OutputFormat::Jpeg];
    preset.quality = QualityTarget::VerifyZensimA { target: 80.0 };
    preset.effort = imageflow_tool_lib::bulk::EncodeEffort::Fast;

    let cfg = BulkConfig {
        schema: BULK_SCHEMA_VERSION.to_string(),
        inputs: vec![in_dir.clone()],
        output_dir: out_dir.clone(),
        recursive: true,
        include_globs: Vec::new(),
        exclude_globs: Vec::new(),
        follow_symlinks: false,
        output_layout: OutputPathLayout::ContentAddressed,
        concurrency: 2,
        incremental_cache: true,
        force_rebuild: false,
        on_error: FailurePolicy::FailFast,
        write_manifest: true,
        write_snippets: true,
        preset: preset.clone(),
        limits: BulkLimits::default(),
    };

    let rep1 = run_bulk(&cfg, None, None).expect("first bulk run must succeed");
    assert_eq!(rep1.discovered, 2);
    assert_eq!(rep1.processed, 2);
    assert_eq!(rep1.skipped_cached, 0);
    assert_eq!(rep1.failed, 0);
    assert!(out_dir.join("manifest.json").is_file());
    assert!(out_dir.join("snippets.html").is_file());

    let sunset = rep1
        .manifest
        .assets
        .iter()
        .find(|a| a.source_relative_path == "01_sunset.jpg")
        .unwrap();
    assert_eq!((sunset.original_width, sunset.original_height), (160, 120));
    assert_eq!(sunset.variants.len(), 6, "2 widths x 3 formats = 6 variants");
    let jpg_v = sunset.variants.iter().find(|v| v.format == OutputFormat::Jpeg).unwrap();
    assert_eq!(jpg_v.verified_metric.as_deref(), Some("zensim_a"));
    assert!(jpg_v.verified_score.unwrap() >= 80.0);
    let avif_v = sunset.variants.iter().find(|v| v.format == OutputFormat::Avif).unwrap();
    assert_eq!(avif_v.fallback_reason.as_deref(), Some("avif_encoder_verify_unavailable"));
    assert!(sunset.picture_html.contains("image/avif"));
    assert!(sunset.picture_html.contains("image/webp"));
    assert!(sunset.picture_html.contains("64w"));
    assert!(sunset.picture_html.contains("128w"));

    // Second run must hit incremental cache 100%!
    let rep2 = run_bulk(&cfg, None, None).expect("second bulk run must reuse cache");
    assert_eq!(rep2.processed, 0);
    assert_eq!(rep2.skipped_cached, 2);

    // Build static gallery with XSS payloads in title and caption — MUST be escaped in HTML & JSON!
    let mut gcfg = init_gallery_config(&in_dir, Some("Test </script><script>alert(1)</script>".into()), "web-responsive").unwrap();
    gcfg.encoding = preset;
    gcfg.design.layout = GalleryLayout::Editorial;
    gcfg.design.theme = GalleryTheme::WarmLinen;
    gcfg.items.get_mut("01_sunset.jpg").unwrap().caption =
        Some("Caption \"with\" <img src=x onerror=alert(2)>".into());

    let grep1 = build_gallery(&in_dir, &site_dir, &gcfg, 2, false, BulkLimits::default(), None, None).unwrap();
    assert_eq!(grep1.visible_items, 2);
    assert!(grep1.index_html_path.is_file());
    assert!(grep1.gallery_json_path.is_file());
    assert!(grep1.config_toml_path.is_file());
    let html = std::fs::read_to_string(&grep1.index_html_path).unwrap();
    assert!(!html.contains("<script>alert(1)</script>"), "raw XSS script tag leaked into HTML!");
    assert!(!html.contains("<img src=x onerror=alert(2)>"), "raw XSS img tag leaked into HTML!");
    assert!(html.contains("layout-editorial"));

    // Layout-only change MUST reuse cached media variants with zero re-encoding!
    gcfg.design.layout = GalleryLayout::Masonry;
    gcfg.design.gap_px = 20;
    let grep2 = build_gallery(&in_dir, &site_dir, &gcfg, 2, false, BulkLimits::default(), None, None).unwrap();
    assert_eq!(grep2.bulk_report.processed, 0, "layout edit must NOT trigger image re-encoding!");
    assert_eq!(grep2.bulk_report.skipped_cached, 2);
    let html2 = std::fs::read_to_string(&grep2.index_html_path).unwrap();
    assert!(html2.contains("layout-masonry"));

    let _ = std::fs::remove_dir_all(&root);
}

fn http_roundtrip(port: u16, raw_req: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect studio server");
    stream.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
    stream.write_all(raw_req.as_bytes()).unwrap();
    stream.flush().unwrap();
    let mut resp = Vec::new();
    stream.read_to_end(&mut resp).unwrap();
    let s = String::from_utf8_lossy(&resp).into_owned();
    let code = s
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse::<u16>().ok())
        .unwrap_or(0);
    (code, s)
}

#[test]
fn designer_server_enforces_host_origin_csrf_and_serves_live_compare_and_export() {
    let root = temp_test_dir("designer_server");
    let in_dir = root.join("photos");
    let export_dir = root.join("export");
    std::fs::create_dir_all(&in_dir).unwrap();
    std::fs::write(in_dir.join("sample.jpg"), make_textured_jpeg(96, 64, 7)).unwrap();

    // Pick a free ephemeral loopback port.
    let probe = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);

    let stop = Arc::new(AtomicBool::new(false));
    let stop_srv = Arc::clone(&stop);
    let opts = DesignerServerOptions {
        input_dir: in_dir.clone(),
        export_dir: export_dir.clone(),
        config_path: Some(root.join("gallery.json")),
        bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port,
        access_token: None,
        public_origin: None,
    };
    let handle = std::thread::spawn(move || {
        serve_gallery_designer(opts, Some(stop_srv)).unwrap();
    });

    // Wait briefly until server accepts connections.
    for _ in 0..80 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }

    // 1. Invalid Host header MUST be refused (403); duplicate Host header MUST be refused (400).
    let (code_host, _) = http_roundtrip(
        port,
        "GET /api/state HTTP/1.1\r\nHost: evil.example:3380\r\nConnection: close\r\n\r\n",
    );
    assert_eq!(code_host, 403, "invalid Host header must be rejected");

    let (code_dup_host, _) = http_roundtrip(
        port,
        &format!(
            "GET /api/state HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
        ),
    );
    assert_eq!(code_dup_host, 400, "duplicate Host header must be rejected");

    // 2. GET / returns studio shell with embedded CSRF token and CSP header.
    let (code_root, resp_root) = http_roundtrip(
        port,
        &format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"),
    );
    assert_eq!(code_root, 200);
    assert!(resp_root.contains("Content-Security-Policy:"));
    let csrf = resp_root
        .split("data-csrf=\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .expect("embedded csrf token")
        .to_string();

    // 3. POST /api/export WITHOUT Origin/CSRF MUST be refused (403).
    let (code_nocsrf, _) = http_roundtrip(
        port,
        &format!(
            "POST /api/export HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
        ),
    );
    assert_eq!(code_nocsrf, 403, "POST without Origin/CSRF must be rejected");

    // 4. GET /api/state & /preview/gallery & /media/a0001.jpg succeed.
    let (code_state, resp_state) = http_roundtrip(
        port,
        &format!("GET /api/state HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"),
    );
    assert_eq!(code_state, 200);
    assert!(resp_state.contains("a0001"));

    let (code_media, _) = http_roundtrip(
        port,
        &format!("GET /media/a0001.jpg HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"),
    );
    assert_eq!(code_media, 200);

    // 5. POST /api/compare with valid Origin + CSRF returns live format bytes & zensim_a score!
    let cmp_body = r#"{"asset_id":"a0001","zensim_target":82.0,"preview_width":96,"formats":["webp","jpeg"]}"#;
    let (code_cmp, resp_cmp) = http_roundtrip(
        port,
        &format!(
            "POST /api/compare HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\nX-Imageflow-Csrf: {csrf}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cmp_body}",
            cmp_body.len()
        ),
    );
    assert_eq!(code_cmp, 200, "live compare failed: {resp_cmp}");
    assert!(resp_cmp.contains("\"measured_zensim_a\":"));

    // 5b. POST /api/save-config on a .json config_path MUST write reloadable JSON (not TOML)!
    let (code_save, resp_save) = http_roundtrip(
        port,
        &format!(
            "POST /api/save-config HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\nX-Imageflow-Csrf: {csrf}\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
        ),
    );
    assert_eq!(code_save, 200, "save-config failed: {resp_save}");
    let reloaded = GalleryConfig::load_file(&root.join("gallery.json"))
        .expect("saved gallery.json must reload cleanly as JSON");
    assert_eq!(reloaded.schema, "imageflow-gallery-v1");

    // 6a. Unreachable Verify(zensim_a=99.0) MUST return 422 (never silently 200 with 0 items) and leave export_dir clean!
    let mut bad_cfg = reloaded.clone();
    bad_cfg.encoding.quality = QualityTarget::VerifyZensimA { target: 99.0 };
    let bad_cfg_body = bad_cfg.to_json_string().unwrap();
    let (code_cfg_bad, _) = http_roundtrip(
        port,
        &format!(
            "POST /api/config HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\nX-Imageflow-Csrf: {csrf}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{bad_cfg_body}",
            bad_cfg_body.len()
        ),
    );
    assert_eq!(code_cfg_bad, 200);
    let (code_exp_bad, resp_exp_bad) = http_roundtrip(
        port,
        &format!(
            "POST /api/export HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\nX-Imageflow-Csrf: {csrf}\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
        ),
    );
    assert_eq!(code_exp_bad, 422, "unreachable Verify target must return 422, got: {resp_exp_bad}");
    assert!(!export_dir.join("index.html").exists(), "failed export must not publish empty index.html");

    // 6b. Restore valid zensim_a=82.0 config and export full static gallery to export_dir!
    let mut ok_cfg = reloaded;
    ok_cfg.encoding.quality = QualityTarget::VerifyZensimA { target: 82.0 };
    let ok_cfg_body = ok_cfg.to_json_string().unwrap();
    let (code_cfg_ok, _) = http_roundtrip(
        port,
        &format!(
            "POST /api/config HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\nX-Imageflow-Csrf: {csrf}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{ok_cfg_body}",
            ok_cfg_body.len()
        ),
    );
    assert_eq!(code_cfg_ok, 200);
    let (code_exp, resp_exp) = http_roundtrip(
        port,
        &format!(
            "POST /api/export HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\nX-Imageflow-Csrf: {csrf}\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}"
        ),
    );
    assert_eq!(code_exp, 200, "export failed: {resp_exp}");
    assert!(resp_exp.contains("\"visible_items\":1"));
    assert!(resp_exp.contains("\"failed\":0"));
    assert!(export_dir.join("index.html").is_file());
    assert!(export_dir.join("gallery.json").is_file());

    stop.store(true, Ordering::SeqCst);
    handle.join().unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

fn inject_exif_orientation_6(jpeg: &[u8]) -> Vec<u8> {
    assert!(jpeg.len() >= 2 && jpeg[0] == 0xFF && jpeg[1] == 0xD8);
    // APP1 Exif segment (34 bytes incl. length field): big-endian TIFF, IFD0 tag 0x0112 = 6 (Rotate90).
    let exif_app1: &[u8] = &[
        0xFF, 0xE1, 0x00, 0x22, b'E', b'x', b'i', b'f', 0x00, 0x00, b'M', b'M', 0x00, 0x2A, 0x00,
        0x00, 0x00, 0x08, 0x00, 0x01, 0x01, 0x12, 0x00, 0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0x06,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    let mut out = Vec::with_capacity(jpeg.len() + exif_app1.len());
    out.extend_from_slice(&jpeg[..2]);
    out.extend_from_slice(exif_app1);
    out.extend_from_slice(&jpeg[2..]);
    out
}

fn make_two_frame_gif() -> Vec<u8> {
    // Minimal valid 2x2 2-frame GIF89a with NETSCAPE2.0 loop + 2 Graphic Control Extensions.
    vec![
        b'G', b'I', b'F', b'8', b'9', b'a', 2, 0, 2, 0, 0x80, 0, 0, 255, 0, 0, 0, 0, 255, 0x21,
        0xFF, 11, b'N', b'E', b'T', b'S', b'C', b'A', b'P', b'E', b'2', b'.', b'0', 3, 1, 0, 0, 0,
        0x21, 0xF9, 4, 0, 10, 0, 0, 0, 0x2C, 0, 0, 0, 0, 2, 0, 2, 0, 0, 2, 2, 0x44, 0x01, 0, 0x21,
        0xF9, 4, 0, 10, 0, 0, 0, 0x2C, 0, 0, 0, 0, 2, 0, 2, 0, 0, 2, 2, 0x4C, 0x01, 0, 0x3B,
    ]
}

#[test]
fn inspect_reports_upright_exif_display_dims_and_animation_kind_and_refuses_symlinks() {
    let root = temp_test_dir("orientation_and_symlinks");
    let in_dir = root.join("in");
    let outside_dir = root.join("outside");
    let out_dir = root.join("out");
    std::fs::create_dir_all(&in_dir).unwrap();
    std::fs::create_dir_all(&outside_dir).unwrap();

    // 1. EXIF Orientation=6 (Rotate90) on 120x80 coded JPEG -> display must be 80x120!
    let rotated_jpg = inject_exif_orientation_6(&make_textured_jpeg(120, 80, 3));
    let mut ctx = imageflow_core::Context::create().unwrap();
    ctx.add_copied_input_buffer(0, &rotated_jpg).unwrap();
    let (resp, res) = ctx.message(
        "v2/inspect",
        br#"{"inputs":{"src":0},"input":"src","scan":"headers"}"#,
    );
    res.expect("v2/inspect rotated JPEG must succeed");
    let body: serde_json::Value = serde_json::from_slice(&resp.response_json).unwrap();
    let track = &body["data"]["tracks"][0];
    assert_eq!(track["kind"].as_str(), Some("image"));
    assert_eq!(track["coded"]["width"].as_u64(), Some(120));
    assert_eq!(track["coded"]["height"].as_u64(), Some(80));
    assert_eq!(track["display"]["width"].as_u64(), Some(80));
    assert_eq!(track["display"]["height"].as_u64(), Some(120));

    // 2. Animated 2-frame GIF -> kind must be "animation"!
    let gif = make_two_frame_gif();
    let mut gctx = imageflow_core::Context::create().unwrap();
    gctx.add_copied_input_buffer(0, &gif).unwrap();
    let (gresp, gres) = gctx.message(
        "v2/inspect",
        br#"{"inputs":{"src":0},"input":"src","scan":"headers"}"#,
    );
    gres.expect("v2/inspect animated GIF must succeed");
    let gbody: serde_json::Value = serde_json::from_slice(&gresp.response_json).unwrap();
    assert_eq!(gbody["data"]["tracks"][0]["kind"].as_str(), Some("animation"));

    // 3. Symlink pointing outside input_dir MUST be skipped when follow_symlinks=false!
    std::fs::write(in_dir.join("upright.jpg"), &rotated_jpg).unwrap();
    let secret_img = outside_dir.join("secret.jpg");
    std::fs::write(&secret_img, make_textured_jpeg(64, 64, 9)).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&secret_img, in_dir.join("escaped_link.jpg")).unwrap();

    let mut preset = ResponsivePreset::web_responsive();
    preset.widths_px = vec![60];
    preset.formats = vec![OutputFormat::Jpeg];
    preset.quality = QualityTarget::CalibratedQuality { quality: 85.0 };
    let cfg = BulkConfig {
        schema: BULK_SCHEMA_VERSION.to_string(),
        inputs: vec![in_dir.clone()],
        output_dir: out_dir.clone(),
        recursive: true,
        include_globs: Vec::new(),
        exclude_globs: Vec::new(),
        follow_symlinks: false,
        output_layout: OutputPathLayout::ContentAddressed,
        concurrency: 1,
        incremental_cache: true,
        force_rebuild: false,
        on_error: FailurePolicy::FailFast,
        write_manifest: true,
        write_snippets: false,
        preset,
        limits: BulkLimits::default(),
    };
    let rep = run_bulk(&cfg, None, None).expect("bulk run on rotated JPEG must succeed");
    assert_eq!(rep.discovered, 1, "symlink outside root must NOT be discovered");
    let a = &rep.manifest.assets[0];
    assert_eq!((a.original_width, a.original_height), (80, 120));
    assert_eq!((a.variants[0].width, a.variants[0].height), (60, 90));

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn designer_server_enforces_bearer_and_httponly_cookie_token_auth() {
    let root = temp_test_dir("designer_auth");
    let in_dir = root.join("photos");
    let export_dir = root.join("export");
    std::fs::create_dir_all(&in_dir).unwrap();
    std::fs::write(in_dir.join("sample.jpg"), make_textured_jpeg(80, 60, 4)).unwrap();

    let probe = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);

    let token = "0123456789abcdef0123456789abcdef";

    // 0. Non-loopback bind MUST fail closed even if public_origin is provided!
    let bad_lan = DesignerServerOptions {
        input_dir: in_dir.clone(),
        export_dir: export_dir.clone(),
        config_path: Some(root.join("gallery.toml")),
        bind_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        port,
        access_token: Some(token.to_string()),
        public_origin: Some(format!("https://studio.example:{port}")),
    };
    assert!(
        serve_gallery_designer(bad_lan, None).is_err(),
        "non-loopback bind must be refused to prevent plaintext remote listeners"
    );

    // 0b. Loopback + --public-origin WITHOUT explicit access_token MUST STILL enforce auth (401)!
    let probe2 = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port2 = probe2.local_addr().unwrap().port();
    drop(probe2);
    let stop2 = Arc::new(AtomicBool::new(false));
    let stop2_srv = Arc::clone(&stop2);
    let proxy_opts = DesignerServerOptions {
        input_dir: in_dir.clone(),
        export_dir: export_dir.clone(),
        config_path: Some(root.join("gallery_proxy.toml")),
        bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port: port2,
        access_token: None,
        public_origin: Some(format!("https://proxy.example:{port2}")),
    };
    let h2 = std::thread::spawn(move || {
        serve_gallery_designer(proxy_opts, Some(stop2_srv)).unwrap();
    });
    for _ in 0..80 {
        if TcpStream::connect(("127.0.0.1", port2)).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let (c_proxy_unauth, r_proxy_unauth) = http_roundtrip(
        port2,
        &format!("GET / HTTP/1.1\r\nHost: proxy.example:{port2}\r\nConnection: close\r\n\r\n"),
    );
    assert_eq!(c_proxy_unauth, 401, "proxy mode without --access-token must auto-enforce auth");
    assert!(!r_proxy_unauth.contains("data-csrf="), "unauthenticated proxy response must not leak CSRF");
    stop2.store(true, Ordering::SeqCst);
    h2.join().unwrap();

    let stop = Arc::new(AtomicBool::new(false));
    let stop_srv = Arc::clone(&stop);
    let opts = DesignerServerOptions {
        input_dir: in_dir,
        export_dir,
        config_path: Some(root.join("gallery.toml")),
        bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port,
        access_token: Some(token.to_string()),
        public_origin: Some(format!("https://studio.example:{port}")),
    };
    let handle = std::thread::spawn(move || {
        serve_gallery_designer(opts, Some(stop_srv)).unwrap();
    });

    for _ in 0..80 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }

    // 1. Unpinned Host header MUST be refused (403) even when auth is enabled.
    let (c_wrong_host, _) = http_roundtrip(
        port,
        &format!("GET /api/state HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"),
    );
    assert_eq!(c_wrong_host, 403, "unpinned Host must be rejected");

    // 2. Query-string ?token= credential MUST NOT authenticate (401).
    let (c_qtok, _) = http_roundtrip(
        port,
        &format!(
            "GET /?token={token} HTTP/1.1\r\nHost: studio.example:{port}\r\nConnection: close\r\n\r\n"
        ),
    );
    assert_eq!(c_qtok, 401, "reusable token in URL query string must NOT authenticate");

    // 3. POST /api/session with pinned https Origin sets HttpOnly; SameSite=Strict; Secure cookie!
    let sess_body = format!(r#"{{"token":"{token}"}}"#);
    let (c_login, r_login) = http_roundtrip(
        port,
        &format!(
            "POST /api/session HTTP/1.1\r\nHost: studio.example:{port}\r\nOrigin: https://studio.example:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{sess_body}",
            sess_body.len()
        ),
    );
    assert_eq!(c_login, 200, "POST /api/session failed: {r_login}");
    assert!(r_login.contains("HttpOnly; SameSite=Strict; Path=/; Secure"));
    assert!(r_login.contains(&format!("if_studio_token={token}")));

    // 4. GET /api/state with Authorization: Bearer <valid> and pinned Host MUST succeed with 200!
    let (c_bearer, _) = http_roundtrip(
        port,
        &format!(
            "GET /api/state HTTP/1.1\r\nHost: studio.example:{port}\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n"
        ),
    );
    assert_eq!(c_bearer, 200);

    stop.store(true, Ordering::SeqCst);
    handle.join().unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn bulk_refuses_symlinked_output_assets_dir_and_sanitizes_snippets() {
    let root = temp_test_dir("output_confinement");
    let in_dir = root.join("in");
    let out_dir = root.join("out");
    let escape_dir = root.join("escape");
    std::fs::create_dir_all(&in_dir).unwrap();
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::create_dir_all(&escape_dir).unwrap();
    std::fs::write(in_dir.join("photo.jpg"), make_textured_jpeg(96, 64, 5)).unwrap();

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&escape_dir, out_dir.join("assets")).unwrap();
        let mut preset = ResponsivePreset::web_responsive();
        preset.widths_px = vec![64];
        preset.formats = vec![OutputFormat::Jpeg];
        preset.quality = QualityTarget::CalibratedQuality { quality: 82.0 };
        let cfg = BulkConfig {
            schema: BULK_SCHEMA_VERSION.to_string(),
            inputs: vec![in_dir],
            output_dir: out_dir.clone(),
            recursive: true,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            follow_symlinks: false,
            output_layout: OutputPathLayout::ContentAddressed,
            concurrency: 1,
            incremental_cache: true,
            force_rebuild: false,
            on_error: FailurePolicy::FailFast,
            write_manifest: true,
            write_snippets: true,
            preset,
            limits: BulkLimits::default(),
        };
        let err = run_bulk(&cfg, None, None)
            .expect_err("pre-existing symlink at out/assets must be refused");
        assert!(err.contains("symlink"), "expected symlink refusal error, got: {err}");
        assert_eq!(
            std::fs::read_dir(&escape_dir).unwrap().count(),
            0,
            "zero files may be written into symlinked escape directory"
        );
        assert!(
            !out_dir.join("manifest.json").exists(),
            "FailFast must not publish manifest.json when an item fails"
        );
    }

    // Late destination-is-directory collision in publish_staged_files_atomic must leave earlier
    // files completely untouched and leave zero temp or backup artifacts behind!
    {
        let pub_root = root.join("atomic_dir_collision");
        std::fs::create_dir_all(&pub_root).unwrap();
        let first = pub_root.join("first.txt");
        let second_dir = pub_root.join("second.txt");
        std::fs::write(&first, b"gen1-intact").unwrap();
        std::fs::create_dir_all(&second_dir).unwrap();
        let err = imageflow_tool_lib::bulk::engine::publish_staged_files_atomic(
            &pub_root,
            &[
                (first.clone(), b"gen2-must-not-stick".to_vec()),
                (second_dir.clone(), b"collides-with-dir".to_vec()),
            ],
        )
        .expect_err("directory destination must fail atomic publish");
        assert!(err.contains("non-regular"), "expected non-regular refusal, got: {err}");
        assert_eq!(std::fs::read(&first).unwrap(), b"gen1-intact");
        let leftovers: Vec<_> = std::fs::read_dir(&pub_root)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "zero temp/backup files may remain: {leftovers:?}");
    }

    let _ = std::fs::remove_dir_all(&root);
}
