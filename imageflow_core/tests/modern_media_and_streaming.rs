//! End-to-end PR 6 qualification in `imageflow_core`:
//! - Modern JSON endpoints (`v2/configure_context`, `v1/execute_zen`, `v2/execute`,
//!   `v2/inspect`, `v2/estimate`, `v2/capabilities`),
//! - First-class AV1, VP8, VP9 × MP4, WebM, IVF + independent `ffprobe` oracle verification,
//! - Video -> still poster extraction & animation <-> video roundtrips,
//! - Safe `ReadAtSource` deferred registration with bounded header-only `v2/inspect`,
//! - Sans-I/O `MediaSession` 1-byte chunking, short range aggregation (`transfer_max = 64`),
//!   partial ACKs, stale-ticket invariance, single-atomic `JobStop` Stop-vs-Commit race,
//!   accurate partial-append `IoError` and interrupted incremental receipts,
//! - Tokio `AsyncJobHandle` deadlock-free reentrant notify (`cancel`/`reply`), abandoned Commit
//!   quiescence, panicking notify unwind safety, compute-permit release during host I/O,
//! - Unified request-level `limits` parity, cross-transport parity, `metadata: "preserve_bytes"`,
//!   bounded SDR `Coding::Verify` qualification, and 256 KiB IIS small-stack thread execution.

#![cfg(all(
    feature = "zen-codecs",
    feature = "all-media",
    feature = "streaming",
    feature = "streaming-async"
))]

use std::num::NonZeroU32;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Condvar, Mutex};
use std::time::{Duration, Instant};

use imageflow_core::media::worker::AsyncMediaRuntime;
use imageflow_core::media::{
    AsyncJobHandle, AsyncJobHooks, BindingSpec, CopyPendingError, IoEvent, IoInput, JobStop,
    MediaSession, OperationKind, ReadAtSource, Reply, SessionError, SliceReadAtSource, StopCause,
    Visibility, WorkQuota, host_services_from_context, shared_modern_engine,
};
use imageflow_core::{Context, ErrorCategory, Result};

/// Serializes process-global `AsyncMediaRuntime::global()` tests in this integration binary
/// so exact `active_jobs()` / `available_compute_permits()` assertions are deterministic under
/// parallel `cargo test` execution.
static ASYNC_RT_TEST_LOCK: Mutex<()> = Mutex::new(());

fn make_two_frame_gif() -> Vec<u8> {
    fn lzw_solid_256(sym: u8) -> Vec<u8> {
        let mut bits: u64 = 0;
        let mut nbits: u32 = 0;
        let mut out = Vec::new();
        let mut emit = |code: u16| {
            bits |= (u64::from(code)) << nbits;
            nbits += 3;
            while nbits >= 8 {
                out.push((bits & 0xFF) as u8);
                bits >>= 8;
                nbits -= 8;
            }
        };
        for _ in 0..128 {
            emit(4);
            emit(u16::from(sym));
            emit(u16::from(sym));
        }
        emit(4);
        emit(5);
        if nbits > 0 {
            out.push((bits & 0xFF) as u8);
        }
        out
    }
    let mut g = Vec::new();
    g.extend_from_slice(b"GIF89a");
    g.extend_from_slice(&[16, 0, 16, 0, 0x80, 0, 0]);
    g.extend_from_slice(&[220, 40, 30, 30, 120, 230]);
    // Frame 0: red, 40ms
    g.extend_from_slice(&[0x21, 0xF9, 0x04, 0x04, 0x04, 0x00, 0x00, 0x00]);
    g.extend_from_slice(&[0x2C, 0, 0, 0, 0, 16, 0, 16, 0, 0x00]);
    g.push(2);
    let f0 = lzw_solid_256(0);
    for chunk in f0.chunks(255) {
        g.push(chunk.len() as u8);
        g.extend_from_slice(chunk);
    }
    g.push(0x00);
    // Frame 1: blue, 40ms
    g.extend_from_slice(&[0x21, 0xF9, 0x04, 0x04, 0x04, 0x00, 0x00, 0x00]);
    g.extend_from_slice(&[0x2C, 0, 0, 0, 0, 16, 0, 16, 0, 0x00]);
    g.push(2);
    let f1 = lzw_solid_256(1);
    for chunk in f1.chunks(255) {
        g.push(chunk.len() as u8);
        g.extend_from_slice(chunk);
    }
    g.push(0x00);
    g.push(0x3B);
    g
}

fn ffprobe_verify(bytes: &[u8], ext: &str, expected_codec: &str, exp_w: u64, exp_h: u64) {
    let dir = std::env::var("HOME").unwrap_or_else(|_| ".".into()) + "/tmp/imageflow-ffprobe-test";
    std::fs::create_dir_all(&dir).expect("create ~/tmp dir");
    let path = format!("{dir}/clip_{expected_codec}_{ext}_{}.{ext}", std::process::id());
    std::fs::write(&path, bytes).expect("write clip for ffprobe");
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name,width,height",
            "-of",
            "json",
            &path,
        ])
        .output()
        .expect("ffprobe must be available for independent oracle verification");
    let _ = std::fs::remove_file(&path);
    assert!(
        out.status.success(),
        "ffprobe failed on {expected_codec}/{ext}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("valid ffprobe json");
    let stream = &v["streams"][0];
    assert_eq!(stream["codec_name"].as_str(), Some(expected_codec));
    assert_eq!(stream["width"].as_u64(), Some(exp_w));
    assert_eq!(stream["height"].as_u64(), Some(exp_h));
}

struct CountingReadAtSource {
    bytes: Arc<Vec<u8>>,
    bytes_read: Arc<AtomicU64>,
}

impl ReadAtSource for CountingReadAtSource {
    fn read_at(&self, offset: u64, dst: &mut [u8]) -> Result<usize> {
        let Ok(off) = usize::try_from(offset) else {
            return Ok(0);
        };
        if off >= self.bytes.len() || dst.is_empty() {
            return Ok(0);
        }
        let n = (self.bytes.len() - off).min(dst.len());
        dst[..n].copy_from_slice(&self.bytes[off..off + n]);
        self.bytes_read.fetch_add(n as u64, Ordering::SeqCst);
        Ok(n)
    }

    fn length(&self) -> Result<Option<u64>> {
        Ok(Some(self.bytes.len() as u64))
    }
}

fn drive_session_manual(
    session: &mut MediaSession,
    input: &[u8],
    in_chunk_size: usize,
    out_ack_step: usize,
    fail_on_commit_cancel: bool,
    assert_max_write_len: Option<u32>,
) -> (Vec<u8>, String) {
    let mut output = Vec::new();
    let mut committed = false;

    for _ in 0..400_000 {
        let ev = session.next(WorkQuota(16)).expect("next succeeds");
        if !matches!(ev, IoEvent::Yield | IoEvent::Complete | IoEvent::Failed { .. }) {
            let ev2 = session.next(WorkQuota(16)).expect("repeat next stable");
            assert_eq!(ev, ev2, "level-triggered event must repeat identically before reply");
        }
        match ev {
            IoEvent::Yield => continue,
            IoEvent::Complete => {
                let json = String::from_utf8(
                    session.result_json().expect("terminal json present").to_vec(),
                )
                .expect("utf8 json");
                return (output, json);
            }
            IoEvent::Failed { category, cause } => {
                panic!(
                    "unexpected Failed({category:?}, {cause:?}): {}",
                    String::from_utf8_lossy(session.result_json().unwrap_or(&[]))
                );
            }
            IoEvent::NeedInput { ticket, request, .. } => {
                let stale = imageflow_core::media::IoTicket::from_raw(ticket.raw() + 999).unwrap();
                assert_eq!(
                    session.reply(stale, Reply::UnknownLength),
                    Err(SessionError::StaleTicket)
                );
                match request {
                    IoInput::Len => {
                        session.reply(ticket, Reply::Length(input.len() as u64)).unwrap();
                    }
                    IoInput::AtLeast { next_offset, max_transfer, .. } => {
                        let off = next_offset as usize;
                        if off >= input.len() {
                            session
                                .reply(ticket, Reply::End { total_len: Some(input.len() as u64) })
                                .unwrap();
                        } else {
                            let take = in_chunk_size
                                .min(max_transfer as usize)
                                .min(input.len() - off)
                                .max(1);
                            session
                                .reply(ticket, Reply::Bytes(&input[off..off + take]))
                                .unwrap();
                        }
                    }
                    IoInput::Range { offset, len } => {
                        let off = offset as usize;
                        if off >= input.len() {
                            session
                                .reply(ticket, Reply::End { total_len: Some(input.len() as u64) })
                                .unwrap();
                        } else {
                            let take = in_chunk_size
                                .min(len as usize)
                                .min(input.len() - off)
                                .max(1);
                            session
                                .reply(ticket, Reply::Bytes(&input[off..off + take]))
                                .unwrap();
                        }
                    }
                }
            }
            IoEvent::Output { ticket, offset, len, .. } => {
                if let Some(cap) = assert_max_write_len {
                    assert!(len <= cap, "Output chunk {len} exceeded transfer_max {cap}");
                }
                if len > 1 {
                    let mut tiny = [0u8; 1];
                    assert_eq!(
                        session.copy_pending(ticket, &mut tiny),
                        Err(CopyPendingError::BufferTooSmall(len as usize))
                    );
                }
                let mut buf = vec![0u8; len as usize];
                let copied = session.copy_pending(ticket, &mut buf).unwrap();
                assert_eq!(copied, len as usize);
                let ack = out_ack_step.min(copied).max(1);
                assert_eq!(output.len() as u64, offset);
                output.extend_from_slice(&buf[..ack]);
                session.reply(ticket, Reply::Ack { bytes: ack as u64 }).unwrap();
            }
            IoEvent::Patch { ticket, offset, len, .. } => {
                if let Some(cap) = assert_max_write_len {
                    assert!(len <= cap, "Patch chunk {len} exceeded transfer_max {cap}");
                }
                let mut buf = vec![0u8; len as usize];
                let copied = session.copy_pending(ticket, &mut buf).unwrap();
                let off = offset as usize;
                output[off..off + copied].copy_from_slice(&buf[..copied]);
                session.reply(ticket, Reply::Ack { bytes: copied as u64 }).unwrap();
            }
            IoEvent::Flush { ticket, .. } => {
                session.reply(ticket, Reply::Ack { bytes: 0 }).unwrap();
            }
            IoEvent::Commit { ticket } => {
                if fail_on_commit_cancel {
                    // Race cancellation AFTER Commit authorization — Commit MUST STILL WIN!
                    let won = session.stop_handle().trip(StopCause::CallerCancel);
                    assert!(!won, "trip after Commit authorization must return false");
                    assert!(
                        !session.stop_handle().is_stopped(),
                        "stop must not latch after Commit authorization"
                    );
                    let repeated = session.next(WorkQuota(4)).unwrap();
                    assert_eq!(
                        repeated,
                        IoEvent::Commit { ticket },
                        "cancellation after Commit authorization must NOT abort in-flight Commit"
                    );
                }
                committed = true;
                session.reply(ticket, Reply::Ack { bytes: 0 }).unwrap();
            }
        }
    }
    let _ = committed;
    panic!("session exceeded max manual steps");
}

