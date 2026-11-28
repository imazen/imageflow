//! Security-hardened local interactive gallery designer HTTP server (`imageflow_tool gallery serve`).
//!
//! Security & framing invariants:
//! - Uses `httparse` (maintained HTTP/1.1 parser) with 16 KiB header cap, 2 MiB body cap,
//!   15s socket deadlines, and bounded concurrency (max 8 active connections).
//! - Defaults to loopback (`127.0.0.1:3380`); non-loopback binds automatically enforce a
//!   128-bit session access token (`Authorization: Bearer` or `HttpOnly; SameSite=Strict` cookie)
//!   protecting BOTH private reads AND mutations.
//! - All `POST` mutations verify `Host`, exact `Origin`, and session-bound `X-Imageflow-Csrf`.
//! - Exposes opaque asset IDs (`a0001`, ...) only — never accepts raw filesystem paths from HTTP
//!   requests for reads or exports; source reads refuse symlinks and stay confined beneath the
//!   pre-resolved `input_dir`.
//! - Slider previews use an atomic generation counter so superseded encodes are cancelled before
//!   burning CPU and stale responses are discarded.

use super::{build_gallery, init_gallery_config, render, resolve_gallery_items};
use crate::bulk::engine::{
    DiscoveredInput, csprng_hex_128, discover_media_files, read_confined_regular_file,
    write_file_atomic,
};
use crate::bulk::planner::{escape_html, resolve_variant_coding};
use crate::bulk::schema::{
    BulkLimits, GalleryConfig, ManifestAsset, ManifestVariant, OutputFormat, QualityTarget,
    WorkspaceAdmission,
};
use imageflow_core::Context;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_CONCURRENT_CONNS: usize = 8;

#[derive(Debug, Clone)]
pub struct DesignerServerOptions {
    pub input_dir: PathBuf,
    pub export_dir: PathBuf,
    pub config_path: Option<PathBuf>,
    pub bind_ip: IpAddr,
    pub port: u16,
    pub access_token: Option<String>,
    /// Required `https://<host>[:<port>]` origin when binding non-loopback behind a TLS proxy.
    pub public_origin: Option<String>,
}

struct StudioState {
    input_root: PathBuf,
    export_dir: PathBuf,
    config_path: PathBuf,
    bind_addr: SocketAddr,
    require_auth: bool,
    secure_cookie: bool,
    pinned_origin: Option<String>,
    pinned_host: Option<String>,
    access_token: String,
    csrf_token: String,
    config: Mutex<GalleryConfig>,
    assets_by_id: BTreeMap<String, DiscoveredInput>,
    preview_assets: Mutex<Vec<ManifestAsset>>,
    thumb_cache: Mutex<BTreeMap<String, Vec<u8>>>,
    compare_gen: AtomicU64,
    active_conns: AtomicUsize,
}

#[derive(Debug, Deserialize)]
struct CompareRequest {
    asset_id: String,
    zensim_target: f32,
    #[serde(default = "default_preview_w")]
    preview_width: u32,
    #[serde(default)]
    formats: Vec<OutputFormat>,
}

fn default_preview_w() -> u32 {
    800
}

#[derive(Debug, Serialize)]
struct CompareFormatEntry {
    format: String,
    mime_type: String,
    bytes: usize,
    savings_pct: f64,
    requested_coding: String,
    effective_coding: String,
    fallback_reason: Option<String>,
    measured_zensim_a: Option<f64>,
    compare_status: String,
    preview_data_url: String,
}

fn random_hex_token_128() -> Result<String, String> {
    csprng_hex_128()
}

