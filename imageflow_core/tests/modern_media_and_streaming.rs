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
    host_services_from_context, shared_modern_engine, AsyncJobHandle, AsyncJobHooks, BindingSpec,
    CopyPendingError, IoEvent, IoInput, JobStop, MediaSession, OperationKind, ReadAtSource, Reply,
    SessionError, SliceReadAtSource, StopCause, Visibility, WorkQuota,
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
                            session.reply(ticket, Reply::Bytes(&input[off..off + take])).unwrap();
                        }
                    }
                    IoInput::Range { offset, len } => {
                        let off = offset as usize;
                        if off >= input.len() {
                            session
                                .reply(ticket, Reply::End { total_len: Some(input.len() as u64) })
                                .unwrap();
                        } else {
                            let take =
                                in_chunk_size.min(len as usize).min(input.len() - off).max(1);
                            session.reply(ticket, Reply::Bytes(&input[off..off + take])).unwrap();
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
    if cfg!(feature = "iqa") {
        let vm: Vec<&str> = cap_json["data"]["verify_metrics"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(vm, vec!["zensim_a", "ssimulacra2", "butteraugli_max"]);
        let im = cap_json["data"]["iqa_metrics"].as_array().unwrap();
        assert_eq!(im.len(), 3);
        assert_eq!(im[0]["metric"], "zensim_a");
        assert_eq!(im[0]["direction"], "higher_is_better");
        assert_eq!(im[0]["metric_id"]["backend"], "cpu");
    } else {
        assert_eq!(cap_json["data"]["verify_metrics"].as_array().unwrap().len(), 0);
        assert_eq!(cap_json["data"]["iqa_metrics"].as_array().unwrap().len(), 0);
    }
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
        .find(|r| {
            r["container"] == "mp4" && r["video_codec"] == "av1" && r["direction"] == "decode"
        })
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
    c_insp.message("v2/configure_context", br#"{"input_registration":"deferred"}"#).1.unwrap();
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
        .message(
            "v2/configure_context",
            br#"{"input_registration":"deferred","workspace_policy":"best_effort"}"#,
        )
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
        &[BindingSpec::read_at_source(0, true), BindingSpec::forward_sink(1, Visibility::Staged)],
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
    let bindings =
        [BindingSpec::read_at_source(0, true), BindingSpec::forward_sink(1, Visibility::Staged)];

    // 5a. Sync v2/execute directly on ReadAtSource MP4/AV1 -> WebM/VP8
    let mut c_sync = Context::create().unwrap();
    c_sync
        .message(
            "v2/configure_context",
            br#"{"input_registration":"deferred","workspace_policy":"best_effort"}"#,
        )
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
                IoInput::Len => {
                    handle.reply(ticket, Reply::Length(mp4_av1_bytes.len() as u64)).unwrap()
                }
                IoInput::AtLeast { next_offset, max_transfer, .. } => {
                    let off = next_offset as usize;
                    if off >= mp4_av1_bytes.len() {
                        handle
                            .reply(
                                ticket,
                                Reply::End { total_len: Some(mp4_av1_bytes.len() as u64) },
                            )
                            .unwrap();
                    } else {
                        let take = (max_transfer as usize).min(mp4_av1_bytes.len() - off);
                        handle
                            .reply(ticket, Reply::Bytes(&mp4_av1_bytes[off..off + take]))
                            .unwrap();
                    }
                }
                IoInput::Range { offset, len } => {
                    let off = offset as usize;
                    if off >= mp4_av1_bytes.len() {
                        handle
                            .reply(
                                ticket,
                                Reply::End { total_len: Some(mp4_av1_bytes.len() as u64) },
                            )
                            .unwrap();
                    } else {
                        let take = (len as usize).min(mp4_av1_bytes.len() - off);
                        handle
                            .reply(ticket, Reply::Bytes(&mp4_av1_bytes[off..off + take]))
                            .unwrap();
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
            let (ir, ires) = sc.message("v2/inspect", br#"{"inputs":{"clip":0},"input":"clip"}"#);
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
    s1.reply(t2, Reply::IoError { accepted_prefix: Some(3), diagnostic: b"disk full" }).unwrap();
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
        assert_eq!(
            rel.load(Ordering::SeqCst),
            1,
            "reentrant cancel inside notify must release once"
        );
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
        assert!(
            completed.load(Ordering::SeqCst),
            "reentrant reply inside notify must reach Complete"
        );
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
        while !matches!(handle.poll_event(), Some(IoEvent::Output { .. }))
            && Instant::now() < dl_wait
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
    c0.message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#).1.unwrap();
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
    c_pb.message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#).1.unwrap();
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
    c_refuse.message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#).1.unwrap();
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
    // 2a. Without `iqa` feature, default shared engine truthfully refuses Coding::Verify;
    //     with `iqa` enabled, default shared engine converges out of the box under Strict!
    let verify_req = br#"{
        "inputs": {"src": 0},
        "outputs": {"dst": 1},
        "pipeline": {
            "steps": [
                {"decode": {"input": "src"}},
                {"encode": {
                    "output": "dst",
                    "format": "jpeg",
                    "coding": {"verify": {"metric": "ssimulacra2", "value": 70.0}}
                }}
            ]
        }
    }"#;
    let mut c_v_def = Context::create().unwrap();
    c_v_def.add_copied_input_buffer(0, &base_jpg).unwrap();
    c_v_def.add_output_buffer(1).unwrap();
    let (resp_v_def, res_v_def) = c_v_def.message("v2/execute", verify_req);
    if cfg!(feature = "iqa") {
        res_v_def.expect("iqa-enabled shared engine must converge Coding::Verify under Strict");
        let v_json: serde_json::Value = serde_json::from_slice(&resp_v_def.response_json).unwrap();
        assert_eq!(v_json["data"]["outputs"][0]["verified_metric"], "ssimulacra2");
        assert!(v_json["data"]["outputs"][0]["verified_score_f64"].as_f64().unwrap() >= 70.0);
    } else {
        assert!(
            res_v_def.is_err(),
            "default engine without registered MetricScorer must truthfully refuse Coding::Verify"
        );
    }

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

    let custom_set = zencodecs::bundles::register_all_enabled(zc::CodecSet::new()).unwrap();
    let scoring_engine = Arc::new(
        zencodecs::engine::Engine::new(custom_set)
            .unwrap()
            .with_color_services(zencodecs::bundles::moxcms_color_services())
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
        c.message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#).1.unwrap();
        c.add_output_buffer(1).unwrap();
        let (_, res) = c.message("v2/execute", timeout_req);
        assert!(res.is_err(), "sync v2/execute must enforce envelope timeout_ms");
    }

    // 4. Positive `timeout_ms` on buffered multi-node Execute is NOW cooperatively enforced via
    //    `PreparedJob::run_with_pulse`, while unrepresentable overflow deadlines fail cleanly with InvalidArgument!
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
        let mut got_complete = false;
        let mut out_bytes = Vec::new();
        for _ in 0..32 {
            match s.next(WorkQuota(4)).unwrap() {
                IoEvent::Yield => continue,
                IoEvent::Output { ticket, len, .. } => {
                    let mut chunk = vec![0u8; len as usize];
                    let n = s.copy_pending(ticket, &mut chunk).unwrap();
                    out_bytes.extend_from_slice(&chunk[..n]);
                    s.reply(ticket, Reply::Ack { bytes: n as u64 }).unwrap();
                }
                IoEvent::Flush { ticket, .. } | IoEvent::Commit { ticket } => {
                    s.reply(ticket, Reply::Ack { bytes: 0 }).unwrap();
                }
                IoEvent::Complete => {
                    got_complete = true;
                    break;
                }
                other => {
                    panic!("unexpected event on buffered Execute with valid timeout_ms: {other:?}")
                }
            }
        }
        assert!(got_complete && out_bytes.starts_with(b"\x89PNG"));

        let mut c = Context::create().unwrap();
        c.message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#).1.unwrap();
        c.add_output_buffer(1).unwrap();
        let (_, res) = c.message("v2/execute", buffered_deadline_req);
        res.expect(
            "sync buffered v2/execute with generous timeout_ms must succeed via run_with_pulse",
        );

        // Unrepresentable overflow deadline (`u64::MAX` ms) is rejected cleanly as InvalidArgument!
        let overflow_req = br#"{
            "inputs": {},
            "outputs": {"out": 1},
            "limits": {"timeout_ms": 18446744073709551615},
            "pipeline": {
                "steps": [
                    {"canvas": {"width": 4, "height": 4, "fill": [10, 200, 150, 255]}},
                    {"encode": {"output": "out", "format": "png"}}
                ]
            }
        }"#;
        assert!(
            matches!(
                MediaSession::create(
                    Arc::clone(&engine),
                    best_effort.clone(),
                    OperationKind::Execute,
                    overflow_req,
                    &staged_bindings,
                ),
                Err(SessionError::InvalidArgument(_))
            ),
            "MediaSession must reject unrepresentable timeout_ms overflow"
        );
        let mut c_ovf = Context::create().unwrap();
        c_ovf.add_output_buffer(1).unwrap();
        let (_, ovf_res) = c_ovf.message("v2/execute", overflow_req);
        assert!(
            ovf_res.is_err(),
            "sync v2/execute must reject unrepresentable timeout_ms overflow"
        );

        // Sync v2/estimate also enforces pre-expired timeout_ms!
        let mut c2 = Context::create().unwrap();
        c2.message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#).1.unwrap();
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
        let cap =
            imageflow_core::media::report::build_capabilities_report_for_engine(&dec_only_engine);
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
            fn as_bounded_mut(
                &mut self,
            ) -> Option<&mut dyn zencodec_media::container::BoundedMuxer> {
                Some(self)
            }
        }
        impl zencodec_media::container::BoundedMuxer for SplitWriteOfferMuxer {
            fn admitted_offer(&self) -> &zencodec_media::container::MuxOffer {
                self.inner.as_bounded().expect("inner ivf muxer is bounded").admitted_offer()
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
        fc.add_input_read_at(1, Arc::new(SliceReadAtSource::new(Arc::new(gif)))).unwrap();
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
                            s.reply(ticket, Reply::End { total_len: Some(mp4_bytes.len() as u64) })
                                .unwrap();
                        } else {
                            s.reply(ticket, Reply::Bytes(&mp4_bytes[start..end])).unwrap();
                        }
                    }
                    IoInput::Range { offset, len } => {
                        let start = (offset as usize).min(mp4_bytes.len());
                        let end = (start + len as usize).min(mp4_bytes.len());
                        if start >= mp4_bytes.len() {
                            s.reply(ticket, Reply::End { total_len: Some(mp4_bytes.len() as u64) })
                                .unwrap();
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
        assert_eq!(
            v["outputs"][0]["width"], 16,
            "must recover exact width across split WriteOffers"
        );
        assert_eq!(
            v["outputs"][0]["height"], 16,
            "must recover exact height across split WriteOffers"
        );
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
        c_est.message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#).1.unwrap();
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

#[cfg(feature = "iqa")]
#[test]
fn embedded_iqa_capabilities_strict_verify_aimfor_and_v2_compare() {
    // Helper: build a smooth textured 64x48 RGB8 PNG + lossy JPEG + smaller 32x32 PNG + translucent PNG.
    let make_textured_png = |w: u32, h: u32, translucent: bool| -> Vec<u8> {
        let mut raw = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            let yf = y as f32 / (h.max(1) as f32);
            for x in 0..w {
                let xf = x as f32 / (w.max(1) as f32);
                let wave = ((xf * 10.0 + yf * 6.0).sin() * 0.5 + 0.5) * 160.0;
                let r = (40.0 + 170.0 * xf).clamp(0.0, 255.0) as u8;
                let g = (35.0 + 170.0 * yf).clamp(0.0, 255.0) as u8;
                let b = (30.0 + wave).clamp(0.0, 255.0) as u8;
                let a = if translucent { 128 } else { 255 };
                raw.extend_from_slice(&[r, g, b, a]);
            }
        }
        let buf =
            zenpixels::PixelBuf::from_bytes(raw, w, h, zenpixels::PixelFormat::RGBA8_SRGB).unwrap();
        let engine = shared_modern_engine().unwrap();
        let done = engine
            .encode_pixels(
                &zencodecs::engine::OutputIntent::format(zc::ImageFormat::Png),
                buf.as_view(),
                &zc::Metadata::none(),
                None,
                &zencodecs::engine::Run::new(),
            )
            .unwrap();
        done.output.into_vec()
    };

    let ref_png = make_textured_png(64, 48, false);
    let small_png = make_textured_png(32, 32, false);
    let alpha_png = make_textured_png(64, 48, true);
    let anim_gif = make_two_frame_gif();

    // 1. Produce a lossy JPEG via v2/execute Coding::Verify (zensim_a) under DEFAULT WorkspacePolicy::Strict!
    let mut c_ver = Context::create().unwrap();
    c_ver.add_copied_input_buffer(0, &ref_png).unwrap();
    c_ver.add_output_buffer(1).unwrap();
    let (v_resp, v_res) = c_ver.message(
        "v2/execute",
        br#"{
            "inputs": {"in": 0},
            "outputs": {"out": 1},
            "pipeline": {
                "steps": [
                    {"decode": {"input": "in"}},
                    {"encode": {
                        "output": "out",
                        "format": "jpeg",
                        "coding": {"verify": {"metric": "zensim_a", "value": 80.0}}
                    }}
                ]
            }
        }"#,
    );
    v_res.expect("v2/execute Coding::Verify(zensim_a) must converge under default Strict policy");
    let v_json: serde_json::Value = serde_json::from_slice(&v_resp.response_json).unwrap();
    assert_eq!(v_json["data"]["outputs"][0]["verified_metric"], "zensim_a");
    assert_eq!(v_json["data"]["outputs"][0]["verified_revision"], "zensim-a.r7");
    let zscore = v_json["data"]["outputs"][0]["verified_score_f64"].as_f64().unwrap();
    assert!((80.0..=100.0).contains(&zscore), "unexpected zensim_a score {zscore}");
    assert!(v_json["data"]["outputs"][0]["verify_attempts"].as_u64().unwrap() >= 1);
    let cand_jpg = c_ver.take_output_buffer(1).unwrap();

    // 2. Unreachable Verify target returns typed ActionNotSupported (TargetUnmet)!
    let mut c_unmet = Context::create().unwrap();
    c_unmet.add_copied_input_buffer(0, &ref_png).unwrap();
    c_unmet.add_output_buffer(1).unwrap();
    let (_, unmet_res) = c_unmet.message(
        "v2/execute",
        br#"{
            "inputs": {"in": 0},
            "outputs": {"out": 1},
            "pipeline": {
                "steps": [
                    {"decode": {"input": "in"}},
                    {"encode": {
                        "output": "out",
                        "format": "jpeg",
                        "coding": {"verify": {"metric": "zensim_a", "value": 100.0}}
                    }}
                ]
            }
        }"#,
    );
    let unmet_err = unmet_res.expect_err("impossible zensim_a=100 on lossy JPEG must fail");
    assert_eq!(
        unmet_err.category(),
        ErrorCategory::ActionNotSupported,
        "TargetUnmet must map to ErrorCategory::ActionNotSupported, got {unmet_err:?}"
    );

    // 3. Coding::AimFor exposes aim_metric/aim_target/aim_revision/aim_estimated_quality with no verified_score.
    let mut c_aim = Context::create().unwrap();
    c_aim.add_copied_input_buffer(0, &ref_png).unwrap();
    c_aim.add_output_buffer(1).unwrap();
    let (aim_resp, aim_res) = c_aim.message(
        "v2/execute",
        br#"{
            "inputs": {"in": 0},
            "outputs": {"out": 1},
            "pipeline": {
                "steps": [
                    {"decode": {"input": "in"}},
                    {"encode": {
                        "output": "out",
                        "format": "jpeg",
                        "coding": {"aim_for": {"metric": "zensim_a", "value": 85.0}}
                    }}
                ]
            }
        }"#,
    );
    aim_res.unwrap();
    let aim_json: serde_json::Value = serde_json::from_slice(&aim_resp.response_json).unwrap();
    assert_eq!(aim_json["data"]["outputs"][0]["aim_metric"], "zensim_a");
    assert_eq!(aim_json["data"]["outputs"][0]["aim_target"], 85.0);
    assert!(aim_json["data"]["outputs"][0]["aim_revision"].is_string());
    assert!(aim_json["data"]["outputs"][0]["aim_estimated_quality"].is_number());
    assert!(aim_json["data"]["outputs"][0].get("verified_score").is_none());

    // 4. v2/compare default (omitted `metrics` -> ["zensim_a"]) on identical PNG = perfect 100.0!
    let mut c_cmp = Context::create().unwrap();
    c_cmp.add_copied_input_buffer(0, &ref_png).unwrap();
    c_cmp.add_copied_input_buffer(1, &cand_jpg).unwrap();
    c_cmp.add_copied_input_buffer(2, &small_png).unwrap();
    c_cmp.add_copied_input_buffer(3, &anim_gif).unwrap();
    c_cmp.add_copied_input_buffer(4, &alpha_png).unwrap();

    let (id_resp, id_res) =
        c_cmp.message("v2/compare", br#"{"reference_io_id": 0, "candidate_io_id": 0}"#);
    id_res.unwrap();
    let id_json: serde_json::Value = serde_json::from_slice(&id_resp.response_json).unwrap();
    assert_eq!(id_json["data"]["version"], 2);
    assert_eq!(id_json["data"]["width"], 64);
    assert_eq!(id_json["data"]["height"], 48);
    assert_eq!(id_json["data"]["comparison_format"], "rgba8_srgb");
    let id_results = id_json["data"]["results"].as_array().unwrap();
    assert_eq!(id_results.len(), 1);
    assert_eq!(id_results[0]["metric"], "zensim_a");
    assert_eq!(id_results[0]["score"].as_f64().unwrap(), 100.0);
    assert_eq!(id_results[0]["direction"], "higher_is_better");
    assert_eq!(id_results[0]["perfect"], 100.0);
    assert_eq!(id_results[0]["revision"], "zensim-a.r7");
    assert_eq!(id_results[0]["metric_id"]["family"], "zensim");

    // 5. v2/compare multi-metric preserves exact request order & scores real distortion!
    let (multi_resp, multi_res) = c_cmp.message(
        "v2/compare",
        br#"{
            "reference_io_id": 0,
            "candidate_io_id": 1,
            "metrics": ["butteraugli_max", "ssimulacra2", "zensim_a"]
        }"#,
    );
    multi_res.unwrap();
    let mj: serde_json::Value = serde_json::from_slice(&multi_resp.response_json).unwrap();
    let mres = mj["data"]["results"].as_array().unwrap();
    assert_eq!(mres.len(), 3);
    assert_eq!(mres[0]["metric"], "butteraugli_max");
    assert_eq!(mres[0]["direction"], "lower_is_better");
    assert_eq!(mres[0]["perfect"], 0.0);
    assert!(mres[0]["score"].as_f64().unwrap() > 0.0);
    assert_eq!(mres[1]["metric"], "ssimulacra2");
    assert!(mres[1]["score"].as_f64().unwrap() > 50.0);
    assert_eq!(mres[2]["metric"], "zensim_a");
    assert!(mres[2]["score"].as_f64().unwrap() >= 80.0);

    // 6. v2/compare strict error contracts:
    //    empty metrics, duplicate metrics, unknown metric, unknown field, dim mismatch, animation,
    //    translucent Butteraugli refusal (vs ZensimA pass!), tight memory budget, overflow timeout.
    assert!(c_cmp
        .message("v2/compare", br#"{"reference_io_id":0,"candidate_io_id":1,"metrics":[]}"#)
        .1
        .is_err());
    assert!(c_cmp
        .message(
            "v2/compare",
            br#"{"reference_io_id":0,"candidate_io_id":1,"metrics":["zensim_a","zensim_a"]}"#
        )
        .1
        .is_err());
    assert!(c_cmp
        .message("v2/compare", br#"{"reference_io_id":0,"candidate_io_id":1,"metrics":["psnr"]}"#)
        .1
        .is_err());
    assert!(c_cmp
        .message("v2/compare", br#"{"reference_io_id":0,"candidate_io_id":1,"bogus":1}"#)
        .1
        .is_err());
    assert!(
        c_cmp.message("v2/compare", br#"{"reference_io_id":0,"candidate_io_id":2}"#).1.is_err(),
        "dimension mismatch must be rejected"
    );
    assert!(
        c_cmp.message("v2/compare", br#"{"reference_io_id":3,"candidate_io_id":3}"#).1.is_err(),
        "animated GIF must be rejected by v2/compare"
    );
    // Translucent alpha: ZensimA accepts RGBA8 straight alpha, while Butteraugli refuses translucency!
    c_cmp
        .message(
            "v2/compare",
            br#"{"reference_io_id":4,"candidate_io_id":4,"metrics":["zensim_a","ssimulacra2"]}"#,
        )
        .1
        .expect("zensim_a and ssimulacra2 accept straight alpha");
    assert!(
        c_cmp
            .message(
                "v2/compare",
                br#"{"reference_io_id":4,"candidate_io_id":4,"metrics":["butteraugli_max"]}"#,
            )
            .1
            .is_err(),
        "butteraugli_max must refuse genuinely translucent RGBA input"
    );
    // Tight combined memory limit fails with SizeLimitExceeded before decoding.
    let mem_err = c_cmp
        .message(
            "v2/compare",
            br#"{"reference_io_id":0,"candidate_io_id":1,"limits":{"max_memory_bytes":16384}}"#,
        )
        .1
        .expect_err("16 KiB max_memory_bytes must reject 64x48 comparison");
    assert_eq!(mem_err.kind, imageflow_core::ErrorKind::SizeLimitExceeded);
    assert_eq!(mem_err.category(), ErrorCategory::ArgumentInvalid);
    // Overflow timeout fails cleanly as InvalidArgument.
    let ovf_err = c_cmp
        .message(
            "v2/compare",
            br#"{"reference_io_id":0,"candidate_io_id":1,"limits":{"timeout_ms":18446744073709551615}}"#,
        )
        .1
        .expect_err("overflow timeout_ms must be rejected");
    assert_eq!(ovf_err.kind, imageflow_core::ErrorKind::InvalidArgument);
    assert_eq!(ovf_err.category(), ErrorCategory::ArgumentInvalid);

    // 7. Timeout precedence in BOTH directions:
    //    - v2/configure_context rejects overflow timeout_ms atomically (preserving prior config)
    //    - A valid shorter context timeout_ms MUST NOT mask an invalid request timeout_ms!
    let mut c_to = Context::create().unwrap();
    c_to.add_copied_input_buffer(0, &ref_png).unwrap();
    c_to.add_copied_input_buffer(1, &cand_jpg).unwrap();
    c_to.add_output_buffer(2).unwrap();
    c_to.message("v2/configure_context", br#"{"limits":{"timeout_ms":1000}}"#).1.unwrap();
    let cfg_ovf = c_to
        .message(
            "v2/configure_context",
            br#"{"workspace_policy":"best_effort","limits":{"timeout_ms":18446744073709551615}}"#,
        )
        .1
        .expect_err("v2/configure_context must reject overflow timeout_ms");
    assert_eq!(cfg_ovf.kind, imageflow_core::ErrorKind::InvalidArgument);
    // Atomic rollback: workspace_policy stayed Strict (not mutated to best_effort!).
    assert!(!c_to.modern_config().workspace_policy_best_effort);
    assert_eq!(c_to.modern_config().timeout_ms, Some(1000));
    // Context 1000 ms + request u64::MAX must STILL reject as InvalidArgument on v2/compare & v2/execute!
    let cmp_masked = c_to
        .message(
            "v2/compare",
            br#"{"reference_io_id":0,"candidate_io_id":1,"limits":{"timeout_ms":18446744073709551615}}"#,
        )
        .1
        .expect_err("shorter context timeout must not mask invalid request timeout on v2/compare");
    assert_eq!(cmp_masked.kind, imageflow_core::ErrorKind::InvalidArgument);
    let exec_masked = c_to
        .message(
            "v2/execute",
            br#"{
                "inputs": {},
                "outputs": {"out": 2},
                "limits": {"timeout_ms": 18446744073709551615},
                "pipeline": {
                    "steps": [
                        {"canvas": {"width": 4, "height": 4, "fill": [10, 20, 30, 255]}},
                        {"encode": {"output": "out", "format": "png"}}
                    ]
                }
            }"#,
        )
        .1
        .expect_err("shorter context timeout must not mask invalid request timeout on v2/execute");
    assert_eq!(exec_masked.kind, imageflow_core::ErrorKind::InvalidArgument);
    // Valid context 5000 ms + valid request 1000 ms succeeds cleanly.
    c_to.message("v2/configure_context", br#"{"limits":{"timeout_ms":5000}}"#).1.unwrap();
    c_to.message(
        "v2/compare",
        br#"{"reference_io_id":0,"candidate_io_id":1,"limits":{"timeout_ms":1000}}"#,
    )
    .1
    .expect("valid shorter request timeout intersected with context timeout must succeed");

    // 8. Animated GIF refusal under BestEffort too (proving post-probe MediaKind::Animation check,
    //    not just Strict unproven-probe refusal!).
    let mut c_be = Context::create().unwrap();
    c_be.message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#).1.unwrap();
    c_be.add_copied_input_buffer(0, &anim_gif).unwrap();
    let gif_be_err = c_be
        .message("v2/compare", br#"{"reference_io_id":0,"candidate_io_id":0}"#)
        .1
        .expect_err("animated GIF must be rejected under BestEffort too");
    assert_eq!(gif_be_err.category(), ErrorCategory::ActionNotSupported);

    // 9. Bounded JPEG container discovery: refuse Ultra HDR gain-map JPEG AND multi-image MPO JPEG,
    //    and preserve fidelity on XYB JPEG without double-applying stored XYB ICC!
    let mut rgb64x48 = Vec::with_capacity(64 * 48 * 3);
    for y in 0..48u32 {
        let yf = y as f32 / 48.0;
        for x in 0..64u32 {
            let xf = x as f32 / 64.0;
            let wave = ((xf * 10.0 + yf * 6.0).sin() * 0.5 + 0.5) * 160.0;
            let r = (40.0 + 170.0 * xf).clamp(0.0, 255.0) as u8;
            let g = (35.0 + 170.0 * yf).clamp(0.0, 255.0) as u8;
            let b = (30.0 + wave).clamp(0.0, 255.0) as u8;
            rgb64x48.extend_from_slice(&[r, g, b]);
        }
    }
    let uhdr_cfg =
        zenjpeg::encoder::EncoderConfig::ycbcr(90.0, zenjpeg::encoder::ChromaSubsampling::Quarter)
            .add_gainmap(cand_jpg.clone());
    let uhdr_jpg =
        uhdr_cfg.encode_bytes(&rgb64x48, 64, 48, zenjpeg::encoder::PixelLayout::Rgb8Srgb).unwrap();
    let mut mpo_jpg = cand_jpg.clone();
    mpo_jpg.extend_from_slice(&cand_jpg);

    let xyb_cfg =
        zenjpeg::encoder::EncoderConfig::xyb(90.0, zenjpeg::encoder::XybSubsampling::BQuarter);
    let xyb_jpg =
        xyb_cfg.encode_bytes(&rgb64x48, 64, 48, zenjpeg::encoder::PixelLayout::Rgb8Srgb).unwrap();

    let prog_cfg =
        zenjpeg::encoder::EncoderConfig::ycbcr(90.0, zenjpeg::encoder::ChromaSubsampling::Quarter)
            .progressive(true);
    let prog_jpg =
        prog_cfg.encode_bytes(&rgb64x48, 64, 48, zenjpeg::encoder::PixelLayout::Rgb8Srgb).unwrap();

    let mut c_jpg = Context::create().unwrap();
    c_jpg.add_copied_input_buffer(0, &ref_png).unwrap();
    c_jpg.add_copied_input_buffer(1, &uhdr_jpg).unwrap();
    c_jpg.add_copied_input_buffer(2, &mpo_jpg).unwrap();
    c_jpg.add_copied_input_buffer(3, &xyb_jpg).unwrap();
    c_jpg.add_copied_input_buffer(4, &prog_jpg).unwrap();

    let uhdr_err = c_jpg
        .message("v2/compare", br#"{"reference_io_id":0,"candidate_io_id":1}"#)
        .1
        .expect_err("Ultra HDR gain-map JPEG must be rejected by v2/compare");
    assert_eq!(uhdr_err.category(), ErrorCategory::ActionNotSupported);

    let mpo_err = c_jpg
        .message("v2/compare", br#"{"reference_io_id":0,"candidate_io_id":2}"#)
        .1
        .expect_err("multi-image MPO JPEG must be rejected by v2/compare");
    assert_eq!(mpo_err.category(), ErrorCategory::ActionNotSupported);

    let (xyb_resp, xyb_res) =
        c_jpg.message("v2/compare", br#"{"reference_io_id":0,"candidate_io_id":3}"#);
    xyb_res.expect("XYB JPEG must decode to sRGB without double-applying stored XYB ICC");
    let xyb_json: serde_json::Value = serde_json::from_slice(&xyb_resp.response_json).unwrap();
    let xyb_score = xyb_json["data"]["results"][0]["score"].as_f64().unwrap();
    assert!(
        xyb_score >= 80.0,
        "XYB JPEG compared against source PNG must preserve high fidelity (no double ICC invert), got zensim_a={xyb_score}"
    );

    let (prog_resp, prog_res) =
        c_jpg.message("v2/compare", br#"{"reference_io_id":0,"candidate_io_id":4}"#);
    prog_res.expect("progressive JPEG must probe and decode cleanly via stop-aware JPEG path");
    let prog_json: serde_json::Value = serde_json::from_slice(&prog_resp.response_json).unwrap();
    let prog_score = prog_json["data"]["results"][0]["score"].as_f64().unwrap();
    assert!(
        prog_score >= 80.0,
        "progressive JPEG compared against source PNG must preserve high fidelity, got zensim_a={prog_score}"
    );

    // 10. PNG raw chunk ingress: catch silently-discarded corrupt iCCP, enforce 1 MiB iCCP inflate
    //     buffer BEFORE probing, and enforce Strict refusal on compressed zTXt chunks!
    let inject_png_chunk = |png: &[u8], tag: [u8; 4], payload: &[u8]| -> Vec<u8> {
        let crc32 = |tag: &[u8; 4], data: &[u8]| -> u32 {
            let mut crc: u32 = 0xFFFF_FFFF;
            for &b in tag.iter().chain(data.iter()) {
                crc ^= u32::from(b);
                for _ in 0..8 {
                    let mask = (crc & 1).wrapping_neg();
                    crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
                }
            }
            !crc
        };
        // Insert right after PNG signature (8 B) + IHDR chunk (4 len + 4 tag + 13 data + 4 crc = 25 B).
        let ihdr_end = 8 + 25;
        let mut out = Vec::with_capacity(png.len() + 12 + payload.len());
        out.extend_from_slice(&png[..ihdr_end]);
        out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        out.extend_from_slice(&tag);
        out.extend_from_slice(payload);
        out.extend_from_slice(&crc32(&tag, payload).to_be_bytes());
        out.extend_from_slice(&png[ihdr_end..]);
        out
    };
    let bad_iccp_png = inject_png_chunk(&ref_png, *b"iCCP", b"bad\0\0\xFF\xFF\xFF\xFF");
    let ztxt_png =
        inject_png_chunk(&ref_png, *b"zTXt", b"Comment\0\0\x78\x9c\x03\x00\x00\x00\x00\x01");
    let mut p3_prof = moxcms::ColorProfile::new_display_p3();
    if let Some(ref mut cicp) = p3_prof.cicp {
        cicp.full_range = true;
        cicp.matrix_coefficients = moxcms::MatrixCoefficients::Identity;
    }
    let p3_icc: Arc<[u8]> = Arc::from(p3_prof.encode().unwrap());
    let mut p3_meta = zc::Metadata::none();
    p3_meta.icc_profile = Some(Arc::clone(&p3_icc));
    let p3_fmt = zenpixels::PixelFormat::new(
        zenpixels::SampleLayout::RGB8,
        zenpixels::ColorSpace::Icc {
            profile: Arc::clone(&p3_icc),
            color_channels: 3,
            alpha: zenpixels::AlphaMode::None,
            intent: zenpixels::RenderingIntent::Perceptual,
        },
    );
    let p3_buf = zenpixels::PixelBuf::from_bytes(rgb64x48, 64, 48, p3_fmt).unwrap();
    let p3_png = shared_modern_engine()
        .unwrap()
        .encode_pixels(
            &zencodecs::engine::OutputIntent::format(zc::ImageFormat::Png),
            p3_buf.as_view(),
            &p3_meta,
            None,
            &zencodecs::engine::Run::new(),
        )
        .unwrap()
        .output
        .into_vec();

    let mut c_png = Context::create().unwrap();
    c_png.add_copied_input_buffer(0, &ref_png).unwrap();
    c_png.add_copied_input_buffer(1, &bad_iccp_png).unwrap();
    c_png.add_copied_input_buffer(2, &ztxt_png).unwrap();
    c_png.add_copied_input_buffer(3, &p3_png).unwrap();

    let bad_iccp_err = c_png
        .message("v2/compare", br#"{"reference_io_id":0,"candidate_io_id":1}"#)
        .1
        .expect_err("corrupt PNG iCCP discarded by zenpng must be rejected as ColorProfileError");
    assert_eq!(bad_iccp_err.kind, imageflow_core::ErrorKind::ColorProfileError);

    let ztxt_strict_err = c_png
        .message("v2/compare", br#"{"reference_io_id":0,"candidate_io_id":2}"#)
        .1
        .expect_err("Strict policy must refuse PNG with compressed zTXt chunk");
    assert_eq!(ztxt_strict_err.category(), ErrorCategory::ActionNotSupported);

    // Valid Display-P3 PNG with iCCP requires >= 1 MiB inflate buffer before probing:
    // 512 KiB budget rejects with SizeLimitExceeded BEFORE probe; 16 MiB budget succeeds!
    let p3_tight_err = c_png
        .message(
            "v2/compare",
            br#"{"reference_io_id":3,"candidate_io_id":3,"limits":{"max_memory_bytes":524288}}"#,
        )
        .1
        .expect_err("512 KiB cap must reject PNG iCCP 1 MiB inflate buffer before probing");
    assert_eq!(p3_tight_err.kind, imageflow_core::ErrorKind::SizeLimitExceeded);

    let (p3_ok_resp, p3_ok_res) = c_png.message(
        "v2/compare",
        br#"{"reference_io_id":3,"candidate_io_id":3,"limits":{"max_memory_bytes":16777216}}"#,
    );
    p3_ok_res.expect("Display-P3 ICC PNG self-compare under 16 MiB cap must succeed");
    let p3_ok_json: serde_json::Value = serde_json::from_slice(&p3_ok_resp.response_json).unwrap();
    assert_eq!(p3_ok_json["data"]["results"][0]["score"].as_f64().unwrap(), 100.0);

    // 11. R3/R4 regression witnesses:
    //     (a) Valid Display-P3 iCCP followed by corrupt duplicate iCCP -> ColorProfileError
    //     (b) Misplaced iCCP after IDAT -> ColorProfileError
    //     (c) Large Latin-1 tEXt Comment under 4 MiB cap (where plain compare SUCCEEDS!)
    //         -> SizeLimitExceeded specifically in "PNG header probe" BEFORE probing
    //     (d) BestEffort compression-flagged Software iTXt retained memory charge
    //     (e) Truncated trailing second JPEG (`SOI` without `EOI`) -> ActionNotSupported
    //     (f) Complete, moxcms-valid 45x45x45 `mAB ` CLUT ICC profile (273,375 pts > 262,144 cap)
    //         -> moxcms itself accepts it, while preflight_raw_icc rejects it BEFORE moxcms!
    let dup_iccp_png = inject_png_chunk(&p3_png, *b"iCCP", b"dup\0\0\xFF\xFF\xFF\xFF");

    // Append an iCCP chunk right before IEND (after IDAT).
    let iend_offset = ref_png.len() - 12;
    let crc32_fn = |tag: &[u8; 4], data: &[u8]| -> u32 {
        let mut crc: u32 = 0xFFFF_FFFF;
        for &b in tag.iter().chain(data.iter()) {
            crc ^= u32::from(b);
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !crc
    };
    let mut late_iccp_png = Vec::with_capacity(ref_png.len() + 32);
    late_iccp_png.extend_from_slice(&ref_png[..iend_offset]);
    let late_payload = b"late\0\0\x78\x9c\x03\x00\x00\x00\x00\x01";
    late_iccp_png.extend_from_slice(&(late_payload.len() as u32).to_be_bytes());
    late_iccp_png.extend_from_slice(b"iCCP");
    late_iccp_png.extend_from_slice(late_payload);
    late_iccp_png.extend_from_slice(&crc32_fn(b"iCCP", late_payload).to_be_bytes());
    late_iccp_png.extend_from_slice(&ref_png[iend_offset..]);

    // 768 KiB Latin-1 tEXt Comment (10x probe bound > 7.68 MiB, exceeding a 6 MiB cap where plain
    // comparison succeeds cleanly!).
    let mut big_text_payload = b"Comment\0".to_vec();
    big_text_payload.extend(std::iter::repeat_n(0xE9u8, 768 * 1024));
    let big_text_png = inject_png_chunk(&ref_png, *b"tEXt", &big_text_payload);

    // Complete primary JPEG followed by a truncated second JPEG header (no EOI).
    let mut truncated_second_jpg = cand_jpg.clone();
    truncated_second_jpg.extend_from_slice(b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01");

    // Complete, non-truncated ICC v4 profile whose A2B0 `mAB ` tag carries a real 45x45x45x3
    // (273,375-point) 1-byte CLUT — strictly accepted by moxcms 0.9.2 default parser (< 10M pts)
    // yet rejected by our preflight_raw_icc (max 262,144 pts)!
    let clut_points = 45 * 45 * 45 * 3; // 273,375 > 262,144
    let mab_size = (48 + 20 + clut_points + 3) & !3; // 4-byte aligned
    let mut big_clut_icc = vec![0u8; 132 + 12 + mab_size];
    let icc_len = big_clut_icc.len() as u32;
    big_clut_icc[0..4].copy_from_slice(&icc_len.to_be_bytes());
    big_clut_icc[8..12].copy_from_slice(&[4, 0x30, 0, 0]); // ICC v4.3
    big_clut_icc[12..16].copy_from_slice(b"mntr"); // Display device
    big_clut_icc[16..20].copy_from_slice(b"RGB "); // RGB color space
    big_clut_icc[20..24].copy_from_slice(b"XYZ "); // PCS XYZ
    big_clut_icc[36..40].copy_from_slice(b"acsp"); // magic signature
    big_clut_icc[68..72].copy_from_slice(&0x0000_F6D6u32.to_be_bytes()); // D50 X
    big_clut_icc[72..76].copy_from_slice(&0x0001_0000u32.to_be_bytes()); // D50 Y
    big_clut_icc[76..80].copy_from_slice(&0x0000_D32Du32.to_be_bytes()); // D50 Z
    big_clut_icc[128..132].copy_from_slice(&1u32.to_be_bytes()); // 1 tag
    big_clut_icc[132..136].copy_from_slice(b"A2B0");
    big_clut_icc[136..140].copy_from_slice(&144u32.to_be_bytes()); // offset 144
    big_clut_icc[140..144].copy_from_slice(&(mab_size as u32).to_be_bytes());
    let mab = &mut big_clut_icc[144..144 + mab_size];
    mab[0..4].copy_from_slice(b"mAB ");
    mab[8] = 3; // in_channels
    mab[9] = 3; // out_channels
    mab[24..28].copy_from_slice(&48u32.to_be_bytes()); // clut_offset = 48
    mab[48] = 45; // grid[0]
    mab[49] = 45; // grid[1]
    mab[50] = 45; // grid[2] -> 45^3 * 3 = 273,375 points!
    mab[64] = 1; // 1-byte entries (zero-filled CLUT table of exact length 273,375)
                 // Prove moxcms 0.9.2 itself accepts this non-truncated 273,375-point mAB CLUT!
    assert!(
        moxcms::ColorProfile::new_from_slice(&big_clut_icc).is_ok(),
        "fixture must be accepted by moxcms 0.9.2 so preflight_raw_icc refusal is unambiguously proved"
    );
    // Wrap `big_clut_icc` in a valid stored-block zlib stream for PNG iCCP (273 KiB < 1 MiB inflate cap).
    let zlib_store = |raw: &[u8]| -> Vec<u8> {
        let mut out = Vec::with_capacity(raw.len() + (raw.len() / 65535 + 1) * 5 + 6);
        out.extend_from_slice(&[0x78, 0x01]);
        let mut off = 0usize;
        while off < raw.len() {
            let take = (raw.len() - off).min(65535);
            let is_last = off + take == raw.len();
            out.push(if is_last { 0x01 } else { 0x00 });
            let len16 = take as u16;
            out.extend_from_slice(&len16.to_le_bytes());
            out.extend_from_slice(&(!len16).to_le_bytes());
            out.extend_from_slice(&raw[off..off + take]);
            off += take;
        }
        let mut s1: u32 = 1;
        let mut s2: u32 = 0;
        for &b in raw {
            s1 = (s1 + u32::from(b)) % 65521;
            s2 = (s2 + s1) % 65521;
        }
        out.extend_from_slice(&((s2 << 16) | s1).to_be_bytes());
        out
    };
    let mut big_clut_iccp_payload = b"bigmab\0\0".to_vec();
    big_clut_iccp_payload.extend_from_slice(&zlib_store(&big_clut_icc));
    let big_clut_png = inject_png_chunk(&ref_png, *b"iCCP", &big_clut_iccp_payload);

    let mut c_r3 = Context::create().unwrap();
    c_r3.add_copied_input_buffer(0, &ref_png).unwrap();
    c_r3.add_copied_input_buffer(1, &dup_iccp_png).unwrap();
    c_r3.add_copied_input_buffer(2, &late_iccp_png).unwrap();
    c_r3.add_copied_input_buffer(3, &big_text_png).unwrap();
    c_r3.add_copied_input_buffer(4, &truncated_second_jpg).unwrap();
    c_r3.add_copied_input_buffer(5, &big_clut_png).unwrap();

    let dup_err = c_r3
        .message("v2/compare", br#"{"reference_io_id":0,"candidate_io_id":1}"#)
        .1
        .expect_err("duplicate PNG iCCP chunk must be rejected as ColorProfileError");
    assert_eq!(dup_err.kind, imageflow_core::ErrorKind::ColorProfileError);

    let late_err = c_r3
        .message("v2/compare", br#"{"reference_io_id":0,"candidate_io_id":2}"#)
        .1
        .expect_err("misplaced post-IDAT PNG iCCP chunk must be rejected as ColorProfileError");
    assert_eq!(late_err.kind, imageflow_core::ErrorKind::ColorProfileError);

    // Control: plain (ref_png, ref_png) SUCCEEDS under 6 MiB cap (proving Zensim scratch + PNG decode fit)!
    c_r3.message(
        "v2/compare",
        br#"{"reference_io_id":0,"candidate_io_id":0,"limits":{"max_memory_bytes":6291456}}"#,
    )
    .1
    .expect("plain 64x48 PNG comparison must succeed under 6 MiB cap");

    // 6 MiB budget MUST reject 768 KiB Latin-1 tEXt PNG specifically at PNG header probe (> 7.68 MiB)!
    let text_mem_err = c_r3
        .message(
            "v2/compare",
            br#"{"reference_io_id":3,"candidate_io_id":0,"limits":{"max_memory_bytes":6291456}}"#,
        )
        .1
        .expect_err("6 MiB budget must reject 768 KiB Latin-1 tEXt PNG probe expansion");
    assert_eq!(text_mem_err.kind, imageflow_core::ErrorKind::SizeLimitExceeded);
    assert!(
        text_mem_err.to_string().contains("PNG header probe"),
        "expected pre-probe PNG header refusal, got: {text_mem_err}"
    );

    let trunc_jpg_err = c_r3
        .message("v2/compare", br#"{"reference_io_id":0,"candidate_io_id":4}"#)
        .1
        .expect_err("truncated trailing second JPEG must be rejected as ActionNotSupported");
    assert_eq!(trunc_jpg_err.category(), ErrorCategory::ActionNotSupported);

    let big_clut_err = c_r3
        .message("v2/compare", br#"{"reference_io_id":0,"candidate_io_id":5}"#)
        .1
        .expect_err("moxcms-valid 273,375-point mAB CLUT must be rejected by preflight_raw_icc");
    assert_eq!(big_clut_err.kind, imageflow_core::ErrorKind::ColorProfileError);
    let big_clut_msg = big_clut_err.to_string();
    assert!(
        big_clut_msg.contains("273375") && big_clut_msg.contains("262144"),
        "expected preflight_raw_icc CLUT cap diagnostic, got: {big_clut_msg}"
    );

    // 12. R6 direct-JPEG parity witnesses:
    //     (a) `limits.max_output_bytes` rejects a JPEG whose decoded raster exceeds the cap.
    //     (b) Distinct-intent (`A2B0` Perceptual = all-black vs `A2B1` RelativeColorimetric = sRGB
    //         XYZ cube) non-XYB ICC profile embedded in BOTH PNG (Engine/zenpng) and JPEG (direct
    //         `decode_into` + `pixel_format_for_decoded_pixels`) yields high parity (`zensim_a >= 80`),
    //         proving JPEG uses `RelativeColorimetric` (`A2B1`), NEVER `Perceptual` (`A2B0`)!
    let mut c_r6 = Context::create().unwrap();
    c_r6.add_copied_input_buffer(0, &cand_jpg).unwrap();
    let max_out_err = c_r6
        .message(
            "v2/compare",
            br#"{"reference_io_id":0,"candidate_io_id":0,"limits":{"max_output_bytes":1024}}"#,
        )
        .1
        .expect_err("max_output_bytes=1024 must reject 64x48 JPEG (9216 B decoded raster)");
    assert_eq!(max_out_err.kind, imageflow_core::ErrorKind::SizeLimitExceeded);

    // Build a valid ICC v4 RGB profile via moxcms with TWO distinct mAB 2x2x2 CLUTs:
    // - A2B0 (Perceptual): all-zero XYZ -> converts every pixel to black [0,0,0]!
    // - A2B1 (RelativeColorimetric): 2x2x2 sRGB-to-D50-XYZ cube -> preserves full bright image!
    let xyz_u16 = |v: f64| -> u16 { ((v * 32768.0).round() as u32).min(65535) as u16 };
    let r_xyz = [0.43607, 0.22250, 0.01393];
    let g_xyz = [0.38506, 0.71687, 0.09710];
    let b_xyz = [0.14308, 0.06062, 0.71417];
    let mut a2b1_flat = Vec::with_capacity(24);
    for r in 0..2 {
        for g in 0..2 {
            for b in 0..2 {
                let rf = r as f64;
                let gf = g as f64;
                let bf = b as f64;
                a2b1_flat.push(xyz_u16(rf * r_xyz[0] + gf * g_xyz[0] + bf * b_xyz[0]));
                a2b1_flat.push(xyz_u16(rf * r_xyz[1] + gf * g_xyz[1] + bf * b_xyz[1]));
                a2b1_flat.push(xyz_u16(rf * r_xyz[2] + gf * g_xyz[2] + bf * b_xyz[2]));
            }
        }
    }
    let mut grid = [0u8; 16];
    grid[0] = 2;
    grid[1] = 2;
    grid[2] = 2;
    let id_curves = vec![moxcms::ToneReprCurve::Lut(vec![]); 3];
    let mut dual_prof = moxcms::ColorProfile::new_srgb();
    dual_prof.cicp = None;
    dual_prof.lut_a_to_b_perceptual =
        Some(moxcms::LutWarehouse::Multidimensional(moxcms::LutMultidimensionalType {
            num_input_channels: 3,
            num_output_channels: 3,
            grid_points: grid,
            clut: Some(moxcms::LutStore::Store16(vec![0u16; 24])),
            a_curves: id_curves.clone(),
            b_curves: id_curves.clone(),
            m_curves: id_curves.clone(),
            matrix: moxcms::Matrix3d::IDENTITY,
            bias: moxcms::Vector3d::default(),
        }));
    dual_prof.lut_a_to_b_colorimetric =
        Some(moxcms::LutWarehouse::Multidimensional(moxcms::LutMultidimensionalType {
            num_input_channels: 3,
            num_output_channels: 3,
            grid_points: grid,
            clut: Some(moxcms::LutStore::Store16(a2b1_flat)),
            a_curves: id_curves.clone(),
            b_curves: id_curves.clone(),
            m_curves: id_curves,
            matrix: moxcms::Matrix3d::IDENTITY,
            bias: moxcms::Vector3d::default(),
        }));
    let dual_icc = dual_prof.encode().unwrap();
    // Prove the two rendering intents produce starkly different pixels in moxcms!
    let parsed_dual = moxcms::ColorProfile::new_from_slice(&dual_icc).unwrap();
    let srgb_dst = moxcms::ColorProfile::new_srgb();
    let mut perc_px = [0u8; 3];
    parsed_dual
        .create_transform_8bit(
            moxcms::Layout::Rgb,
            &srgb_dst,
            moxcms::Layout::Rgb,
            moxcms::TransformOptions {
                rendering_intent: moxcms::RenderingIntent::Perceptual,
                ..Default::default()
            },
        )
        .unwrap()
        .transform(&[200, 150, 100], &mut perc_px)
        .unwrap();
    let mut rel_px = [0u8; 3];
    parsed_dual
        .create_transform_8bit(
            moxcms::Layout::Rgb,
            &srgb_dst,
            moxcms::Layout::Rgb,
            moxcms::TransformOptions {
                rendering_intent: moxcms::RenderingIntent::RelativeColorimetric,
                ..Default::default()
            },
        )
        .unwrap()
        .transform(&[200, 150, 100], &mut rel_px)
        .unwrap();
    assert_eq!(perc_px, [0, 0, 0], "Perceptual A2B0 must map to black");
    assert!(rel_px[0] > 150, "RelativeColorimetric A2B1 must preserve bright red, got {rel_px:?}");

    // Encode a genuine non-XYB YCbCr JPEG carrying ONLY `dual_icc`, then encode a PNG with the
    // exact same decoded RGB8 grid and exact same `dual_icc` (no sRGB chunk) so PNG (Engine/zenpng)
    // and JPEG (direct decode_into + pixel_format_for_decoded_pixels) can differ ONLY in ICC intent!
    let mut rgb_src = Vec::with_capacity(64 * 48 * 3);
    for y in 0..48u32 {
        let yf = y as f32 / 48.0;
        for x in 0..64u32 {
            let xf = x as f32 / 64.0;
            let wave = ((xf * 10.0 + yf * 6.0).sin() * 0.5 + 0.5) * 160.0;
            rgb_src.push((40.0 + 170.0 * xf).clamp(0.0, 255.0) as u8);
            rgb_src.push((35.0 + 170.0 * yf).clamp(0.0, 255.0) as u8);
            rgb_src.push((30.0 + wave).clamp(0.0, 255.0) as u8);
        }
    }
    let mut icc_app2_body = b"ICC_PROFILE\0\x01\x01".to_vec();
    icc_app2_body.extend_from_slice(&dual_icc);
    let dual_jpg =
        zenjpeg::encoder::EncoderConfig::ycbcr(95.0, zenjpeg::encoder::ChromaSubsampling::None)
            .add_segment(0xE2, icc_app2_body)
            .encode_bytes(&rgb_src, 64, 48, zenjpeg::encoder::PixelLayout::Rgb8Srgb)
            .unwrap();

    let mut raw_rgb_from_dual_jpg = vec![0u8; 64 * 48 * 3];
    zenjpeg::decoder::Decoder::new()
        .decode_into(
            &dual_jpg,
            zenjpeg::decoder::PixelFormat::Rgb,
            &mut raw_rgb_from_dual_jpg,
            zc::enough::Unstoppable,
        )
        .unwrap();
    // Transform `raw_rgb_from_dual_jpg` via `zenpixels_convert::RowPlan` + `MoxCms` under BOTH
    // `RelativeColorimetric` (expected_rel_png) and `Perceptual` (expected_perc_png).
    // Comparing `(expected_rel_png, dual_jpg)` MUST score 100.0 bit-for-bit, while
    // `(expected_perc_png, dual_jpg)` MUST score < 20 (all-black A2B0 vs bright A2B1)!
    let render_with_intent = |intent: zenpixels::RenderingIntent| -> Vec<u8> {
        let src_fmt = zenpixels::PixelFormat::new(
            zenpixels::SampleLayout::RGB8,
            zenpixels::ColorSpace::Icc {
                profile: Arc::from(dual_icc.clone()),
                color_channels: 3,
                alpha: zenpixels::AlphaMode::None,
                intent,
            },
        );
        let policy = zenpixels_convert::ConvertPolicy::default()
            .with_depth(zenpixels_convert::DepthPolicy::Round)
            .with_gamut(zenpixels_convert::GamutPolicy::Clip)
            .with_cms(Arc::new(zenpixels_convert::MoxCms));
        let plan = zenpixels_convert::RowPlan::build(
            &src_fmt,
            &zenpixels::PixelFormat::RGBA8_SRGB,
            &policy,
            None,
        )
        .unwrap();
        let mut rgba = vec![0u8; 64 * 48 * 4];
        for y in 0..48usize {
            plan.convert_row(
                &raw_rgb_from_dual_jpg[y * 64 * 3..(y + 1) * 64 * 3],
                &mut rgba[y * 64 * 4..(y + 1) * 64 * 4],
                64,
            )
            .unwrap();
        }
        let buf = zenpixels::PixelBuf::from_bytes(rgba, 64, 48, zenpixels::PixelFormat::RGBA8_SRGB)
            .unwrap();
        shared_modern_engine()
            .unwrap()
            .encode_pixels(
                &zencodecs::engine::OutputIntent::format(zc::ImageFormat::Png),
                buf.as_view(),
                &zc::Metadata::none(),
                None,
                &zencodecs::engine::Run::new(),
            )
            .unwrap()
            .output
            .into_vec()
    };
    let expected_rel_png = render_with_intent(zenpixels::RenderingIntent::RelativeColorimetric);
    let expected_perc_png = render_with_intent(zenpixels::RenderingIntent::Perceptual);

    let probed_dual_jpg = zenjpeg::decoder::Decoder::new().read_info(&dual_jpg).unwrap();
    assert!(!probed_dual_jpg.is_xyb, "dual_jpg must be non-XYB");
    assert_eq!(probed_dual_jpg.icc_profile.as_deref(), Some(dual_icc.as_slice()));

    let mut c_intent = Context::create().unwrap();
    c_intent.add_copied_input_buffer(0, &expected_rel_png).unwrap();
    c_intent.add_copied_input_buffer(1, &dual_jpg).unwrap();
    c_intent.add_copied_input_buffer(2, &expected_perc_png).unwrap();
    let (intent_resp, intent_res) =
        c_intent.message("v2/compare", br#"{"reference_io_id":0,"candidate_io_id":1}"#);
    intent_res.expect("dual-intent ICC JPEG comparison must succeed");
    let intent_json: serde_json::Value =
        serde_json::from_slice(&intent_resp.response_json).unwrap();
    let intent_score = intent_json["data"]["results"][0]["score"].as_f64().unwrap();
    assert_eq!(
        intent_score, 100.0,
        "direct JPEG ICC must match RelativeColorimetric (A2B1) bit-for-bit, got zensim_a={intent_score}"
    );
    let (perc_resp, perc_res) =
        c_intent.message("v2/compare", br#"{"reference_io_id":2,"candidate_io_id":1}"#);
    perc_res.unwrap();
    let perc_json: serde_json::Value = serde_json::from_slice(&perc_resp.response_json).unwrap();
    let perc_score = perc_json["data"]["results"][0]["score"].as_f64().unwrap();
    assert!(
        perc_score < 20.0,
        "Perceptual A2B0 rendering must differ starkly from RelativeColorimetric A2B1 JPEG, got zensim_a={perc_score}"
    );
}