#[test]
fn stop_vs_commit_single_atomic_race_is_strictly_mutually_exclusive() {
    for _ in 0..5_000 {
        let stop = JobStop::new();
        let b = Arc::new(Barrier::new(2));
        let s2 = stop.clone();
        let b2 = Arc::clone(&b);
        let t = std::thread::spawn(move || {
            b2.wait();
            s2.trip(StopCause::CallerCancel)
        });
        b.wait();
        let commit_res = stop.try_enter_committing();
        let cancel_won = t.join().unwrap();

        match (commit_res, cancel_won) {
            (Ok(()), false) => {
                assert!(stop.is_committing());
                assert!(!stop.is_stopped());
            }
            (Err(StopCause::CallerCancel), true) => {
                assert!(stop.is_stopped());
                assert!(!stop.is_committing());
            }
            other => panic!("double-win or double-loss in JobStop race: {other:?}"),
        }
    }
}

#[test]
fn modern_endpoints_av1_vp8_vp9_mp4_webm_ivf_and_ffprobe_oracle() {
    let gif_bytes = make_two_frame_gif();

    // 1. Live registry-driven v2/capabilities check
    let mut ctx = Context::create().unwrap();
    let (cap_resp, cap_res) = ctx.message("v2/capabilities", b"{}");
    cap_res.unwrap();
    cap_resp.assert_ok();
    let cap_json: serde_json::Value = serde_json::from_slice(&cap_resp.response_json).unwrap();
    assert_eq!(cap_json["data"]["wire_schema"], "zenpipe-canonical-v1");
    assert_eq!(cap_json["data"]["zen_media"], true);
    assert_eq!(cap_json["data"]["streaming_manual"], true);
    assert_eq!(cap_json["data"]["streaming_async"], true);
    assert_eq!(cap_json["data"]["verify_metrics"].as_array().unwrap().len(), 0);
    let codecs: Vec<&str> = cap_json["data"]["video_codecs"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(codecs.contains(&"av1") && codecs.contains(&"vp8") && codecs.contains(&"vp9"));
    let routes = cap_json["data"]["media_routes"].as_array().unwrap();
    let av1_mp4_dec = routes
        .iter()
        .find(|r| r["container"] == "mp4" && r["video_codec"] == "av1" && r["direction"] == "decode")
        .unwrap();
    assert_eq!(av1_mp4_dec["supported"], true);
    assert_eq!(av1_mp4_dec["provider_id"], "org.imazen.av1.rav1d");

    // 2. Encode GIF -> AV1/MP4, VP8/WebM, VP9/WebM, VP8/IVF via v2/execute + verify with ffprobe!
    let targets: [(&str, &str, &str, &str); 4] = [
        ("mp4", "av1", "mp4", "av1"),
        ("webm", "vp8", "webm", "vp8"),
        ("webm", "vp9", "webm", "vp9"),
        ("ivf", "vp8", "ivf", "vp8"),
    ];
    let mut mp4_av1_bytes = Vec::new();
    let mut webm_vp8_bytes = Vec::new();

    for (container, vcodec, ext, ff_codec) in targets {
        let mut c = Context::create().unwrap();
        let (cfg_r, cfg_res) = c.message(
            "v2/configure_context",
            br#"{"input_registration":"deferred","workspace_policy":"best_effort"}"#,
        );
        cfg_res.unwrap();
        cfg_r.assert_ok();

        c.add_copied_input_buffer(0, &gif_bytes).unwrap();
        c.add_output_buffer(1).unwrap();

        let timing_str = if container == "mp4" {
            r#""exact""#
        } else {
            r#"{"quantize_endpoints":{"max_boundary_error":{"numerator":1,"denominator":1000}}}"#
        };
        let req = format!(
            r#"{{
                "inputs": {{"gif": 0}},
                "outputs": {{"vid": 1}},
                "pipeline": {{
                    "steps": [
                        {{"decode": {{"input": "gif"}}}},
                        {{"encode": {{
                            "output": "vid",
                            "format": "{container}",
                            "video_codec": "{vcodec}",
                            "sequence": {{"preserve": {{"timing": {timing_str}, "loops": {{"set": {{"plays": 1}}}}}}}}
                        }}}}
                    ]
                }}
            }}"#
        );

        let (resp, res) = c.message("v2/execute", req.as_bytes());
        res.unwrap_or_else(|e| panic!("v2/execute failed for {container}/{vcodec}: {e}"));
        resp.assert_ok();
        let body: serde_json::Value = serde_json::from_slice(&resp.response_json).unwrap();
        assert_eq!(body["data"]["version"], 2);
        assert_eq!(body["data"]["outputs"][0]["kind"], "video");
        assert_eq!(body["data"]["outputs"][0]["video_codec"], vcodec);

        let out_bytes = c.take_output_buffer(1).unwrap();
        assert!(!out_bytes.is_empty());
        ffprobe_verify(&out_bytes, ext, ff_codec, 16, 16);

        if container == "mp4" && vcodec == "av1" {
            mp4_av1_bytes = out_bytes;
        } else if container == "webm" && vcodec == "vp8" {
            webm_vp8_bytes = out_bytes;
        }
    }

    // 3. Bounded ReadAt v2/inspect (proving zero full-file read on large trailing free/pad bytes),
    //    real StartupDependenciesV2 in v2/estimate, and v1/execute_zen poster extraction!
    let mut padded_webm = webm_vp8_bytes.clone();
    padded_webm.extend(std::iter::repeat_n(0u8, 256 * 1024));
    let read_counter = Arc::new(AtomicU64::new(0));
    let mut c_insp = Context::create().unwrap();
    c_insp
        .message("v2/configure_context", br#"{"input_registration":"deferred"}"#)
        .1
        .unwrap();
    c_insp
        .add_input_read_at(
            0,
            Arc::new(CountingReadAtSource {
                bytes: Arc::new(padded_webm.clone()),
                bytes_read: Arc::clone(&read_counter),
            }),
        )
        .unwrap();
    let (insp_pad_r, insp_pad_res) = c_insp.message(
        "v2/inspect",
        br#"{"inputs":{"clip":0},"input":"clip","video":{"container":"webm"},"scan":"headers"}"#,
    );
    insp_pad_res.unwrap();
    insp_pad_r.assert_ok();
    assert!(
        read_counter.load(Ordering::SeqCst) < padded_webm.len() as u64,
        "header-only ReadAt v2/inspect must NOT materialize the entire {} B file (read {} B)",
        padded_webm.len(),
        read_counter.load(Ordering::SeqCst)
    );

    let mut c_poster = Context::create().unwrap();
    c_poster
        .message(
            "v2/configure_context",
            br#"{"input_registration":"deferred","workspace_policy":"best_effort"}"#,
        )
        .1
        .unwrap();
    c_poster
        .add_input_read_at(0, Arc::new(SliceReadAtSource::new(Arc::new(mp4_av1_bytes.clone()))))
        .unwrap();
    c_poster.add_output_buffer(1).unwrap();

    let (insp_r, insp_res) = c_poster.message(
        "v2/inspect",
        br#"{"inputs":{"clip":0},"input":"clip","video":{"container":"mp4"},"scan":"bounded_index"}"#,
    );
    insp_res.unwrap();
    insp_r.assert_ok();
    let insp_json: serde_json::Value = serde_json::from_slice(&insp_r.response_json).unwrap();
    assert_eq!(insp_json["data"]["container"], "mp4");
    assert_eq!(insp_json["data"]["tracks"][0]["kind"], "video");
    assert_eq!(insp_json["data"]["tracks"][0]["codec"], "av1");
    assert_eq!(insp_json["data"]["tracks"][0]["coded"]["width"], 16);

    let poster_req = br#"{
        "inputs": {"clip": 0},
        "outputs": {"poster": 1},
        "pipeline": {
            "steps": [
                {"decode": {"input": "clip", "select": {"presentation_index": {"index": 0}}}},
                {"geometry": {"constrain": {"mode": "fit", "width": 8, "height": 8}}},
                {"encode": {"output": "poster", "format": "png"}}
            ]
        }
    }"#;
    let (est_r, est_res) = c_poster.message("v2/estimate", poster_req);
    est_res.unwrap();
    est_r.assert_ok();
    let est_json: serde_json::Value = serde_json::from_slice(&est_r.response_json).unwrap();
    assert_eq!(est_json["data"]["version"], 2);
    assert_eq!(est_json["data"]["output_count"], 1);
    assert_eq!(
        est_json["data"]["startup_dependencies"]["needs_length"], true,
        "MP4 input estimate must report needs_length=true"
    );
    assert_eq!(
        est_json["data"]["startup_dependencies"]["needs_range"], true,
        "MP4 input estimate must report needs_range=true"
    );

    let (zen_r, zen_res) = c_poster.message("v1/execute_zen", poster_req);
    zen_res.unwrap();
    zen_r.assert_ok();
    let poster_png = c_poster.take_output_buffer(1).unwrap();
    assert!(poster_png.starts_with(b"\x89PNG\r\n\x1a\n"));

    // Taken output slot MUST NOT be overwritten by a second v2/execute!
    let (_, second_err) = c_poster.message("v2/execute", poster_req);
    assert!(second_err.is_err(), "already-taken output buffer must refuse overwrite");

    // 4. Unified request-level `limits` enforcement parity across sync AND streaming!
    let tight_limit_req = br#"{
        "inputs": {"src": 0},
        "outputs": {"dst": 1},
        "limits": {"max_input_bytes": 16},
        "pipeline": {
            "steps": [
                {"decode": {"input": "src"}},
                {"encode": {
                    "output": "dst",
                    "format": "webm",
                    "video_codec": "vp8",
                    "sequence": {"preserve": {"timing": {"quantize_endpoints":{"max_boundary_error":{"numerator":1,"denominator":1000}}}, "loops": {"set": {"plays": 1}}}}
                }}
            ]
        }
    }"#;
    let engine = shared_modern_engine().unwrap();
    let mut cfg_ctx = Context::create().unwrap();
    cfg_ctx
        .message("v2/configure_context", br#"{"input_registration":"deferred","workspace_policy":"best_effort"}"#)
        .1
        .unwrap();
    let services = host_services_from_context(&cfg_ctx);
    cfg_ctx.add_copied_input_buffer(0, &webm_vp8_bytes).unwrap();
    cfg_ctx.add_output_buffer(1).unwrap();
    let (_, sync_lim_err) = cfg_ctx.message("v2/execute", tight_limit_req);
    assert!(sync_lim_err.is_err(), "sync v2/execute must enforce envelope limits.max_input_bytes");

    let mut lim_sess = MediaSession::create(
        Arc::clone(&engine),
        services.clone(),
        OperationKind::Execute,
        tight_limit_req,
        &[
            BindingSpec::read_at_source(0, true),
            BindingSpec::forward_sink(1, Visibility::Staged),
        ],
    )
    .unwrap();
    let ev = lim_sess.next(WorkQuota(4)).unwrap();
    let IoEvent::NeedInput { ticket, request: IoInput::Len, .. } = ev else {
        panic!("expected NeedLen, got {ev:?}");
    };
    lim_sess.reply(ticket, Reply::Length(webm_vp8_bytes.len() as u64)).unwrap();
    let term = lim_sess.next(WorkQuota(4)).unwrap();
    assert!(
        matches!(term, IoEvent::Failed { .. }),
        "streaming MediaSession must also enforce envelope limits.max_input_bytes (got {term:?})"
    );

    // 5. Cross-transport parity + small transfer_max (64 B) + 7-byte short range reads on MP4 -> WebM/VP8!
    let transcode_envelope = br#"{
        "inputs": {"src": 0},
        "outputs": {"dst": 1},
        "pipeline": {
            "steps": [
                {"decode": {"input": "src"}},
                {"encode": {
                    "output": "dst",
                    "format": "webm",
                    "video_codec": "vp8",
                    "sequence": {"preserve": {"timing": {"quantize_endpoints":{"max_boundary_error":{"numerator":1,"denominator":1000}}}, "loops": {"set": {"plays": 1}}}}
                }}
            ]
        }
    }"#;
    let bindings = [
        BindingSpec::read_at_source(0, true),
        BindingSpec::forward_sink(1, Visibility::Staged),
    ];

    // 5a. Sync v2/execute directly on ReadAtSource MP4/AV1 -> WebM/VP8
    let mut c_sync = Context::create().unwrap();
    c_sync
        .message("v2/configure_context", br#"{"input_registration":"deferred","workspace_policy":"best_effort"}"#)
        .1
        .unwrap();
    c_sync
        .add_input_read_at(0, Arc::new(SliceReadAtSource::new(Arc::new(mp4_av1_bytes.clone()))))
        .unwrap();
    c_sync.add_output_buffer(1).unwrap();
    let (sync_r, sync_res) = c_sync.message("v2/execute", transcode_envelope);
    sync_res.unwrap();
    sync_r.assert_ok();
    let sync_v2: serde_json::Value = serde_json::from_slice(&sync_r.response_json).unwrap();
    let sync_out = c_sync.take_output_buffer(1).unwrap();
    assert_eq!(sync_v2["data"]["outputs"][0]["width"], 16);
    assert_eq!(sync_v2["data"]["outputs"][0]["height"], 16);
    ffprobe_verify(&sync_out, "webm", "vp8", 16, 16);

    // 5b. Manual MediaSession with transfer_max=64, 7-byte short Range reads, 7-byte partial ACKs,
    //     post-Commit cancel immunity, AND real width/height == 16x16!
    let mut sess = MediaSession::create(
        Arc::clone(&engine),
        services.clone(),
        OperationKind::Execute,
        transcode_envelope,
        &bindings,
    )
    .unwrap();
    sess.set_transfer_max(64);
    let (streamed_webm, stream_json) =
        drive_session_manual(&mut sess, &mp4_av1_bytes, 7, 7, true, Some(64));
    let stream_v2: serde_json::Value = serde_json::from_str(&stream_json).unwrap();
    assert_eq!(stream_v2["version"], 2);
    assert_eq!(stream_v2["outputs"][0]["video_codec"], "vp8");
    assert_eq!(stream_v2["outputs"][0]["width"], 16, "direct pump must report real coded width");
    assert_eq!(stream_v2["outputs"][0]["height"], 16, "direct pump must report real coded height");
    ffprobe_verify(&streamed_webm, "webm", "vp8", 16, 16);

    // 5c. Tokio AsyncJobHandle parity on the same MP4/AV1 fixture!
    let _rt_guard = ASYNC_RT_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let notify_pair = Arc::new((Mutex::new(0u32), Condvar::new()));
    let release_count = Arc::new(AtomicUsize::new(0));
    let np_cb = Arc::clone(&notify_pair);
    let rc_cb = Arc::clone(&release_count);

    let async_sess = MediaSession::create(
        Arc::clone(&engine),
        services.clone(),
        OperationKind::Execute,
        transcode_envelope,
        &bindings,
    )
    .unwrap();
    let handle = AsyncJobHandle::start(
        async_sess,
        AsyncJobHooks {
            notify: Box::new(move |flags| {
                let (m, cv) = &*np_cb;
                let mut g = m.lock().unwrap();
                *g |= flags;
                cv.notify_all();
            }),
            release: Box::new(move || {
                rc_cb.fetch_add(1, Ordering::SeqCst);
            }),
        },
        Some(Duration::from_secs(30)),
    )
    .map_err(|(e, _, _)| e)
    .unwrap();

    let mut async_out = Vec::new();
    let async_json = loop {
        match handle.poll_event() {
            None => {
                let (m, cv) = &*notify_pair;
                let mut g = m.lock().unwrap();
                while *g == 0 && handle.poll_event().is_none() {
                    let (ng, _) = cv.wait_timeout(g, Duration::from_millis(500)).unwrap();
                    g = ng;
                }
                *g = 0;
            }
            Some(IoEvent::Complete) => {
                break String::from_utf8(handle.result_json_cloned().unwrap()).unwrap();
            }
            Some(IoEvent::Failed { category, cause }) => {
                panic!("async job failed ({category:?}, {cause:?})");
            }
            Some(IoEvent::NeedInput { ticket, request, .. }) => match request {
                IoInput::Len => handle.reply(ticket, Reply::Length(mp4_av1_bytes.len() as u64)).unwrap(),
                IoInput::AtLeast { next_offset, max_transfer, .. } => {
                    let off = next_offset as usize;
                    if off >= mp4_av1_bytes.len() {
                        handle
                            .reply(ticket, Reply::End { total_len: Some(mp4_av1_bytes.len() as u64) })
                            .unwrap();
                    } else {
                        let take = (max_transfer as usize).min(mp4_av1_bytes.len() - off);
                        handle.reply(ticket, Reply::Bytes(&mp4_av1_bytes[off..off + take])).unwrap();
                    }
                }
                IoInput::Range { offset, len } => {
                    let off = offset as usize;
                    if off >= mp4_av1_bytes.len() {
                        handle
                            .reply(ticket, Reply::End { total_len: Some(mp4_av1_bytes.len() as u64) })
                            .unwrap();
                    } else {
                        let take = (len as usize).min(mp4_av1_bytes.len() - off);
                        handle.reply(ticket, Reply::Bytes(&mp4_av1_bytes[off..off + take])).unwrap();
                    }
                }
            },
            Some(IoEvent::Output { ticket, len, .. }) => {
                let mut buf = vec![0u8; len as usize];
                let n = handle.copy_pending(ticket, &mut buf).unwrap();
                async_out.extend_from_slice(&buf[..n]);
                handle.reply(ticket, Reply::Ack { bytes: n as u64 }).unwrap();
            }
            Some(IoEvent::Patch { ticket, offset, len, .. }) => {
                let mut buf = vec![0u8; len as usize];
                let n = handle.copy_pending(ticket, &mut buf).unwrap();
                let off = offset as usize;
                async_out[off..off + n].copy_from_slice(&buf[..n]);
                handle.reply(ticket, Reply::Ack { bytes: n as u64 }).unwrap();
            }
            Some(IoEvent::Flush { ticket, .. } | IoEvent::Commit { ticket }) => {
                handle.reply(ticket, Reply::Ack { bytes: 0 }).unwrap();
            }
            Some(IoEvent::Yield) => {}
        }
    };
    handle.release_caller_handle();

    let deadline = Instant::now() + Duration::from_secs(2);
    while release_count.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(release_count.load(Ordering::SeqCst), 1);
    let async_v2: serde_json::Value = serde_json::from_str(&async_json).unwrap();
    assert_eq!(async_v2["outputs"][0]["video_codec"], "vp8");
    assert_eq!(async_v2["outputs"][0]["width"], 16);
    assert_eq!(async_v2["outputs"][0]["height"], 16);
    ffprobe_verify(&async_out, "webm", "vp8", 16, 16);
    assert_eq!(
        streamed_webm, async_out,
        "manual MediaSession and Tokio AsyncJobHandle must emit byte-identical output"
    );

    // 6. 256 KiB IIS small-stack caller thread execution!
    let mp4_for_stack = mp4_av1_bytes;
    let stack_join = std::thread::Builder::new()
        .name("iis-256k-stack-check".to_string())
        .stack_size(256 * 1024)
        .spawn(move || {
            let mut sc = Context::create().unwrap();
            sc.message(
                "v2/configure_context",
                br#"{"input_registration":"deferred","workspace_policy":"best_effort"}"#,
            )
            .1
            .unwrap();
            sc.add_copied_input_buffer(0, &mp4_for_stack).unwrap();
            let (ir, ires) = sc.message(
                "v2/inspect",
                br#"{"inputs":{"clip":0},"input":"clip"}"#,
            );
            ires.unwrap();
            ir.assert_ok();
        })
        .expect("spawn 256 KiB stack thread");
    stack_join.join().expect("256 KiB stack thread must complete without overflow");
}