fn parse_pinned_origin(raw: &str, require_https: bool) -> Result<(String, String, bool), String> {
    let trimmed = raw.trim().trim_end_matches('/');
    let (scheme, host_port) = if let Some(rest) = trimmed.strip_prefix("https://") {
        ("https", rest)
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        if require_https {
            return Err("non-loopback --public-origin must use https:// (or bind 127.0.0.1 with SSH port forwarding)".into());
        }
        ("http", rest)
    } else {
        return Err(format!("invalid public_origin '{raw}' (expected https://<host>[:<port>])"));
    };
    if host_port.is_empty()
        || host_port.contains('/')
        || host_port.contains('?')
        || host_port.contains('#')
        || host_port.contains('@')
        || host_port.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(format!("invalid authority in public_origin '{raw}'"));
    }
    let normalized = format!("{scheme}://{host_port}");
    Ok((normalized, host_port.to_string(), scheme == "https"))
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    if ab.len() != bb.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in ab.iter().zip(bb.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn read_confined_source(root: &Path, path: &Path) -> Result<Vec<u8>, String> {
    read_confined_regular_file(root, path)
}

fn generate_preview_thumbnail(
    bytes: &[u8],
    is_video: bool,
    max_width: u32,
) -> Result<(Vec<u8>, u32, u32, u32, u32, String), String> {
    let mut ctx = Context::create().map_err(|e| format!("ctx failed: {e}"))?;
    ctx.message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#)
        .1
        .map_err(|e| format!("cfg failed: {e}"))?;
    ctx.add_copied_input_buffer(0, bytes).map_err(|e| format!("input failed: {e}"))?;
    let insp_req = if is_video {
        let container = if bytes.len() >= 4 && &bytes[0..4] == b"\x1A\x45\xDF\xA3" {
            "webm"
        } else if bytes.len() >= 4 && &bytes[0..4] == b"DKIF" {
            "ivf"
        } else {
            "mp4"
        };
        serde_json::json!({ "inputs": {"src": 0}, "input": "src", "video": {"container": container}, "scan": "headers" })
    } else {
        serde_json::json!({ "inputs": {"src": 0}, "input": "src", "scan": "headers" })
    };
    let (ir, ires) = ctx.message("v2/inspect", insp_req.to_string().as_bytes());
    ires.map_err(|e| format!("inspect failed: {e}"))?;
    let iv: serde_json::Value =
        serde_json::from_slice(&ir.response_json).map_err(|e| format!("inspect json: {e}"))?;
    let track = &iv["data"]["tracks"][0];
    let dims = track.get("display").or_else(|| track.get("coded")).unwrap_or(track);
    let orig_w = dims["width"].as_u64().unwrap_or(1) as u32;
    let orig_h = dims["height"].as_u64().unwrap_or(1) as u32;
    let kind = track["kind"].as_str().unwrap_or("image").to_string();

    let mut ectx = Context::create().map_err(|e| format!("ectx failed: {e}"))?;
    ectx.message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#)
        .1
        .map_err(|e| format!("ecfg failed: {e}"))?;
    ectx.add_copied_input_buffer(0, bytes).map_err(|e| format!("einput failed: {e}"))?;
    ectx.add_output_buffer(1).map_err(|e| format!("eout failed: {e}"))?;
    let decode_step = if kind == "video" || kind == "animation" {
        serde_json::json!({"decode": {"input": "src", "orientation": "apply", "select": {"presentation_index": {"index": 0}}}})
    } else {
        serde_json::json!({"decode": {"input": "src", "orientation": "apply"}})
    };
    let tw = orig_w.min(max_width).max(1);
    let exec_req = serde_json::json!({
        "inputs": {"src": 0},
        "outputs": {"thumb": 1},
        "pipeline": {
            "steps": [
                decode_step,
                {"geometry": {"constrain": {"mode": "within", "width": tw}}},
                {"encode": {"output": "thumb", "format": "jpeg", "coding": {"calibrated_quality": 82.0}, "effort": "fast"}}
            ]
        }
    });
    let (er, eres) = ectx.message("v2/execute", exec_req.to_string().as_bytes());
    eres.map_err(|e| format!("preview encode failed: {e}"))?;
    let ev: serde_json::Value = serde_json::from_slice(&er.response_json).unwrap_or_default();
    let out_w = ev["data"]["outputs"][0]["width"].as_u64().unwrap_or(u64::from(tw)) as u32;
    let out_h = ev["data"]["outputs"][0]["height"].as_u64().unwrap_or(u64::from(orig_h)) as u32;
    let jpeg = ectx.take_output_buffer(1).map_err(|e| format!("take thumb failed: {e}"))?;
    Ok((jpeg, orig_w, orig_h, out_w, out_h, kind))
}

fn build_preview_manifest_assets(
    input_root: &Path,
    discovered: &[DiscoveredInput],
) -> (Vec<ManifestAsset>, BTreeMap<String, Vec<u8>>) {
    let mut assets = Vec::new();
    let mut thumbs = BTreeMap::new();
    for d in discovered {
        let Ok(raw) = read_confined_source(input_root, &d.absolute_path) else {
            continue;
        };
        let Ok((jpeg, orig_w, orig_h, tw, th, kind)) =
            generate_preview_thumbnail(&raw, d.is_video_ext, 960)
        else {
            continue;
        };
        let url_rel = format!("/media/{}.jpg", d.id);
        let variant = ManifestVariant {
            relative_path: url_rel.clone(),
            format: OutputFormat::Jpeg,
            mime_type: "image/jpeg".to_string(),
            width: tw,
            height: th,
            bytes: jpeg.len() as u64,
            requested_coding: "preview".to_string(),
            effective_coding: "calibrated_quality(82.0)".to_string(),
            fallback_reason: None,
            verified_metric: None,
            verified_score: None,
        };
        let mut asset = ManifestAsset {
            id: d.id.clone(),
            source_relative_path: d.relative_key.clone(),
            source_sha256: String::new(),
            source_bytes: raw.len() as u64,
            media_kind: kind,
            source_format: d
                .absolute_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("img")
                .to_ascii_lowercase(),
            original_width: orig_w,
            original_height: orig_h,
            variants: vec![variant],
            video_variants: Vec::new(),
            picture_html: String::new(),
        };
        asset.picture_html =
            crate::bulk::planner::render_picture_snippet(&asset, "", "50vw", &d.relative_key, false);
        thumbs.insert(d.id.clone(), jpeg);
        assets.push(asset);
    }
    (assets, thumbs)
}

fn run_live_format_comparison(
    state: &StudioState,
    req: &CompareRequest,
    gen_ticket: u64,
) -> Result<serde_json::Value, String> {
    let item = state
        .assets_by_id
        .get(&req.asset_id)
        .ok_or_else(|| "unknown asset_id".to_string())?;
    let target = req.zensim_target.clamp(40.0, 98.0);
    let preview_w = req.preview_width.clamp(160, 1200);
    let formats = if req.formats.is_empty() {
        vec![OutputFormat::Avif, OutputFormat::Webp, OutputFormat::Jpeg]
    } else {
        req.formats.iter().copied().filter(|f| !f.is_video()).take(5).collect()
    };

    if state.compare_gen.load(Ordering::SeqCst) != gen_ticket {
        return Ok(serde_json::json!({ "stale": true, "results": [] }));
    }

    let raw = read_confined_source(&state.input_root, &item.absolute_path)?;
    // 1. Render ONE canonical resized PNG reference so every format is compared against the
    //    exact same resized reference frame.
    let mut ref_ctx = Context::create().map_err(|e| format!("ref ctx failed: {e}"))?;
    ref_ctx
        .message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#)
        .1
        .map_err(|e| format!("ref cfg failed: {e}"))?;
    ref_ctx.add_copied_input_buffer(0, &raw).map_err(|e| format!("ref in: {e}"))?;
    ref_ctx.add_output_buffer(1).map_err(|e| format!("ref out: {e}"))?;
    let dec_step = if item.is_video_ext {
        serde_json::json!({"decode": {"input": "src", "orientation": "apply", "select": {"presentation_index": {"index": 0}}}})
    } else {
        serde_json::json!({"decode": {"input": "src", "orientation": "apply"}})
    };
    let ref_env = serde_json::json!({
        "inputs": {"src": 0},
        "outputs": {"ref_png": 1},
        "pipeline": {
            "steps": [
                dec_step,
                {"geometry": {"constrain": {"mode": "within", "width": preview_w}}},
                {"encode": {"output": "ref_png", "format": "png", "coding": "lossless"}}
            ]
        }
    });
    ref_ctx
        .message("v2/execute", ref_env.to_string().as_bytes())
        .1
        .map_err(|e| format!("reference render failed: {e}"))?;
    let ref_png = ref_ctx.take_output_buffer(1).map_err(|e| format!("ref take: {e}"))?;

    let mut entries = Vec::with_capacity(formats.len());
    let qt = QualityTarget::VerifyZensimA { target };
    for fmt in formats {
        if state.compare_gen.load(Ordering::SeqCst) != gen_ticket {
            return Ok(serde_json::json!({ "stale": true, "results": [] }));
        }
        let resolved = resolve_variant_coding(
            fmt,
            &qt,
            crate::bulk::QualityFallbackPolicy::Calibrated,
            WorkspaceAdmission::BestEffort,
        )?;
        let mut ectx = Context::create().map_err(|e| format!("enc ctx: {e}"))?;
        ectx.message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#)
            .1
            .map_err(|e| format!("enc cfg: {e}"))?;
        ectx.add_copied_input_buffer(0, &ref_png).map_err(|e| format!("enc in: {e}"))?;
        ectx.add_output_buffer(1).map_err(|e| format!("enc out: {e}"))?;
        let env = serde_json::json!({
            "inputs": {"src": 0},
            "outputs": {"cand": 1},
            "pipeline": {
                "steps": [
                    {"decode": {"input": "src"}},
                    {"encode": {"output": "cand", "format": fmt.wire_format(), "coding": resolved.coding_json, "effort": "fast"}}
                ]
            }
        });
        let (er, eres) = ectx.message("v2/execute", env.to_string().as_bytes());
        if let Err(err) = eres {
            entries.push(CompareFormatEntry {
                format: fmt.to_string(),
                mime_type: fmt.mime_type().to_string(),
                bytes: 0,
                savings_pct: 0.0,
                requested_coding: resolved.requested_label,
                effective_coding: resolved.effective_label,
                fallback_reason: Some(format!("encode_failed: {err}")),
                measured_zensim_a: None,
                compare_status: "encode_error".to_string(),
                preview_data_url: String::new(),
            });
            continue;
        }
        let cand_bytes = ectx.take_output_buffer(1).map_err(|e| format!("cand take: {e}"))?;
        let er_val: serde_json::Value = serde_json::from_slice(&er.response_json).unwrap_or_default();
        let mut measured_score = er_val["data"]["outputs"][0]["verified_score_f64"].as_f64();
        let mut compare_status = if measured_score.is_some() {
            "verified".to_string()
        } else {
            "unavailable".to_string()
        };

        // Measure actual zensim_a via v2/compare (BestEffort) when encode didn't already verify it.
        if measured_score.is_none() {
            let mut cctx = Context::create().map_err(|e| format!("cmp ctx: {e}"))?;
            let _ = cctx.message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#);
            if cctx.add_copied_input_buffer(0, &ref_png).is_ok()
                && cctx.add_copied_input_buffer(1, &cand_bytes).is_ok()
            {
                let (cr, cres) = cctx.message(
                    "v2/compare",
                    br#"{"reference_io_id":0,"candidate_io_id":1,"metrics":["zensim_a"]}"#,
                );
                if cres.is_ok()
                    && let Ok(cv) = serde_json::from_slice::<serde_json::Value>(&cr.response_json)
                    && let Some(sc) = cv["data"]["results"][0]["score"].as_f64()
                {
                    measured_score = Some(sc);
                    compare_status = "measured".to_string();
                }
            }
        }

        let savings_pct = if raw.is_empty() {
            0.0
        } else {
            ((raw.len() as f64 - cand_bytes.len() as f64) / raw.len() as f64) * 100.0
        };
        let b64 = {
            use base64::Engine as _;
            base64::engine::general_purpose::STANDARD.encode(&cand_bytes)
        };
        entries.push(CompareFormatEntry {
            format: fmt.to_string(),
            mime_type: fmt.mime_type().to_string(),
            bytes: cand_bytes.len(),
            savings_pct,
            requested_coding: resolved.requested_label,
            effective_coding: resolved.effective_label,
            fallback_reason: resolved.fallback_reason,
            measured_zensim_a: measured_score,
            compare_status,
            preview_data_url: format!("data:{};base64,{b64}", fmt.mime_type()),
        });
    }

    Ok(serde_json::json!({
        "stale": false,
        "asset_id": req.asset_id,
        "original_bytes": raw.len(),
        "reference_png_bytes": ref_png.len(),
        "results": entries
    }))
}

/// Start the interactive gallery designer server until `stop` is triggered (or indefinitely if `None`).
pub fn serve_gallery_designer(
    opts: DesignerServerOptions,
    stop: Option<Arc<AtomicBool>>,
) -> Result<(), String> {
    let input_root = fs::canonicalize(&opts.input_dir)
        .map_err(|e| format!("cannot resolve input_dir '{}': {e}", opts.input_dir.display()))?;
    if !input_root.is_dir() {
        return Err(format!("input_dir '{}' is not a directory", input_root.display()));
    }
    fs::create_dir_all(&opts.export_dir)
        .map_err(|e| format!("cannot create export_dir '{}': {e}", opts.export_dir.display()))?;
    let export_dir = fs::canonicalize(&opts.export_dir)
        .map_err(|e| format!("cannot resolve export_dir '{}': {e}", opts.export_dir.display()))?;

    let config_path = opts
        .config_path
        .clone()
        .unwrap_or_else(|| input_root.join("gallery.toml"));
    let initial_cfg = if config_path.is_file() {
        GalleryConfig::load_file(&config_path)?
    } else {
        init_gallery_config(&input_root, None, "web-responsive")?
    };

    let discovered = discover_media_files(
        &[input_root.clone()],
        Some(&export_dir),
        true,
        &[],
        &[],
        false,
    )?;
    let (preview_assets, thumbs) = build_preview_manifest_assets(&input_root, &discovered);
    let mut assets_by_id = BTreeMap::new();
    for d in discovered {
        assets_by_id.insert(d.id.clone(), d);
    }

    if !opts.bind_ip.is_loopback() {
        return Err(
            "gallery designer only binds to loopback (127.0.0.1 / ::1) to avoid plaintext remote listeners; use SSH port forwarding (ssh -L 3380:127.0.0.1:3380 <host>) or place a local TLS reverse proxy in front of 127.0.0.1 with --public-origin https://<host>"
                .to_string(),
        );
    }
    let (pinned_origin, pinned_host, secure_cookie) = match opts.public_origin.as_deref() {
        Some(raw) => {
            let (orig, host, sec) = parse_pinned_origin(raw, true)?;
            (Some(orig), Some(host), sec)
        }
        None => (None, None, false),
    };

    let bind_addr = SocketAddr::new(opts.bind_ip, opts.port);
    let listener = TcpListener::bind(bind_addr)
        .map_err(|e| format!("failed binding designer server to {bind_addr}: {e}"))?;
    let local_addr = listener.local_addr().map_err(|e| format!("local_addr failed: {e}"))?;
    // ALWAYS require token authentication when exposed via a TLS reverse proxy (--public-origin)
    // or when an explicit access token is configured.
    let require_auth = opts.public_origin.is_some() || opts.access_token.is_some();
    let access_token = match opts.access_token {
        Some(t) => t,
        None => random_hex_token_128()?,
    };
    let csrf_token = random_hex_token_128()?;

    let state = Arc::new(StudioState {
        input_root,
        export_dir,
        config_path,
        bind_addr: local_addr,
        require_auth,
        secure_cookie,
        pinned_origin: pinned_origin.clone(),
        pinned_host,
        access_token: access_token.clone(),
        csrf_token,
        config: Mutex::new(initial_cfg),
        assets_by_id,
        preview_assets: Mutex::new(preview_assets),
        thumb_cache: Mutex::new(thumbs),
        compare_gen: AtomicU64::new(0),
        active_conns: AtomicUsize::new(0),
    });

    if require_auth {
        eprintln!(
            "Imageflow Gallery Designer listening on {local_addr} (token auth enabled; send Authorization: Bearer <token> or POST /api/session)"
        );
        if let Some(ref po) = pinned_origin {
            println!("{po}/");
        } else {
            println!("http://127.0.0.1:{}/", local_addr.port());
        }
        eprintln!("Session token: {access_token}");
    } else {
        eprintln!("Imageflow Gallery Designer listening on {local_addr} (loopback only)");
        println!("http://127.0.0.1:{}/", local_addr.port());
    }

    if stop.is_some() {
        let _ = listener.set_nonblocking(true);
    }

    loop {
        if let Some(ref s) = stop
            && s.load(Ordering::Relaxed)
        {
            break;
        }
        match listener.accept() {
            Ok((stream, _peer)) => {
                if state.active_conns.fetch_add(1, Ordering::SeqCst) >= MAX_CONCURRENT_CONNS {
                    state.active_conns.fetch_sub(1, Ordering::SeqCst);
                    let mut s = stream;
                    let _ = send_response(
                        &mut s,
                        503,
                        "Service Unavailable",
                        "text/plain; charset=utf-8",
                        b"Server busy",
                        &[],
                    );
                    continue;
                }
                let st = Arc::clone(&state);
                std::thread::spawn(move || {
                    let _ = handle_connection(stream, &st);
                    st.active_conns.fetch_sub(1, Ordering::SeqCst);
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => {
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    }
    Ok(())
}

struct ParsedHttpRequest {
    method: String,
    path: String,
    query: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

fn read_http_request(stream: &mut TcpStream) -> Result<ParsedHttpRequest, (u16, &'static str)> {
    let deadline = Instant::now() + Duration::from_secs(15);
    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
    let mut buf = Vec::with_capacity(4096);
    let mut tmp = [0u8; 4096];
    let header_end;
    loop {
        let Some(rem) = deadline.checked_duration_since(Instant::now()) else {
            return Err((408, "Request Timeout"));
        };
        if rem.is_zero() {
            return Err((408, "Request Timeout"));
        }
        let _ = stream.set_read_timeout(Some(rem));
        let n = stream.read(&mut tmp).map_err(|_| (408, "Request Timeout"))?;
        if n == 0 {
            return Err((400, "Bad Request"));
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = pos + 4;
            if header_end > MAX_HEADER_BYTES {
                return Err((431, "Request Header Fields Too Large"));
            }
            break;
        }
        if buf.len() > MAX_HEADER_BYTES {
            return Err((431, "Request Header Fields Too Large"));
        }
    }

    let mut raw_headers = [httparse::EMPTY_HEADER; 64];
    let mut req = httparse::Request::new(&mut raw_headers);
    let status = req.parse(&buf[..header_end]).map_err(|_| (400, "Bad Request"))?;
    if !status.is_complete() {
        return Err((400, "Bad Request"));
    }
    let method = req.method.ok_or((400, "Bad Request"))?.to_string();
    let full_target = req.path.ok_or((400, "Bad Request"))?;
    let (path, query) = full_target
        .split_once('?')
        .map(|(p, q)| (p.to_string(), q.to_string()))
        .unwrap_or_else(|| (full_target.to_string(), String::new()));

    const SINGLETON_HEADERS: &[&str] = &[
        "host",
        "content-length",
        "origin",
        "authorization",
        "x-imageflow-csrf",
        "transfer-encoding",
        "cookie",
    ];
    let mut headers = BTreeMap::new();
    for h in req.headers {
        let name = h.name.to_ascii_lowercase();
        let val = std::str::from_utf8(h.value).map_err(|_| (400, "Bad Request"))?.trim();
        if SINGLETON_HEADERS.contains(&name.as_str()) && headers.contains_key(&name) {
            return Err((400, "Bad Request"));
        }
        headers.insert(name, val.to_string());
    }
    if headers.contains_key("transfer-encoding") {
        return Err((501, "Not Implemented"));
    }
    let content_len: usize = match headers.get("content-length") {
        None => 0,
        Some(s) => {
            if !s.bytes().all(|b| b.is_ascii_digit()) {
                return Err((400, "Bad Request"));
            }
            s.parse().map_err(|_| (400, "Bad Request"))?
        }
    };
    if content_len > MAX_BODY_BYTES {
        return Err((413, "Payload Too Large"));
    }
    let mut body = buf[header_end..].to_vec();
    while body.len() < content_len {
        let Some(rem) = deadline.checked_duration_since(Instant::now()) else {
            return Err((408, "Request Timeout"));
        };
        if rem.is_zero() {
            return Err((408, "Request Timeout"));
        }
        let _ = stream.set_read_timeout(Some(rem));
        let need = (content_len - body.len()).min(tmp.len());
        let n = stream.read(&mut tmp[..need]).map_err(|_| (408, "Request Timeout"))?;
        if n == 0 {
            return Err((400, "Bad Request"));
        }
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(content_len);

    Ok(ParsedHttpRequest { method, path, query, headers, body })
}

fn send_response(
    stream: &mut TcpStream,
    code: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
    extra_headers: &[(&str, &str)],
) -> std::io::Result<()> {
    let mut head = format!(
        "HTTP/1.1 {code} {reason}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Referrer-Policy: no-referrer\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n",
        body.len()
    );
    for (k, v) in extra_headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn check_host_header(req: &ParsedHttpRequest, state: &StudioState) -> bool {
    let Some(host) = req.headers.get("host") else {
        return false;
    };
    if let Some(ref pinned) = state.pinned_host {
        return host.eq_ignore_ascii_case(pinned);
    }
    let port = state.bind_addr.port();
    let allowed = [
        format!("127.0.0.1:{port}"),
        format!("localhost:{port}"),
        format!("[::1]:{port}"),
    ];
    allowed.iter().any(|a| host.eq_ignore_ascii_case(a))
}

fn check_pinned_origin(req: &ParsedHttpRequest, state: &StudioState) -> bool {
    let Some(host) = req.headers.get("host") else {
        return false;
    };
    let Some(origin) = req.headers.get("origin") else {
        return false;
    };
    if let Some(ref pinned) = state.pinned_origin {
        origin.eq_ignore_ascii_case(pinned)
    } else {
        let expected_http = format!("http://{host}");
        origin.eq_ignore_ascii_case(&expected_http)
    }
}

fn check_auth(req: &ParsedHttpRequest, state: &StudioState) -> bool {
    if !state.require_auth {
        return true;
    }
    if let Some(auth) = req.headers.get("authorization")
        && let Some(tok) = auth.strip_prefix("Bearer ")
        && constant_time_eq(tok.trim(), &state.access_token)
    {
        return true;
    }
    if let Some(cookie) = req.headers.get("cookie") {
        for part in cookie.split(';') {
            if let Some((k, v)) = part.trim().split_once('=')
                && k == "if_studio_token"
                && constant_time_eq(v, &state.access_token)
            {
                return true;
            }
        }
    }
    false
}

fn check_mutation_csrf(req: &ParsedHttpRequest, state: &StudioState) -> bool {
    if !check_pinned_origin(req, state) {
        return false;
    }
    req.headers
        .get("x-imageflow-csrf")
        .is_some_and(|tok| constant_time_eq(tok, &state.csrf_token))
}

fn handle_connection(mut stream: TcpStream, state: &StudioState) -> std::io::Result<()> {
    let req = match read_http_request(&mut stream) {
        Ok(r) => r,
        Err((code, reason)) => {
            return send_response(&mut stream, code, reason, "text/plain; charset=utf-8", reason.as_bytes(), &[]);
        }
    };
    let _ = &req.query;

    if !check_host_header(&req, state) {
        return send_response(&mut stream, 403, "Forbidden", "text/plain; charset=utf-8", b"Invalid Host header", &[]);
    }

    // Explicit POST /api/session token exchange (never accepts reusable tokens in URL query strings).
    if state.require_auth && req.method == "POST" && req.path == "/api/session" {
        if !check_pinned_origin(&req, state) {
            return send_response(
                &mut stream,
                403,
                "Forbidden",
                "application/json",
                br#"{"error":"invalid origin"}"#,
                &[],
            );
        }
        #[derive(Deserialize)]
        struct SessionLogin {
            token: String,
        }
        let Ok(login) = serde_json::from_slice::<SessionLogin>(&req.body) else {
            return send_response(
                &mut stream,
                400,
                "Bad Request",
                "application/json",
                br#"{"error":"expected JSON {\"token\":\"...\"}"}"#,
                &[],
            );
        };
        if !constant_time_eq(login.token.trim(), &state.access_token) {
            return send_response(
                &mut stream,
                401,
                "Unauthorized",
                "application/json",
                br#"{"error":"invalid session token"}"#,
                &[],
            );
        }
        let secure_attr = if state.secure_cookie { "; Secure" } else { "" };
        let cookie = format!(
            "if_studio_token={}; HttpOnly; SameSite=Strict; Path=/{secure_attr}",
            state.access_token
        );
        return send_response(
            &mut stream,
            200,
            "OK",
            "application/json",
            br#"{"ok":true}"#,
            &[("Set-Cookie", &cookie)],
        );
    }

    if !check_auth(&req, state) {
        return send_response(
            &mut stream,
            401,
            "Unauthorized",
            "text/plain; charset=utf-8",
            b"Access token required (send Authorization: Bearer <token> or POST /api/session)",
            &[],
        );
    }

    let csp = "default-src 'self'; img-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; frame-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'self'";

    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/") => {
            let html = render_studio_shell_html(&state.csrf_token);
            send_response(
                &mut stream,
                200,
                "OK",
                "text/html; charset=utf-8",
                html.as_bytes(),
                &[("Content-Security-Policy", csp)],
            )
        }
        ("GET", "/preview/gallery") => {
            let cfg = state.config.lock().unwrap().clone();
            let assets = state.preview_assets.lock().unwrap().clone();
            let (items, _) = resolve_gallery_items(&cfg, &assets);
            let html = render::render_gallery_html(&cfg, &items, true);
            send_response(
                &mut stream,
                200,
                "OK",
                "text/html; charset=utf-8",
                html.as_bytes(),
                &[("Content-Security-Policy", csp)],
            )
        }
        ("GET", p) if p.starts_with("/media/") && p.ends_with(".jpg") => {
            let id = &p["/media/".len()..p.len() - 4];
            let maybe = state.thumb_cache.lock().unwrap().get(id).cloned();
            match maybe {
                Some(bytes) => send_response(&mut stream, 200, "OK", "image/jpeg", &bytes, &[]),
                None => send_response(&mut stream, 404, "Not Found", "text/plain; charset=utf-8", b"Not Found", &[]),
            }
        }
        ("GET", "/api/state") => {
            let cfg = state.config.lock().unwrap().clone();
            let assets = state.preview_assets.lock().unwrap().clone();
            let (items, hidden) = resolve_gallery_items(&cfg, &assets);
            let payload = serde_json::json!({
                "config": cfg,
                "export_dir_display": state.export_dir.display().to_string(),
                "config_path_display": state.config_path.display().to_string(),
                "visible_count": items.len(),
                "hidden_count": hidden,
                "assets": assets.iter().map(|a| serde_json::json!({
                    "id": a.id,
                    "source_relative_path": a.source_relative_path,
                    "original_width": a.original_width,
                    "original_height": a.original_height,
                    "source_bytes": a.source_bytes,
                    "media_kind": a.media_kind,
                    "thumb_url": format!("/media/{}.jpg", a.id)
                })).collect::<Vec<_>>()
            });
            let body = serde_json::to_vec(&payload).unwrap_or_default();
            send_response(&mut stream, 200, "OK", "application/json", &body, &[])
        }
        ("POST", "/api/config") => {
            if !check_mutation_csrf(&req, state) {
                return send_response(&mut stream, 403, "Forbidden", "application/json", br#"{"error":"invalid origin or csrf"}"#, &[]);
            }
            match serde_json::from_slice::<GalleryConfig>(&req.body) {
                Ok(cfg) => match cfg.validate() {
                    Ok(()) => {
                        *state.config.lock().unwrap() = cfg.clone();
                        let body = serde_json::to_vec(&serde_json::json!({"ok": true, "config": cfg})).unwrap_or_default();
                        send_response(&mut stream, 200, "OK", "application/json", &body, &[])
                    }
                    Err(e) => {
                        let body = serde_json::to_vec(&serde_json::json!({"error": e})).unwrap_or_default();
                        send_response(&mut stream, 400, "Bad Request", "application/json", &body, &[])
                    }
                },
                Err(e) => {
                    let body = serde_json::to_vec(&serde_json::json!({"error": format!("invalid config JSON: {e}")})).unwrap_or_default();
                    send_response(&mut stream, 400, "Bad Request", "application/json", &body, &[])
                }
            }
        }
        ("POST", "/api/save-config") => {
            if !check_mutation_csrf(&req, state) {
                return send_response(&mut stream, 403, "Forbidden", "application/json", br#"{"error":"invalid origin or csrf"}"#, &[]);
            }
            let cfg = state.config.lock().unwrap().clone();
            let is_json = state
                .config_path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("json"));
            let serialized = if is_json {
                cfg.to_json_string()
            } else {
                cfg.to_toml_string()
            };
            match serialized.and_then(|s| write_file_atomic(&state.config_path, s.as_bytes())) {
                Ok(()) => {
                    let body = serde_json::to_vec(&serde_json::json!({"ok": true, "path": state.config_path.display().to_string()})).unwrap_or_default();
                    send_response(&mut stream, 200, "OK", "application/json", &body, &[])
                }
                Err(e) => {
                    let body = serde_json::to_vec(&serde_json::json!({"error": e})).unwrap_or_default();
                    send_response(&mut stream, 500, "Internal Server Error", "application/json", &body, &[])
                }
            }
        }
        ("POST", "/api/compare") => {
            if !check_mutation_csrf(&req, state) {
                return send_response(&mut stream, 403, "Forbidden", "application/json", br#"{"error":"invalid origin or csrf"}"#, &[]);
            }
            let cmp_req: CompareRequest = match serde_json::from_slice(&req.body) {
                Ok(v) => v,
                Err(e) => {
                    let body = serde_json::to_vec(&serde_json::json!({"error": format!("invalid compare payload: {e}")})).unwrap_or_default();
                    return send_response(&mut stream, 400, "Bad Request", "application/json", &body, &[]);
                }
            };
            let ticket = state.compare_gen.fetch_add(1, Ordering::SeqCst) + 1;
            match run_live_format_comparison(state, &cmp_req, ticket) {
                Ok(val) => {
                    let body = serde_json::to_vec(&val).unwrap_or_default();
                    send_response(&mut stream, 200, "OK", "application/json", &body, &[])
                }
                Err(e) => {
                    let body = serde_json::to_vec(&serde_json::json!({"error": e})).unwrap_or_default();
                    send_response(&mut stream, 400, "Bad Request", "application/json", &body, &[])
                }
            }
        }
        ("POST", "/api/export") => {
            if !check_mutation_csrf(&req, state) {
                return send_response(&mut stream, 403, "Forbidden", "application/json", br#"{"error":"invalid origin or csrf"}"#, &[]);
            }
            let cfg = state.config.lock().unwrap().clone();
            match build_gallery(
                &state.input_root,
                &state.export_dir,
                &cfg,
                2,
                false,
                BulkLimits::default(),
                None,
                None,
            ) {
                Ok(rep) => {
                    let ok = rep.bulk_report.failed == 0;
                    let body = serde_json::to_vec(&serde_json::json!({
                        "ok": ok,
                        "index_html": rep.index_html_path.display().to_string(),
                        "gallery_json": rep.gallery_json_path.display().to_string(),
                        "visible_items": rep.visible_items,
                        "processed": rep.bulk_report.processed,
                        "skipped_cached": rep.bulk_report.skipped_cached,
                        "failed": rep.bulk_report.failed,
                        "errors": rep.bulk_report.errors,
                        "total_input_bytes": rep.bulk_report.total_input_bytes,
                        "total_output_bytes": rep.bulk_report.total_output_bytes,
                        "elapsed_ms": rep.bulk_report.elapsed_ms
                    }))
                    .unwrap_or_default();
                    if ok {
                        send_response(&mut stream, 200, "OK", "application/json", &body, &[])
                    } else {
                        send_response(&mut stream, 422, "Unprocessable Entity", "application/json", &body, &[])
                    }
                }
                Err(e) => {
                    let body = serde_json::to_vec(&serde_json::json!({"ok": false, "error": e})).unwrap_or_default();
                    send_response(&mut stream, 422, "Unprocessable Entity", "application/json", &body, &[])
                }
            }
        }
        _ => send_response(&mut stream, 404, "Not Found", "text/plain; charset=utf-8", b"Not Found", &[]),
    }
}

fn render_studio_shell_html(csrf_token: &str) -> String {
    let csrf_esc = escape_html(csrf_token);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Imageflow Gallery Designer</title>
<style>
* {{ box-sizing: border-box; }}
body {{ margin: 0; font-family: Inter, ui-sans-serif, system-ui, sans-serif; background: #0d1117; color: #e6edf3; height: 100vh; display: flex; flex-direction: column; overflow: hidden; }}
header {{ height: 54px; background: #161b22; border-bottom: 1px solid #30363d; display: flex; align-items: center; justify-content: space-between; padding: 0 16px; gap: 12px; flex-shrink: 0; }}
.brand {{ font-weight: 700; font-size: 0.98rem; letter-spacing: -0.01em; display: flex; align-items: center; gap: 8px; }}
.pill {{ background: #1f6feb33; color: #79c0ff; border: 1px solid #388bfd55; font-size: 0.72rem; padding: 2px 8px; border-radius: 999px; }}
.top-actions {{ display: flex; gap: 8px; align-items: center; }}
button {{ background: #21262d; color: #e6edf3; border: 1px solid #30363d; border-radius: 7px; padding: 7px 12px; cursor: pointer; font-size: 0.84rem; font-weight: 500; }}
button:hover {{ border-color: #58a6ff; }}
button.primary {{ background: #238636; border-color: #2ea043; color: #fff; }}
button.primary:hover {{ background: #2ea043; }}
.workspace {{ flex: 1; display: grid; grid-template-columns: 360px 1fr; min-height: 0; }}
aside {{ background: #161b22; border-right: 1px solid #30363d; display: flex; flex-direction: column; min-height: 0; }}
.tabs {{ display: grid; grid-template-columns: repeat(3, 1fr); border-bottom: 1px solid #30363d; }}
.tab-btn {{ border: none; border-radius: 0; background: transparent; padding: 11px 6px; font-size: 0.82rem; color: #8b949e; border-bottom: 2px solid transparent; }}
.tab-btn.active {{ color: #e6edf3; border-bottom-color: #58a6ff; background: #0d111755; }}
.panel {{ flex: 1; overflow-y: auto; padding: 14px; display: none; flex-direction: column; gap: 14px; }}
.panel.active {{ display: flex; }}
label {{ display: flex; flex-direction: column; gap: 5px; font-size: 0.8rem; color: #8b949e; }}
input[type=text], input[type=number], select, textarea {{ background: #0d1117; color: #e6edf3; border: 1px solid #30363d; border-radius: 6px; padding: 7px 9px; font-size: 0.84rem; font-family: inherit; }}
input[type=range] {{ width: 100%; accent-color: #58a6ff; }}
.row2 {{ display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }}
.check {{ flex-direction: row; align-items: center; gap: 8px; color: #e6edf3; }}
.chips {{ display: flex; flex-wrap: wrap; gap: 6px; }}
.chip {{ padding: 4px 9px; font-size: 0.76rem; border-radius: 999px; }}
.chip.on {{ background: #1f6feb; border-color: #388bfd; color: #fff; }}
.card-box {{ background: #0d1117; border: 1px solid #30363d; border-radius: 8px; padding: 10px; display: flex; flex-direction: column; gap: 8px; }}
.photo-row {{ display: flex; align-items: center; gap: 8px; padding: 6px; border: 1px solid #30363d; border-radius: 7px; background: #0d1117; cursor: pointer; }}
.photo-row.sel {{ border-color: #58a6ff; background: #1f6feb22; }}
.photo-row img {{ width: 44px; height: 44px; object-fit: cover; border-radius: 4px; flex-shrink: 0; }}
.photo-info {{ min-width: 0; flex: 1; }}
.photo-name {{ font-size: 0.82rem; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }}
.photo-dim {{ font-size: 0.72rem; color: #8b949e; }}
.cmp-grid {{ display: grid; gap: 8px; }}
.cmp-item {{ border: 1px solid #30363d; border-radius: 7px; padding: 8px; background: #161b22; }}
.cmp-item img {{ width: 100%; max-height: 150px; object-fit: cover; border-radius: 5px; margin-top: 6px; }}
.status-bar {{ font-size: 0.78rem; color: #8b949e; }}
iframe {{ width: 100%; height: 100%; border: none; background: #0b0c10; }}
</style>
</head>
<body data-csrf="{csrf_esc}">
<header>
  <div class="brand">Imageflow Gallery Designer <span class="pill">zensim-a</span></div>
  <div id="status" class="status-bar">Loading gallery…</div>
  <div class="top-actions">
    <button id="btn-save" type="button">Save gallery.toml</button>
    <button id="btn-export" class="primary" type="button">Export Static Gallery</button>
  </div>
</header>
<div class="workspace">
  <aside>
    <div class="tabs">
      <button class="tab-btn active" data-tab="design" type="button">Layout &amp; Theme</button>
      <button class="tab-btn" data-tab="quality" type="button">Quality &amp; Formats</button>
      <button class="tab-btn" data-tab="photos" type="button">Photos &amp; Captions</button>
    </div>

    <div id="panel-design" class="panel active">
      <label>Gallery Title <input id="f-title" type="text"></label>
      <label>Subtitle <input id="f-subtitle" type="text"></label>
      <label>Author <input id="f-author" type="text"></label>
      <div class="row2">
        <label>Layout
          <select id="f-layout">
            <option value="justified">Justified Rows</option>
            <option value="masonry">Masonry</option>
            <option value="grid">Uniform Grid</option>
            <option value="editorial">Editorial Essay</option>
            <option value="filmstrip">Filmstrip</option>
          </select>
        </label>
        <label>Theme
          <select id="f-theme">
            <option value="gallery_dark">Gallery Dark</option>
            <option value="paper_light">Paper Light</option>
            <option value="minimal_white">Minimal White</option>
            <option value="warm_linen">Warm Linen</option>
            <option value="midnight_slate">Midnight Slate</option>
          </select>
        </label>
      </div>
      <div class="row2">
        <label>Font
          <select id="f-font">
            <option value="sans">Modern Sans</option>
            <option value="serif">Editorial Serif</option>
            <option value="mono">Technical Mono</option>
          </select>
        </label>
        <label>Captions
          <select id="f-capstyle">
            <option value="below">Below Card</option>
            <option value="hover_overlay">Hover Overlay</option>
            <option value="lightbox_only">Lightbox Only</option>
            <option value="hidden">Hidden</option>
          </select>
        </label>
      </div>
      <label>Row Height (<span id="v-rowh">280</span>px) <input id="f-rowh" type="range" min="140" max="520" step="10"></label>
      <label>Desktop Columns (<span id="v-cols">3</span>) <input id="f-cols" type="range" min="1" max="6" step="1"></label>
      <label>Gap (<span id="v-gap">14</span>px) <input id="f-gap" type="range" min="0" max="40" step="2"></label>
      <label>Corner Radius (<span id="v-rad">8</span>px) <input id="f-rad" type="range" min="0" max="28" step="1"></label>
      <label>Max Width (<span id="v-maxw">1440</span>px) <input id="f-maxw" type="range" min="720" max="2400" step="40"></label>
      <label class="check"><input id="f-exif" type="checkbox"> Show dimension / savings / zensim badges</label>
      <label class="check"><input id="f-lb" type="checkbox"> Enable accessible keyboard lightbox</label>
      <label class="check"><input id="f-dl" type="checkbox"> Show download button in lightbox</label>
    </div>

    <div id="panel-quality" class="panel">
      <div class="card-box">
        <label>Perceptual Quality Dial — zensim_a (<span id="v-zensim">90</span>)
          <input id="f-zensim" type="range" min="60" max="98" step="1">
        </label>
        <div class="chips">
          <button class="chip" data-z="76" type="button">Compact 76</button>
          <button class="chip" data-z="85" type="button">Web Crisp 85</button>
          <button class="chip" data-z="90" type="button">Gallery HQ 90</button>
          <button class="chip" data-z="95" type="button">Archival 95</button>
        </div>
      </div>
      <label>Responsive Widths (comma-separated px) <input id="f-widths" type="text" placeholder="480, 960, 1600, 2400"></label>
      <div class="card-box">
        <div style="font-size:0.8rem;color:#8b949e">Output Formats</div>
        <label class="check"><input class="fmt-cb" value="avif" type="checkbox"> AVIF (smallest modern web)</label>
        <label class="check"><input class="fmt-cb" value="webp" type="checkbox"> WebP (broad modern fallback)</label>
        <label class="check"><input class="fmt-cb" value="jpeg" type="checkbox"> JPEG (universal fallback, verified zensim_a)</label>
        <label class="check"><input class="fmt-cb" value="png" type="checkbox"> PNG (lossless)</label>
      </div>
      <div class="card-box">
        <div style="display:flex;justify-content:space-between;align-items:center">
          <span style="font-size:0.82rem;font-weight:600">Live Format Comparison</span>
          <button id="btn-compare" type="button">Measure Selected Photo</button>
        </div>
        <div id="cmp-out" class="cmp-grid"></div>
      </div>
    </div>

    <div id="panel-photos" class="panel">
      <div id="photo-editor" class="card-box" style="display:none">
        <div id="pe-file" style="font-size:0.76rem;color:#58a6ff"></div>
        <label>Photo Title <input id="pe-title" type="text"></label>
        <label>Caption <textarea id="pe-caption" rows="2"></textarea></label>
        <label>Alt Text <input id="pe-alt" type="text"></label>
        <div class="row2">
          <label class="check"><input id="pe-feat" type="checkbox"> Featured / Hero</label>
          <label class="check"><input id="pe-hide" type="checkbox"> Hide from gallery</label>
        </div>
      </div>
      <div id="photo-list" style="display:flex;flex-direction:column;gap:6px"></div>
    </div>
  </aside>
  <main style="min-height:0">
    <iframe id="preview-frame" src="/preview/gallery" title="Live gallery preview"></iframe>
  </main>
</div>
<script>
(() => {{
  const csrf = document.body.dataset.csrf;
  let state = null;
  let selectedAssetId = null;
  let syncTimer = null;

  const $ = (id) => document.getElementById(id);
  const statusEl = $('status');
  const frame = $('preview-frame');

  document.querySelectorAll('.tab-btn').forEach((btn) => {{
    btn.addEventListener('click', () => {{
      document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
      document.querySelectorAll('.panel').forEach(p => p.classList.remove('active'));
      btn.classList.add('active');
      $('panel-' + btn.dataset.tab).classList.add('active');
    }});
  }});

  async function api(path, body) {{
    const opts = body === undefined ? {{ method: 'GET' }} : {{
      method: 'POST',
      headers: {{ 'Content-Type': 'application/json', 'X-Imageflow-Csrf': csrf }},
      body: JSON.stringify(body)
    }};
    const res = await fetch(path, opts);
    const data = await res.json();
    if (!res.ok) throw new Error(data.error || ('HTTP ' + res.status));
    return data;
  }}

  function populateForm() {{
    const c = state.config;
    $('f-title').value = c.title || '';
    $('f-subtitle').value = c.subtitle || '';
    $('f-author').value = c.author || '';
    $('f-layout').value = c.design.layout;
    $('f-theme').value = c.design.theme;
    $('f-font').value = c.design.font_family;
    $('f-capstyle').value = c.design.caption_style;
    $('f-rowh').value = c.design.target_row_height_px;
    $('v-rowh').textContent = c.design.target_row_height_px;
    $('f-cols').value = c.design.columns_desktop;
    $('v-cols').textContent = c.design.columns_desktop;
    $('f-gap').value = c.design.gap_px;
    $('v-gap').textContent = c.design.gap_px;
    $('f-rad').value = c.design.corner_radius_px;
    $('v-rad').textContent = c.design.corner_radius_px;
    $('f-maxw').value = c.design.max_width_px;
    $('v-maxw').textContent = c.design.max_width_px;
    $('f-exif').checked = !!c.design.show_exif_badges;
    $('f-lb').checked = !!c.design.enable_lightbox;
    $('f-dl').checked = !!c.design.show_download_button;
    const z = c.encoding.quality && (c.encoding.quality.target || c.encoding.quality.quality) || 90;
    $('f-zensim').value = Math.round(z);
    $('v-zensim').textContent = Math.round(z);
    $('f-widths').value = (c.encoding.widths_px || []).join(', ');
    document.querySelectorAll('.fmt-cb').forEach((cb) => {{
      cb.checked = (c.encoding.formats || []).includes(cb.value);
    }});
    renderPhotoList();
  }}

  function renderPhotoList() {{
    const list = $('photo-list');
    list.replaceChildren();
    for (const a of state.assets) {{
      const ov = (state.config.items && state.config.items[a.source_relative_path]) || {{}};
      const row = document.createElement('div');
      row.className = 'photo-row' + (a.id === selectedAssetId ? ' sel' : '');
      const img = document.createElement('img');
      img.src = a.thumb_url;
      img.alt = a.source_relative_path;
      const info = document.createElement('div');
      info.className = 'photo-info';
      const nm = document.createElement('div');
      nm.className = 'photo-name';
      nm.textContent = (ov.hidden ? '[hidden] ' : '') + (ov.title || a.source_relative_path);
      const dm = document.createElement('div');
      dm.className = 'photo-dim';
      dm.textContent = a.original_width + '×' + a.original_height + ' · ' + (a.source_bytes / 1024).toFixed(1) + ' KB';
      info.append(nm, dm);
      row.append(img, info);
      row.addEventListener('click', () => selectPhoto(a));
      list.appendChild(row);
    }}
  }}

  function selectPhoto(a) {{
    selectedAssetId = a.id;
    const ov = (state.config.items[a.source_relative_path] ||= {{ hidden: false, featured: false }});
    $('photo-editor').style.display = 'flex';
    $('pe-file').textContent = a.source_relative_path;
    $('pe-title').value = ov.title || '';
    $('pe-caption').value = ov.caption || '';
    $('pe-alt').value = ov.alt_text || '';
    $('pe-feat').checked = !!ov.featured;
    $('pe-hide').checked = !!ov.hidden;
    renderPhotoList();
  }}

  function collectAndSync(reloadIframe) {{
    const c = state.config;
    c.title = $('f-title').value;
    c.subtitle = $('f-subtitle').value || null;
    c.author = $('f-author').value || null;
    c.design.layout = $('f-layout').value;
    c.design.theme = $('f-theme').value;
    c.design.font_family = $('f-font').value;
    c.design.caption_style = $('f-capstyle').value;
    c.design.target_row_height_px = Number($('f-rowh').value);
    c.design.columns_desktop = Number($('f-cols').value);
    c.design.gap_px = Number($('f-gap').value);
    c.design.corner_radius_px = Number($('f-rad').value);
    c.design.max_width_px = Number($('f-maxw').value);
    c.design.show_exif_badges = $('f-exif').checked;
    c.design.enable_lightbox = $('f-lb').checked;
    c.design.show_download_button = $('f-dl').checked;
    $('v-rowh').textContent = c.design.target_row_height_px;
    $('v-cols').textContent = c.design.columns_desktop;
    $('v-gap').textContent = c.design.gap_px;
    $('v-rad').textContent = c.design.corner_radius_px;
    $('v-maxw').textContent = c.design.max_width_px;

    const z = Number($('f-zensim').value);
    $('v-zensim').textContent = z;
    c.encoding.quality = {{ mode: 'verify_zensim_a', target: z }};
    const parsedWidths = $('f-widths').value.split(',').map(s => Number(s.trim())).filter(n => Number.isFinite(n) && n >= 16 && n <= 16384);
    if (parsedWidths.length) c.encoding.widths_px = parsedWidths;
    const fmts = Array.from(document.querySelectorAll('.fmt-cb')).filter(cb => cb.checked).map(cb => cb.value);
    if (fmts.length) c.encoding.formats = fmts;

    clearTimeout(syncTimer);
    syncTimer = setTimeout(async () => {{
      try {{
        await api('/api/config', c);
        if (reloadIframe) frame.src = '/preview/gallery?v=' + Date.now();
        statusEl.textContent = 'Live preview updated (' + state.assets.length + ' photos)';
      }} catch (err) {{
        statusEl.textContent = 'Config error: ' + err.message;
      }}
    }}, 120);
  }}

  ['f-title','f-subtitle','f-author','f-layout','f-theme','f-font','f-capstyle','f-rowh','f-cols','f-gap','f-rad','f-maxw','f-exif','f-lb','f-dl','f-zensim','f-widths'].forEach((id) => {{
    $(id).addEventListener('input', () => collectAndSync(true));
    $(id).addEventListener('change', () => collectAndSync(true));
  }});
  document.querySelectorAll('.fmt-cb').forEach(cb => cb.addEventListener('change', () => collectAndSync(true)));
  document.querySelectorAll('.chip[data-z]').forEach((btn) => {{
    btn.addEventListener('click', () => {{ $('f-zensim').value = btn.dataset.z; collectAndSync(false); runCompare(); }});
  }});
  ['pe-title','pe-caption','pe-alt','pe-feat','pe-hide'].forEach((id) => {{
    $(id).addEventListener('input', () => {{
      const a = state.assets.find(x => x.id === selectedAssetId);
      if (!a) return;
      const ov = (state.config.items[a.source_relative_path] ||= {{}});
      ov.title = $('pe-title').value || null;
      ov.caption = $('pe-caption').value || null;
      ov.alt_text = $('pe-alt').value || null;
      ov.featured = $('pe-feat').checked;
      ov.hidden = $('pe-hide').checked;
      renderPhotoList();
      collectAndSync(true);
    }});
  }});

  async function runCompare() {{
    const targetId = selectedAssetId || (state.assets[0] && state.assets[0].id);
    if (!targetId) return;
    const out = $('cmp-out');
    out.replaceChildren();
    const wait = document.createElement('div');
    wait.textContent = 'Encoding & scoring with zensim_a…';
    out.appendChild(wait);
    try {{
      const res = await api('/api/compare', {{
        asset_id: targetId,
        zensim_target: Number($('f-zensim').value),
        preview_width: 800,
        formats: state.config.encoding.formats
      }});
      if (res.stale) return;
      out.replaceChildren();
      for (const r of res.results) {{
        const box = document.createElement('div');
        box.className = 'cmp-item';
        const head = document.createElement('div');
        head.style.fontWeight = '600';
        const sc = r.measured_zensim_a != null ? ('zensim_a ' + r.measured_zensim_a.toFixed(2)) : ('score ' + r.compare_status);
        head.textContent = r.format.toUpperCase() + ' · ' + (r.bytes / 1024).toFixed(1) + ' KB (' + r.savings_pct.toFixed(0) + '% saved) · ' + sc;
        const sub = document.createElement('div');
        sub.style.fontSize = '0.72rem';
        sub.style.color = '#8b949e';
        sub.textContent = r.effective_coding + (r.fallback_reason ? (' (' + r.fallback_reason + ')') : '');
        box.append(head, sub);
        if (r.preview_data_url) {{
          const im = document.createElement('img');
          im.src = r.preview_data_url;
          im.alt = r.format + ' preview';
          box.appendChild(im);
        }}
        out.appendChild(box);
      }}
    }} catch (err) {{
      out.textContent = 'Compare error: ' + err.message;
    }}
  }}

  $('btn-compare').addEventListener('click', runCompare);
  $('btn-save').addEventListener('click', async () => {{
    try {{
      const res = await api('/api/save-config', {{}});
      statusEl.textContent = 'Saved ' + res.path;
    }} catch (err) {{
      statusEl.textContent = 'Save failed: ' + err.message;
    }}
  }});
  $('btn-export').addEventListener('click', async () => {{
    statusEl.textContent = 'Exporting static gallery (encoding responsive formats)…';
    try {{
      const res = await api('/api/export', {{}});
      const savedPct = res.total_input_bytes > 0 ? (((res.total_input_bytes - res.total_output_bytes) / res.total_input_bytes) * 100).toFixed(1) : '0';
      statusEl.textContent = 'Exported ' + res.visible_items + ' items to ' + res.index_html + ' (' + res.processed + ' encoded, ' + res.skipped_cached + ' cached, ' + res.elapsed_ms + 'ms, ' + savedPct + '% delta)';
    }} catch (err) {{
      statusEl.textContent = 'Export failed: ' + err.message;
    }}
  }});

  api('/api/state').then((data) => {{
    state = data;
    if (state.assets.length) selectedAssetId = state.assets[0].id;
    populateForm();
    statusEl.textContent = 'Ready — ' + state.assets.length + ' photos · export -> ' + state.export_dir_display;
  }}).catch((err) => {{
    statusEl.textContent = 'Init error: ' + err.message;
  }});
}})();
</script>
</body>
</html>"#
    )
}