#[test]
fn partial_append_io_error_and_incremental_cancel_receipts() {
    let engine = shared_modern_engine().unwrap();
    let best_effort = zenpipe::canonical::HostServices::new()
        .with_workspace_policy(zencodecs::engine::WorkspacePolicy::BestEffort);
    let canvas_req = br#"{
        "inputs": {},
        "outputs": {"out": 1},
        "pipeline": {
            "steps": [
                {"canvas": {"width": 8, "height": 8, "fill": [40, 120, 220, 255]}},
                {"encode": {"output": "out", "format": "png"}}
            ]
        }
    }"#;
    let bindings = [BindingSpec::forward_sink(1, Visibility::Incremental)];

    // Test A: Append IoError on chunk 2 accumulates accepted_prefix onto prior accepted_bytes!
    let mut s1 = MediaSession::create(
        Arc::clone(&engine),
        best_effort.clone(),
        OperationKind::Execute,
        canvas_req,
        &bindings,
    )
    .unwrap();
    s1.set_transfer_max(16);
    let IoEvent::Output { ticket: t1, len: l1, .. } = s1.next(WorkQuota(16)).unwrap() else {
        panic!("expected first Output");
    };
    assert_eq!(l1, 16);
    s1.reply(t1, Reply::Ack { bytes: 10 }).unwrap();
    let IoEvent::Output { ticket: t2, len: l2, .. } = s1.next(WorkQuota(16)).unwrap() else {
        panic!("expected second Output");
    };
    // Oversized prefix must be rejected without mutating state!
    assert!(matches!(
        s1.reply(
            t2,
            Reply::IoError { accepted_prefix: Some(u64::from(l2) + 1), diagnostic: b"bad" }
        ),
        Err(SessionError::InvalidArgument(_))
    ));
    // Valid relative prefix `3` must add onto previous `10` -> `"13"` (NOT `max(10, 3) == "10"`)!
    s1.reply(t2, Reply::IoError { accepted_prefix: Some(3), diagnostic: b"disk full" })
        .unwrap();
    let term1 = s1.next(WorkQuota(4)).unwrap();
    assert!(matches!(term1, IoEvent::Failed { category: ErrorCategory::IoError, .. }));
    let rep1: serde_json::Value = serde_json::from_slice(s1.result_json().unwrap()).unwrap();
    assert_eq!(rep1["publications"][0]["accepted_bytes"], "13");
    assert_eq!(rep1["publications"][0]["exact_unknown_tail"], false);

    // Test B: Cancelling while an incremental Output ticket is in flight marks exact_unknown_tail=true!
    let mut s2 = MediaSession::create(
        Arc::clone(&engine),
        best_effort,
        OperationKind::Execute,
        canvas_req,
        &bindings,
    )
    .unwrap();
    let IoEvent::Output { .. } = s2.next(WorkQuota(16)).unwrap() else {
        panic!("expected Output");
    };
    s2.stop_handle().trip(StopCause::CallerCancel);
    let term2 = s2.next(WorkQuota(4)).unwrap();
    assert_eq!(
        term2,
        IoEvent::Failed {
            category: ErrorCategory::OperationCancelled,
            cause: StopCause::CallerCancel,
        }
    );
    let rep2: serde_json::Value = serde_json::from_slice(s2.result_json().unwrap()).unwrap();
    assert_eq!(
        rep2["publications"][0]["exact_unknown_tail"], true,
        "interrupted in-flight incremental write must mark exact_unknown_tail=true"
    );
}

#[test]
fn async_reentrant_notify_abandoned_commit_panicking_notify_and_permit_release() {
    let _rt_guard = ASYNC_RT_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let engine = shared_modern_engine().unwrap();
    let best_effort = zenpipe::canonical::HostServices::new()
        .with_workspace_policy(zencodecs::engine::WorkspacePolicy::BestEffort);
    let canvas_req = br#"{
        "inputs": {},
        "outputs": {"out": 1},
        "pipeline": {
            "steps": [
                {"canvas": {"width": 4, "height": 4, "fill": [10, 200, 150, 255]}},
                {"encode": {"output": "out", "format": "png"}}
            ]
        }
    }"#;
    let staged_bindings = [BindingSpec::forward_sink(1, Visibility::Staged)];

    // A. Reentrant notify -> cancel() AND reentrant notify -> poll_event()/reply(): ZERO deadlock!
    {
        let rel = Arc::new(AtomicUsize::new(0));
        let rel_cb = Arc::clone(&rel);
        let slot: Arc<Mutex<Option<Arc<AsyncJobHandle>>>> = Arc::new(Mutex::new(None));
        let slot_cb = Arc::clone(&slot);
        let done_flag = Arc::new(AtomicBool::new(false));
        let done_cb = Arc::clone(&done_flag);

        let sess = MediaSession::create(
            Arc::clone(&engine),
            best_effort.clone(),
            OperationKind::Execute,
            canvas_req,
            &staged_bindings,
        )
        .unwrap();
        let (handle, activator) = AsyncJobHandle::start_paused(
            sess,
            AsyncJobHooks {
                notify: Box::new(move |_flags| {
                    let h = slot_cb.lock().unwrap().clone();
                    if let Some(h) = h {
                        // Synchronously call cancel() inside notify! Must NEVER deadlock!
                        h.cancel();
                        done_cb.store(true, Ordering::SeqCst);
                    }
                }),
                release: Box::new(move || {
                    rel_cb.fetch_add(1, Ordering::SeqCst);
                }),
            },
            None,
        )
        .map_err(|(e, _, _)| e)
        .unwrap();
        let handle = Arc::new(handle);
        *slot.lock().unwrap() = Some(Arc::clone(&handle));
        activator.activate();

        let dl = Instant::now() + Duration::from_secs(2);
        while rel.load(Ordering::SeqCst) == 0 && Instant::now() < dl {
            std::thread::yield_now();
        }
        assert!(done_flag.load(Ordering::SeqCst));
        assert_eq!(rel.load(Ordering::SeqCst), 1, "reentrant cancel inside notify must release once");
    }

    // B. Reentrant notify -> poll_event() + reply() driving full job to Complete: ZERO deadlock!
    {
        let rel = Arc::new(AtomicUsize::new(0));
        let rel_cb = Arc::clone(&rel);
        let completed = Arc::new(AtomicBool::new(false));
        let comp_cb = Arc::clone(&completed);
        let slot: Arc<Mutex<Option<Arc<AsyncJobHandle>>>> = Arc::new(Mutex::new(None));
        let slot_cb = Arc::clone(&slot);

        let sess = MediaSession::create(
            Arc::clone(&engine),
            best_effort.clone(),
            OperationKind::Execute,
            canvas_req,
            &staged_bindings,
        )
        .unwrap();
        let (handle, activator) = AsyncJobHandle::start_paused(
            sess,
            AsyncJobHooks {
                notify: Box::new(move |_flags| {
                    let Some(h) = slot_cb.lock().unwrap().clone() else { return };
                    match h.poll_event() {
                        Some(IoEvent::Output { ticket, len, .. }) => {
                            let _ = h.reply(ticket, Reply::Ack { bytes: u64::from(len) });
                        }
                        Some(IoEvent::Flush { ticket, .. } | IoEvent::Commit { ticket }) => {
                            let _ = h.reply(ticket, Reply::Ack { bytes: 0 });
                        }
                        Some(IoEvent::Complete) => {
                            comp_cb.store(true, Ordering::SeqCst);
                        }
                        _ => {}
                    }
                }),
                release: Box::new(move || {
                    rel_cb.fetch_add(1, Ordering::SeqCst);
                }),
            },
            None,
        )
        .map_err(|(e, _, _)| e)
        .unwrap();
        let handle = Arc::new(handle);
        *slot.lock().unwrap() = Some(Arc::clone(&handle));
        activator.activate();

        let dl = Instant::now() + Duration::from_secs(2);
        while rel.load(Ordering::SeqCst) == 0 && Instant::now() < dl {
            std::thread::yield_now();
        }
        assert!(completed.load(Ordering::SeqCst), "reentrant reply inside notify must reach Complete");
        assert_eq!(rel.load(Ordering::SeqCst), 1);
    }

    // C. Abandoning handle AFTER receiving IoEvent::Commit still reaches quiescence and releases once!
    {
        let rel = Arc::new(AtomicUsize::new(0));
        let rel_cb = Arc::clone(&rel);
        let pair = Arc::new((Mutex::new(0u32), Condvar::new()));
        let pair_cb = Arc::clone(&pair);

        let sess = MediaSession::create(
            Arc::clone(&engine),
            best_effort.clone(),
            OperationKind::Execute,
            canvas_req,
            &staged_bindings,
        )
        .unwrap();
        let handle = AsyncJobHandle::start(
            sess,
            AsyncJobHooks {
                notify: Box::new(move |flags| {
                    let (m, cv) = &*pair_cb;
                    *m.lock().unwrap() |= flags;
                    cv.notify_all();
                }),
                release: Box::new(move || {
                    rel_cb.fetch_add(1, Ordering::SeqCst);
                }),
            },
            None, // Deliberately NO deadline to prove abandonment alone drives quiescence!
        )
        .map_err(|(e, _, _)| e)
        .unwrap();

        loop {
            match handle.poll_event() {
                None => {
                    let (m, cv) = &*pair;
                    let mut g = m.lock().unwrap();
                    while *g == 0 && handle.poll_event().is_none() {
                        let (ng, _) = cv.wait_timeout(g, Duration::from_millis(200)).unwrap();
                        g = ng;
                    }
                    *g = 0;
                }
                Some(IoEvent::Output { ticket, len, .. }) => {
                    handle.reply(ticket, Reply::Ack { bytes: u64::from(len) }).unwrap();
                }
                Some(IoEvent::Flush { ticket, .. }) => {
                    handle.reply(ticket, Reply::Ack { bytes: 0 }).unwrap();
                }
                Some(IoEvent::Commit { .. }) => {
                    // Drop handle WITHOUT replying to Commit!
                    drop(handle);
                    break;
                }
                other => panic!("unexpected event before Commit: {other:?}"),
            }
        }

        let dl = Instant::now() + Duration::from_secs(2);
        while rel.load(Ordering::SeqCst) == 0 && Instant::now() < dl {
            std::thread::yield_now();
        }
        assert_eq!(rel.load(Ordering::SeqCst), 1, "abandoned Commit must still fire release once");
    }

    // D. Panicking notify hook STILL fires release once and returns active_jobs() to 0!
    {
        let rel = Arc::new(AtomicUsize::new(0));
        let rel_cb = Arc::clone(&rel);
        let sess = MediaSession::create(
            Arc::clone(&engine),
            best_effort.clone(),
            OperationKind::Execute,
            canvas_req,
            &staged_bindings,
        )
        .unwrap();
        let handle = AsyncJobHandle::start(
            sess,
            AsyncJobHooks {
                notify: Box::new(|_flags| {
                    panic!("deliberate test panic inside notify callback");
                }),
                release: Box::new(move || {
                    rel_cb.fetch_add(1, Ordering::SeqCst);
                }),
            },
            None,
        )
        .map_err(|(e, _, _)| e)
        .unwrap();

        std::thread::sleep(Duration::from_millis(50));
        handle.release_caller_handle();

        let dl = Instant::now() + Duration::from_secs(2);
        while rel.load(Ordering::SeqCst) == 0 && Instant::now() < dl {
            std::thread::yield_now();
        }
        assert_eq!(rel.load(Ordering::SeqCst), 1, "panicking notify must still fire release once");
    }

    // E. Job stalled awaiting host Output ACK does NOT hold any compute permit!
    {
        let rt = AsyncMediaRuntime::global().unwrap();
        let dl_idle = Instant::now() + Duration::from_secs(2);
        while rt.active_jobs() > 0 && Instant::now() < dl_idle {
            std::thread::yield_now();
        }
        let baseline_permits = rt.available_compute_permits();
        let rel = Arc::new(AtomicUsize::new(0));
        let rel_cb = Arc::clone(&rel);
        let sess = MediaSession::create(
            Arc::clone(&engine),
            best_effort,
            OperationKind::Execute,
            canvas_req,
            &staged_bindings,
        )
        .unwrap();
        let handle = AsyncJobHandle::start(
            sess,
            AsyncJobHooks {
                notify: Box::new(|_| {}),
                release: Box::new(move || {
                    rel_cb.fetch_add(1, Ordering::SeqCst);
                }),
            },
            None,
        )
        .map_err(|(e, _, _)| e)
        .unwrap();

        // Wait until worker emits Output and parks waiting for host ACK!
        let dl_wait = Instant::now() + Duration::from_secs(2);
        while !matches!(handle.poll_event(), Some(IoEvent::Output { .. })) && Instant::now() < dl_wait
        {
            std::thread::yield_now();
        }
        assert!(matches!(handle.poll_event(), Some(IoEvent::Output { .. })));
        assert_eq!(
            rt.available_compute_permits(),
            baseline_permits,
            "stalled job awaiting host ACK must release its compute permit"
        );
        handle.release_caller_handle();
        let dl_end = Instant::now() + Duration::from_secs(2);
        while rel.load(Ordering::SeqCst) == 0 && Instant::now() < dl_end {
            std::thread::yield_now();
        }
        assert_eq!(rel.load(Ordering::SeqCst), 1);
    }
}

/// Inject an APP1 EXIF segment immediately after SOI (`FF D8`) in a baseline JPEG.
fn inject_jpeg_exif(jpeg: &[u8], exif_payload: &[u8]) -> Vec<u8> {
    assert!(jpeg.starts_with(&[0xFF, 0xD8]));
    let mut out = Vec::with_capacity(jpeg.len() + exif_payload.len() + 10);
    out.extend_from_slice(&jpeg[..2]);
    let seg_len = (2 + 6 + exif_payload.len()) as u16;
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&seg_len.to_be_bytes());
    out.extend_from_slice(b"Exif\0\0");
    out.extend_from_slice(exif_payload);
    out.extend_from_slice(&jpeg[2..]);
    out
}

/// Minimal valid little-endian TIFF EXIF with Orientation tag (`0x0112`).
fn make_exif_with_orientation(orientation: u16) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(b"II"); // little-endian
    b.extend_from_slice(&42u16.to_le_bytes());
    b.extend_from_slice(&8u32.to_le_bytes()); // IFD0 offset
    b.extend_from_slice(&1u16.to_le_bytes()); // 1 entry
    b.extend_from_slice(&0x0112u16.to_le_bytes()); // Orientation tag
    b.extend_from_slice(&3u16.to_le_bytes()); // SHORT
    b.extend_from_slice(&1u32.to_le_bytes()); // count 1
    b.extend_from_slice(&orientation.to_le_bytes());
    b.extend_from_slice(&0u16.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes()); // next IFD = 0
    b
}

#[test]
fn route_level_preserve_bytes_and_bounded_sdr_verify_qualification() {
    // 1. Produce a clean 16x16 JPEG via v2/execute, then inject upright (1) and rotated (6) EXIF!
    let mut c0 = Context::create().unwrap();
    c0.message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#)
        .1
        .unwrap();
    c0.add_output_buffer(1).unwrap();
    let (r0, res0) = c0.message(
        "v2/execute",
        br#"{
            "inputs": {},
            "outputs": {"jpg": 1},
            "pipeline": {
                "steps": [
                    {"canvas": {"width": 16, "height": 16, "fill": [200, 80, 60, 255]}},
                    {"encode": {"output": "jpg", "format": "jpeg"}}
                ]
            }
        }"#,
    );
    res0.unwrap();
    r0.assert_ok();
    let base_jpg = c0.take_output_buffer(1).unwrap();

    let upright_exif = make_exif_with_orientation(1);
    let upright_jpg = inject_jpeg_exif(&base_jpg, &upright_exif);

    // 1a. metadata="preserve_bytes" on upright JPEG preserves the exact EXIF bytes!
    let mut c_pb = Context::create().unwrap();
    c_pb.message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#)
        .1
        .unwrap();
    c_pb.add_copied_input_buffer(0, &upright_jpg).unwrap();
    c_pb.add_output_buffer(1).unwrap();
    let (r_pb, res_pb) = c_pb.message(
        "v2/execute",
        br#"{
            "inputs": {"src": 0},
            "outputs": {"dst": 1},
            "pipeline": {
                "steps": [
                    {"decode": {"input": "src", "orientation": "keep_stored"}},
                    {"encode": {"output": "dst", "format": "jpeg", "orientation": "preserve_stored", "metadata": "preserve_bytes"}}
                ]
            }
        }"#,
    );
    res_pb.unwrap();
    r_pb.assert_ok();
    let out_pb = c_pb.take_output_buffer(1).unwrap();
    assert!(
        out_pb.windows(upright_exif.len()).any(|w| w == upright_exif.as_slice()),
        "preserve_bytes output JPEG must contain exact unmodified EXIF payload"
    );

    // 1b. metadata="preserve_bytes" when orientation baking requires rewriting EXIF orientation (6 -> 1)
    //     MUST refuse honestly instead of silently rewriting bytes!
    let rotated_exif = make_exif_with_orientation(6);
    let rotated_jpg = inject_jpeg_exif(&base_jpg, &rotated_exif);
    let mut c_refuse = Context::create().unwrap();
    c_refuse
        .message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#)
        .1
        .unwrap();
    c_refuse.add_copied_input_buffer(0, &rotated_jpg).unwrap();
    c_refuse.add_output_buffer(1).unwrap();
    let (_, res_refuse) = c_refuse.message(
        "v2/execute",
        br#"{
            "inputs": {"src": 0},
            "outputs": {"dst": 1},
            "pipeline": {
                "steps": [
                    {"decode": {"input": "src", "orientation": "apply"}},
                    {"encode": {"output": "dst", "format": "jpeg", "orientation": "bake", "metadata": "preserve_bytes"}}
                ]
            }
        }"#,
    );
    assert!(
        res_refuse.is_err(),
        "preserve_bytes must refuse when orientation bake requires EXIF rewrite"
    );

    // 2. Bounded SDR Coding::Verify:
    // 2a. Default shared engine (no scorer registered) truthfully refuses Coding::Verify!
    let verify_req = br#"{
        "inputs": {"src": 0},
        "outputs": {"dst": 1},
        "pipeline": {
            "steps": [
                {"decode": {"input": "src"}},
                {"encode": {
                    "output": "dst",
                    "format": "jpeg",
                    "coding": {"verify": {"metric": "ssimulacra2", "value": 80.0}}
                }}
            ]
        }
    }"#;
    let mut c_v_def = Context::create().unwrap();
    c_v_def
        .message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#)
        .1
        .unwrap();
    c_v_def.add_copied_input_buffer(0, &base_jpg).unwrap();
    c_v_def.add_output_buffer(1).unwrap();
    let (_, res_v_def) = c_v_def.message("v2/execute", verify_req);
    assert!(
        res_v_def.is_err(),
        "default engine without registered MetricScorer must truthfully refuse Coding::Verify"
    );

    // 2b. With a score-only scalar MetricScorer registered on Engine, Coding::Verify converges cleanly!
    struct SimplePsnrSsim2Proxy;
    impl zencodecs::engine::MetricScorer for SimplePsnrSsim2Proxy {
        fn metric(&self) -> zencodecs::engine::Metric {
            zencodecs::engine::Metric::Ssimulacra2
        }
        fn revision(&self) -> &'static str {
            "test-scalar-v1"
        }
        fn comparison_format(&self) -> zenpixels::PixelFormat {
            zenpixels::PixelFormat::RGB8_SRGB
        }
        fn score(
            &self,
            reference: zenpixels::PixelView<'_>,
            candidate: zenpixels::PixelView<'_>,
            _resources: &zc::resources::JobResources,
        ) -> std::result::Result<f32, whereat::At<zencodecs::engine::EngineError>> {
            let mut sum_sq = 0.0f64;
            let mut n = 0usize;
            for y in 0..reference.height() {
                for (&a, &b) in reference.row(y).iter().zip(candidate.row(y).iter()) {
                    let d = f64::from(a) - f64::from(b);
                    sum_sq += d * d;
                    n += 1;
                }
            }
            let mse = sum_sq / (n.max(1) as f64);
            let score = (100.0 - mse.sqrt() * 2.0).clamp(0.0, 100.0);
            Ok(score as f32)
        }
    }

    let base_engine = shared_modern_engine().unwrap();
    let scoring_engine = Arc::new(
        (*base_engine)
            .clone()
            .with_metric_scorer(Arc::new(SimplePsnrSsim2Proxy), NonZeroU32::new(6).unwrap())
            .unwrap(),
    );
    let env_val: serde_json::Value = serde_json::from_slice(verify_req).unwrap();
    let job = zenpipe::canonical::json::parse_pipeline_value(
        &env_val["pipeline"],
        zenpipe::canonical::WireLimitConfig::default(),
    )
    .unwrap();
    let bindings = zenpipe::canonical::Bindings::new().with_bytes("src", &base_jpg).unwrap();
    let rt = zenpipe::canonical::PipelineRuntime::new(
        scoring_engine,
        zenpipe::canonical::HostServices::new()
            .with_workspace_policy(zencodecs::engine::WorkspacePolicy::BestEffort),
    );
    let prepared = rt.prepare(&job, bindings).unwrap();
    let job_res = prepared.run().unwrap();
    assert_eq!(job_res.outputs.len(), 1);
    assert!(job_res.outputs[0].bytes.starts_with(&[0xFF, 0xD8]));
}

#[test]
fn paused_activation_barrier_panicking_aborted_reply_and_manual_sync_deadlines() {
    let _rt_guard = ASYNC_RT_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let engine = shared_modern_engine().unwrap();
    let best_effort = zenpipe::canonical::HostServices::new()
        .with_workspace_policy(zencodecs::engine::WorkspacePolicy::BestEffort);
    let canvas_req = br#"{
        "inputs": {},
        "outputs": {"out": 1},
        "pipeline": {
            "steps": [
                {"canvas": {"width": 4, "height": 4, "fill": [10, 200, 150, 255]}},
                {"encode": {"output": "out", "format": "png"}}
            ]
        }
    }"#;
    let staged_bindings = [BindingSpec::forward_sink(1, Visibility::Staged)];

    // 1. Paused notification barrier: even when a pre-cancelled worker finishes and closes
    //    the notifier BEFORE `activator.activate()`, ZERO callbacks fire until `activate()`!
    {
        let notify_count = Arc::new(AtomicUsize::new(0));
        let nc_cb = Arc::clone(&notify_count);
        let rel = Arc::new(AtomicUsize::new(0));
        let rel_cb = Arc::clone(&rel);

        let stop = JobStop::new();
        assert!(stop.trip(StopCause::CallerCancel));
        let sess = MediaSession::create_with_stop(
            Arc::clone(&engine),
            best_effort.clone(),
            OperationKind::Execute,
            canvas_req,
            &staged_bindings,
            stop,
        )
        .unwrap();
        let (_handle, activator) = AsyncJobHandle::start_paused(
            sess,
            AsyncJobHooks {
                notify: Box::new(move |_| {
                    nc_cb.fetch_add(1, Ordering::SeqCst);
                }),
                release: Box::new(move || {
                    rel_cb.fetch_add(1, Ordering::SeqCst);
                }),
            },
            None,
        )
        .map_err(|(e, _, _)| e)
        .unwrap();

        // Give the fast pre-cancelled worker plenty of time to finish & call close().
        std::thread::sleep(Duration::from_millis(80));
        assert_eq!(
            notify_count.load(Ordering::SeqCst),
            0,
            "fast-finishing worker must NEVER bypass paused activation"
        );
        assert_eq!(
            rel.load(Ordering::SeqCst),
            0,
            "release must wait strictly until after notifier activates and drains"
        );

        activator.activate();
        let dl = Instant::now() + Duration::from_secs(2);
        while rel.load(Ordering::SeqCst) == 0 && Instant::now() < dl {
            std::thread::yield_now();
        }
        assert!(notify_count.load(Ordering::SeqCst) >= 1);
        assert_eq!(rel.load(Ordering::SeqCst), 1);
    }

    // 2. Panicking user stop hook triggered during Reply::Aborted must NEVER strand reply()!
    {
        let rel = Arc::new(AtomicUsize::new(0));
        let rel_cb = Arc::clone(&rel);
        let stop = JobStop::new();
        stop.on_stop(|_| {
            panic!("deliberate panic in user stop hook on Reply::Aborted");
        });
        let sess = MediaSession::create_with_stop(
            Arc::clone(&engine),
            best_effort.clone(),
            OperationKind::Execute,
            canvas_req,
            &staged_bindings,
            stop,
        )
        .unwrap();
        let handle = AsyncJobHandle::start(
            sess,
            AsyncJobHooks {
                notify: Box::new(|_| {}),
                release: Box::new(move || {
                    rel_cb.fetch_add(1, Ordering::SeqCst);
                }),
            },
            None,
        )
        .map_err(|(e, _, _)| e)
        .unwrap();

        let dl_ev = Instant::now() + Duration::from_secs(2);
        let ticket = loop {
            if let Some(IoEvent::Output { ticket, .. }) = handle.poll_event() {
                break ticket;
            }
            assert!(Instant::now() < dl_ev, "timed out waiting for Output");
            std::thread::yield_now();
        };
        // Calling reply(Aborted) trips the panicking stop hook; must return promptly (zero hang)!
        handle.reply(ticket, Reply::Aborted).unwrap();
        let dl = Instant::now() + Duration::from_secs(2);
        while rel.load(Ordering::SeqCst) == 0 && Instant::now() < dl {
            std::thread::yield_now();
        }
        assert_eq!(rel.load(Ordering::SeqCst), 1);
    }

    // 3. Manual MediaSession AND sync v2/execute enforce envelope `limits.timeout_ms`!
    {
        let timeout_req = br#"{
            "inputs": {},
            "outputs": {"out": 1},
            "limits": {"timeout_ms": 0},
            "pipeline": {
                "steps": [
                    {"canvas": {"width": 4, "height": 4, "fill": [10, 200, 150, 255]}},
                    {"encode": {"output": "out", "format": "png"}}
                ]
            }
        }"#;
        let mut s = MediaSession::create(
            Arc::clone(&engine),
            best_effort.clone(),
            OperationKind::Execute,
            timeout_req,
            &staged_bindings,
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(2));
        let ev = s.next(WorkQuota(4)).unwrap();
        assert_eq!(
            ev,
            IoEvent::Failed {
                category: ErrorCategory::OperationCancelled,
                cause: StopCause::Deadline,
            },
            "manual MediaSession must enforce envelope timeout_ms"
        );

        let mut c = Context::create().unwrap();
        c.message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#)
            .1
            .unwrap();
        c.add_output_buffer(1).unwrap();
        let (_, res) = c.message("v2/execute", timeout_req);
        assert!(res.is_err(), "sync v2/execute must enforce envelope timeout_ms");
    }

    // 4. Positive `timeout_ms` on buffered multi-node Execute is honestly refused with ActionNotSupported
    //    (pinned zenpipe cannot observe cooperative mid-DAG Pulse), while Inspect & Estimate enforce deadlines!
    {
        let buffered_deadline_req = br#"{
            "inputs": {},
            "outputs": {"out": 1},
            "limits": {"timeout_ms": 5000},
            "pipeline": {
                "steps": [
                    {"canvas": {"width": 4, "height": 4, "fill": [10, 200, 150, 255]}},
                    {"encode": {"output": "out", "format": "png"}}
                ]
            }
        }"#;
        let mut s = MediaSession::create(
            Arc::clone(&engine),
            best_effort.clone(),
            OperationKind::Execute,
            buffered_deadline_req,
            &staged_bindings,
        )
        .unwrap();
        let ev = s.next(WorkQuota(4)).unwrap();
        assert_eq!(
            ev,
            IoEvent::Failed {
                category: ErrorCategory::ActionNotSupported,
                cause: StopCause::None,
            },
            "buffered multi-node Execute must honestly refuse live timeout_ms"
        );

        let mut c = Context::create().unwrap();
        c.message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#)
            .1
            .unwrap();
        c.add_output_buffer(1).unwrap();
        let (_, res) = c.message("v2/execute", buffered_deadline_req);
        assert!(res.is_err(), "sync buffered v2/execute must refuse live timeout_ms");

        // Sync v2/estimate also enforces pre-expired timeout_ms!
        let mut c2 = Context::create().unwrap();
        c2.message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#)
            .1
            .unwrap();
        let est_timeout = br#"{
            "inputs": {},
            "outputs": {"out": 1},
            "limits": {"timeout_ms": 0},
            "pipeline": {
                "steps": [
                    {"canvas": {"width": 4, "height": 4, "fill": [10, 200, 150, 255]}},
                    {"encode": {"output": "out", "format": "png"}}
                ]
            }
        }"#;
        std::thread::sleep(Duration::from_millis(1));
        let (_, est_res) = c2.message("v2/estimate", est_timeout);
        assert!(est_res.is_err(), "sync v2/estimate must enforce deadline");
    }

    // 5. Decode-only Engine (demuxers + video decoders ONLY, zero muxers!) reports decode
    //    `supported: true` with `timing: "exact_rational"` (NEVER borrowing decode timing from
    //    a missing muxer!), while encode reports `supported: false` with `timing: "unsupported"`.
    {
        let set = zencodecs::bundles::register_media_demuxers(zc::CodecSet::new()).unwrap();
        let set = zencodecs::bundles::register_video_decoders(set).unwrap();
        let dec_only_engine = zencodecs::engine::Engine::new(set).unwrap();
        let cap = imageflow_core::media::report::build_capabilities_report_for_engine(&dec_only_engine);
        let ivf_vp8_dec = cap
            .media_routes
            .iter()
            .find(|r| r.container == "ivf" && r.video_codec == "vp8" && r.direction == "decode")
            .expect("ivf/vp8 decode route present");
        assert!(ivf_vp8_dec.supported, "decode-only registration must be supported");
        assert_eq!(
            ivf_vp8_dec.timing, "exact_rational",
            "decode route timing must come from decode, NEVER from missing muxer"
        );
        let ivf_vp8_enc = cap
            .media_routes
            .iter()
            .find(|r| r.container == "ivf" && r.video_codec == "vp8" && r.direction == "encode")
            .expect("ivf/vp8 encode route present");
        assert!(!ivf_vp8_enc.supported);
        assert_eq!(ivf_vp8_enc.timing, "unsupported");
    }

    // 6. Real split-WriteOffer muxer witness (R4 #3), slow ReadAtSource::length() absolute
    //    deadline witness (R4 #2), and Estimate unknown-descriptor honesty witness (R4 #3)!
    {
        struct SplitWriteOfferMuxer {
            inner: Box<dyn zencodec_media::container::DynMuxer + Send>,
            max_offer: usize,
            offers_served: Arc<AtomicUsize>,
        }
        impl zencodec_media::container::DynMuxer for SplitWriteOfferMuxer {
            fn begin_exact_timing(
                &mut self,
                tracks: &[zencodec_media::track::TrackInfo],
                hint: zencodec_media::container::MuxHint,
            ) -> std::result::Result<
                zencodec_media::container::MuxTiming,
                zencodec_media::session::SessionError<zencodec_media::session::ErasedError>,
            > {
                self.inner.begin(tracks, hint)
            }
            fn begin(
                &mut self,
                tracks: &[zencodec_media::track::TrackInfo],
                hint: zencodec_media::container::MuxHint,
            ) -> std::result::Result<
                zencodec_media::container::MuxTiming,
                zencodec_media::session::SessionError<zencodec_media::session::ErasedError>,
            > {
                self.inner.begin(tracks, hint)
            }
            fn write_packet(
                &mut self,
                packet: zencodec_media::packet::Packet,
                pulse: Option<&dyn zc::Pulse>,
            ) -> std::result::Result<
                (),
                zencodec_media::session::SessionError<zencodec_media::session::ErasedError>,
            > {
                self.inner.write_packet(packet, pulse)
            }
            fn pending_output(&self) -> &[u8] {
                let slice = self.inner.pending_output();
                &slice[..slice.len().min(self.max_offer)]
            }
            fn consume_output(&mut self, n: usize) {
                if n > 0 {
                    self.offers_served.fetch_add(1, Ordering::SeqCst);
                }
                self.inner.consume_output(n);
            }
            fn take_output(&mut self, into: &mut Vec<u8>) {
                self.inner.take_output(into);
            }
            fn finish(
                self: Box<Self>,
                pulse: Option<&dyn zc::Pulse>,
            ) -> std::result::Result<
                zencodec_media::container::MuxFinish,
                zencodec_media::session::SessionError<zencodec_media::session::ErasedError>,
            > {
                self.inner.finish(pulse)
            }
            fn as_bounded(&self) -> Option<&dyn zencodec_media::container::BoundedMuxer> {
                Some(self)
            }
            fn as_bounded_mut(&mut self) -> Option<&mut dyn zencodec_media::container::BoundedMuxer> {
                Some(self)
            }
        }
        impl zencodec_media::container::BoundedMuxer for SplitWriteOfferMuxer {
            fn admitted_offer(&self) -> &zencodec_media::container::MuxOffer {
                self.inner
                    .as_bounded()
                    .expect("inner ivf muxer is bounded")
                    .admitted_offer()
            }
            fn try_write_packet(
                &mut self,
                packet: &mut Option<zencodec_media::packet::Packet>,
                pulse: Option<&dyn zc::Pulse>,
            ) -> std::result::Result<
                zencodec_media::container::PacketWrite,
                zencodec_media::session::SessionError<zencodec_media::session::ErasedError>,
            > {
                self.inner
                    .as_bounded_mut()
                    .expect("inner ivf muxer is bounded")
                    .try_write_packet(packet, pulse)
            }
        }
        struct SplitWriteOfferMuxFactory {
            inner: Arc<dyn zencodec_media::registry::MuxFactory>,
            max_offer: usize,
            offers_served: Arc<AtomicUsize>,
        }
        impl zencodec_media::registry::MuxFactory for SplitWriteOfferMuxFactory {
            fn quote(
                &self,
                need: &zencodec_media::registry::MuxNeeds,
                run: &zc::resources::JobResources,
            ) -> std::result::Result<
                zencodec_media::registry::MuxPlan,
                zencodec_media::registry::MediaError,
            > {
                let mut adj = need.clone();
                adj.timing = zencodec_media::registry::TimingPolicy::BestEffort;
                self.inner.quote(&adj, run)
            }
            fn open(
                &self,
                plan: &zencodec_media::registry::MuxPlan,
                run: &zc::resources::JobResources,
            ) -> std::result::Result<
                Box<dyn zencodec_media::container::DynMuxer + Send>,
                zencodec_media::registry::MediaError,
            > {
                let inner = self.inner.open(plan, run)?;
                Ok(Box::new(SplitWriteOfferMuxer {
                    inner,
                    max_offer: self.max_offer,
                    offers_served: Arc::clone(&self.offers_served),
                }))
            }
        }

        let gif = make_two_frame_gif();
        let mut fc = Context::create().unwrap();
        fc.message(
            "v2/configure_context",
            br#"{"input_registration":"deferred","workspace_policy":"best_effort"}"#,
        )
        .1
        .unwrap();
        fc.add_input_read_at(1, Arc::new(SliceReadAtSource::new(Arc::new(gif))))
            .unwrap();
        fc.add_output_buffer(2).unwrap();
        fc.message(
            "v2/execute",
            br#"{
                "inputs": {"in": 1},
                "outputs": {"out": 2},
                "pipeline": {
                    "steps": [
                        {"decode": {"input": "in"}},
                        {"encode": {
                            "output": "out",
                            "format": "mp4",
                            "video_codec": "av1",
                            "publication": "verified",
                            "sequence": {"preserve": {"timing": "exact", "loops": {"set": {"plays": 1}}}}
                        }}
                    ]
                }
            }"#,
        )
        .1
        .unwrap();
        let mp4_bytes = fc.take_output_buffer(2).unwrap();

        // 6a. Force the IVF muxer itself to emit the 32-byte header across 4 separate 8-byte
        //     `WriteOffer::Append` offers in `incremental` mode on a `ForwardSink`!
        let real_ivf_mux = Arc::clone(
            &engine
                .codecs()
                .factories::<zencodec_media::registry::MuxRole>(
                    &zencodec_media::registry::ContainerId::Ivf,
                )
                .next()
                .expect("ivf muxer")
                .factory,
        );
        let offers_served = Arc::new(AtomicUsize::new(0));
        let split_mux: Arc<dyn zencodec_media::registry::MuxFactory> =
            Arc::new(SplitWriteOfferMuxFactory {
                inner: real_ivf_mux,
                max_offer: 8,
                offers_served: Arc::clone(&offers_served),
            });
        let set = zencodecs::bundles::register_media_demuxers(zc::CodecSet::new()).unwrap();
        let set = zencodecs::bundles::register_video_decoders(set).unwrap();
        let set = zencodecs::bundles::register_video_encoders(set).unwrap();
        let set = set
            .with_role::<zencodec_media::registry::MuxRole>(
                [zencodec_media::registry::ContainerId::Ivf],
                zc::NamedFactory::new(
                    zc::RegistrationId::new("test.split-ivf").unwrap(),
                    "split-ivf-test",
                    split_mux,
                ),
            )
            .unwrap();
        let split_engine = Arc::new(zencodecs::engine::Engine::new(set).unwrap());

        let transcode_split_incremental = br#"{
            "inputs": {"in": 1},
            "outputs": {"out": 2},
            "limits": {"timeout_ms": 30000, "max_memory_bytes": 67108864},
            "pipeline": {
                "steps": [
                    {"decode": {"input": "in"}},
                    {"encode": {
                        "output": "out",
                        "format": "ivf",
                        "video_codec": "vp8",
                        "publication": "incremental",
                        "sequence": {"preserve": {"timing": "exact", "loops": {"set": {"plays": 1}}}}
                    }}
                ]
            }
        }"#;
        let bindings = [
            BindingSpec::read_at_source(1, true),
            BindingSpec::forward_sink(2, Visibility::Incremental),
        ];
        let mut s = MediaSession::create(
            split_engine,
            best_effort.clone(),
            OperationKind::Execute,
            transcode_split_incremental,
            &bindings,
        )
        .unwrap();
        let mut out_bytes = Vec::new();
        let json_out = loop {
            match s.next(WorkQuota(4)).unwrap() {
                IoEvent::Yield => continue,
                IoEvent::NeedInput { ticket, request, .. } => match request {
                    IoInput::Len => s.reply(ticket, Reply::Length(mp4_bytes.len() as u64)).unwrap(),
                    IoInput::AtLeast { next_offset, max_transfer, .. } => {
                        let start = (next_offset as usize).min(mp4_bytes.len());
                        let end = (start + max_transfer as usize).min(mp4_bytes.len());
                        if start >= mp4_bytes.len() {
                            s.reply(ticket, Reply::End { total_len: Some(mp4_bytes.len() as u64) }).unwrap();
                        } else {
                            s.reply(ticket, Reply::Bytes(&mp4_bytes[start..end])).unwrap();
                        }
                    }
                    IoInput::Range { offset, len } => {
                        let start = (offset as usize).min(mp4_bytes.len());
                        let end = (start + len as usize).min(mp4_bytes.len());
                        if start >= mp4_bytes.len() {
                            s.reply(ticket, Reply::End { total_len: Some(mp4_bytes.len() as u64) }).unwrap();
                        } else {
                            s.reply(ticket, Reply::Bytes(&mp4_bytes[start..end])).unwrap();
                        }
                    }
                },
                IoEvent::Output { ticket, len, .. } => {
                    let mut tmp = vec![0u8; len as usize];
                    let n = s.copy_pending(ticket, &mut tmp).unwrap();
                    out_bytes.extend_from_slice(&tmp[..n]);
                    s.reply(ticket, Reply::Ack { bytes: n as u64 }).unwrap();
                }
                IoEvent::Patch { ticket, offset, len, .. } => {
                    let mut tmp = vec![0u8; len as usize];
                    let n = s.copy_pending(ticket, &mut tmp).unwrap();
                    let off = offset as usize;
                    out_bytes[off..off + n].copy_from_slice(&tmp[..n]);
                    s.reply(ticket, Reply::Ack { bytes: n as u64 }).unwrap();
                }
                IoEvent::Flush { ticket, .. } | IoEvent::Commit { ticket } => {
                    s.reply(ticket, Reply::Ack { bytes: 0 }).unwrap();
                }
                IoEvent::Complete => break s.result_json().unwrap().to_vec(),
                IoEvent::Failed { category, cause } => {
                    panic!(
                        "split-offer incremental transcode failed ({category:?}/{cause:?}): {}",
                        String::from_utf8_lossy(s.result_json().unwrap_or(b""))
                    );
                }
            }
        };
        assert!(
            offers_served.load(Ordering::SeqCst) >= 4,
            "muxer must have emitted 32-byte IVF header across >=4 separate 8-byte WriteOffers"
        );
        let v: serde_json::Value = serde_json::from_slice(&json_out).unwrap();
        assert_eq!(v["outputs"][0]["width"], 16, "must recover exact width across split WriteOffers");
        assert_eq!(v["outputs"][0]["height"], 16, "must recover exact height across split WriteOffers");
        assert!(
            v.get("warnings").is_none_or(|w| w.as_array().is_some_and(Vec::is_empty)),
            "resolved output geometry must produce zero geometry warnings"
        );
        assert!(!out_bytes.is_empty());

        // 6b. Slow `ReadAtSource::length()` consuming the deadline MUST fail sync ReadAt
        //     `v2/execute` (NEVER restarting the deadline after `src.length()`!).
        struct SlowLengthSource {
            bytes: Vec<u8>,
            delay: Duration,
        }
        impl ReadAtSource for SlowLengthSource {
            fn read_at(&self, offset: u64, dst: &mut [u8]) -> Result<usize> {
                let off = usize::try_from(offset).unwrap_or(self.bytes.len());
                if off >= self.bytes.len() {
                    return Ok(0);
                }
                let n = (self.bytes.len() - off).min(dst.len());
                dst[..n].copy_from_slice(&self.bytes[off..off + n]);
                Ok(n)
            }
            fn length(&self) -> Result<Option<u64>> {
                std::thread::sleep(self.delay);
                Ok(Some(self.bytes.len() as u64))
            }
        }
        let mut c_slow = Context::create().unwrap();
        c_slow
            .message(
                "v2/configure_context",
                br#"{"input_registration":"deferred","workspace_policy":"best_effort"}"#,
            )
            .1
            .unwrap();
        c_slow
            .add_input_read_at(
                1,
                Arc::new(SlowLengthSource {
                    bytes: mp4_bytes.clone(),
                    delay: Duration::from_millis(30),
                }),
            )
            .unwrap();
        c_slow.add_output_buffer(2).unwrap();
        let (_, slow_res) = c_slow.message(
            "v2/execute",
            br#"{
                "inputs": {"in": 1},
                "outputs": {"out": 2},
                "limits": {"timeout_ms": 10},
                "pipeline": {
                    "steps": [
                        {"decode": {"input": "in"}},
                        {"encode": {
                            "output": "out",
                            "format": "ivf",
                            "video_codec": "vp8",
                            "publication": "verified",
                            "sequence": {"preserve": {"timing": {"quantize_endpoints":{"max_boundary_error":{"numerator":1,"denominator":1000}}}, "loops": {"set": {"plays": 1}}}}
                        }}
                    ]
                }
            }"#,
        );
        assert!(
            slow_res.is_err(),
            "sync ReadAt v2/execute must NOT restart deadline when src.length() consumes timeout_ms"
        );

        // 6c. `v2/estimate` on non-direct video graph reports `needs_patch: null` + explicit
        //     warning instead of fabricating a synthetic 64x64 @ 30 Hz mux quote!
        let mut c_est = Context::create().unwrap();
        c_est
            .message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#)
            .1
            .unwrap();
        let (est_resp, est_res) = c_est.message(
            "v2/estimate",
            br#"{
                "inputs": {},
                "outputs": {"out": 1},
                "pipeline": {
                    "steps": [
                        {"canvas": {"width": 16, "height": 16, "fill": [10, 20, 30, 255]}},
                        {"encode": {
                            "output": "out",
                            "format": "mp4",
                            "video_codec": "av1",
                            "publication": "verified",
                            "sequence": {"preserve": {"timing": "exact", "loops": {"set": {"plays": 1}}}}
                        }}
                    ]
                }
            }"#,
        );
        est_res.unwrap();
        let est_v: serde_json::Value = serde_json::from_slice(&est_resp.response_json).unwrap();
        assert!(
            est_v["data"]["startup_dependencies"]["needs_patch"].is_null(),
            "estimate must report needs_patch as null when concrete output video track descriptor is unknown"
        );
        assert!(
            !est_v["data"]["warnings"].as_array().unwrap().is_empty(),
            "estimate must warn when video output patch requirement cannot be statically determined"
        );
    }

    // 7. Cancellation racing `Reply::IoError` on `AsyncJobHandle` publishes terminal `Failed`
    //    and exact `StreamErrorReportV2` receipt BEFORE returning `Err(Closed)` (zero stale event)!
    {
        let rel = Arc::new(AtomicUsize::new(0));
        let rel_cb = Arc::clone(&rel);
        let sess = MediaSession::create(
            Arc::clone(&engine),
            best_effort,
            OperationKind::Execute,
            canvas_req,
            &staged_bindings,
        )
        .unwrap();
        let handle = AsyncJobHandle::start(
            sess,
            AsyncJobHooks {
                notify: Box::new(|_| {}),
                release: Box::new(move || {
                    rel_cb.fetch_add(1, Ordering::SeqCst);
                }),
            },
            None,
        )
        .map_err(|(e, _, _)| e)
        .unwrap();

        let dl_ev = Instant::now() + Duration::from_secs(2);
        let ticket = loop {
            if let Some(IoEvent::Output { ticket, .. }) = handle.poll_event() {
                break ticket;
            }
            assert!(Instant::now() < dl_ev, "timed out waiting for Output");
            std::thread::yield_now();
        };
        // Trip cancellation BEFORE delivering IoError so `session.reply` returns `Err(Closed)`.
        handle.cancel();
        let err = handle.reply(
            ticket,
            Reply::IoError { accepted_prefix: None, diagnostic: b"simulated disk full" },
        );
        assert_eq!(err, Err(SessionError::Closed));
        // IMMEDIATELY on the very next poll_event() (zero sleep!), terminal Failed & receipt MUST be visible!
        assert!(
            matches!(
                handle.poll_event(),
                Some(IoEvent::Failed {
                    category: ErrorCategory::OperationCancelled,
                    cause: StopCause::CallerCancel,
                })
            ),
            "terminal Failed must already replace stale Output before reply(IoError) returns Closed"
        );
        let receipt = handle
            .result_json_cloned()
            .expect("terminal StreamErrorReportV2 receipt must already be published");
        let rv: serde_json::Value = serde_json::from_slice(&receipt).unwrap();
        assert_eq!(rv["success"], false);
        assert_eq!(rv["publications"][0]["slot"], "out");

        let dl = Instant::now() + Duration::from_secs(2);
        while rel.load(Ordering::SeqCst) == 0 && Instant::now() < dl {
            std::thread::yield_now();
        }
        assert_eq!(rel.load(Ordering::SeqCst), 1);
    }
}
