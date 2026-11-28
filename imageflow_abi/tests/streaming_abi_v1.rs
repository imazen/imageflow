//! Opt-in C ABI v1 streaming tests (`imageflow_stream_v1.h`):
//! - Independent C compiler (`gcc`/`clang`) `sizeof` + `offsetof` verification against `wire.rs`,
//! - `imageflow_context_add_input_read_at_v1` ownership & release-once semantics,
//! - `imageflow_stream_job_*` manual stepping (surviving Context destruction, wrong-thread safety),
//! - `imageflow_stream_job_start_async_v1` double-start refusal + exact-once `release_user`,
//! - Pre-cancelled async start reentrant `notify -> next_v1` without `IFX_BUSY` or deadlock,
//! - Abandoned in-flight `IFX_EVENT_COMMIT` on `release_v1` reaching quiescence cleanly.

#![cfg(all(
    feature = "streaming-ffi",
    feature = "async-ffi",
    feature = "zen-codecs",
    feature = "all-media"
))]

use core::ffi::c_void;
use core::mem::size_of;
use std::ffi::CString;
use std::process::Command;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use imageflow::streaming::*;
use imageflow::{
    imageflow_context_create, imageflow_context_destroy, imageflow_context_send_json,
    imageflow_json_response_destroy,
};

struct ReadAtState {
    bytes: Vec<u8>,
    releases: Arc<AtomicUsize>,
}

unsafe extern "C" fn test_read_at(
    user: *mut c_void,
    offset: *const IfxU64V1,
    dst: *mut u8,
    capacity: usize,
    written: *mut usize,
) -> i32 {
    let st = unsafe { &*(user as *const ReadAtState) };
    let off = unsafe { (*offset).to_u64() } as usize;
    if off >= st.bytes.len() {
        unsafe { *written = 0 };
        return IFX_END;
    }
    let n = capacity.min(st.bytes.len() - off);
    unsafe {
        ptr::copy_nonoverlapping(st.bytes.as_ptr().add(off), dst, n);
        *written = n;
    }
    IFX_OK
}

unsafe extern "C" fn test_len(user: *mut c_void, out: *mut IfxU64V1) -> i32 {
    let st = unsafe { &*(user as *const ReadAtState) };
    unsafe { *out = IfxU64V1::from_u64(st.bytes.len() as u64) };
    IFX_OK
}

unsafe extern "C" fn test_release_read_at(user: *mut c_void) {
    let boxed = unsafe { Box::from_raw(user as *mut ReadAtState) };
    boxed.releases.fetch_add(1, Ordering::SeqCst);
}

struct AsyncCookie {
    pair: (Mutex<u32>, Condvar),
    releases: Arc<AtomicUsize>,
}

unsafe extern "C" fn test_notify(user: *mut c_void, flags: u32) {
    let c = unsafe { &*(user as *const AsyncCookie) };
    let mut g = c.pair.0.lock().unwrap();
    *g |= flags;
    c.pair.1.notify_all();
}

unsafe extern "C" fn test_release_async(user: *mut c_void) {
    let boxed = unsafe { Box::from_raw(user as *mut AsyncCookie) };
    boxed.releases.fetch_add(1, Ordering::SeqCst);
}

struct ReentrantPollCookie {
    job: *mut IfxJobV1,
    poll_rcs: Arc<Mutex<Vec<i32>>>,
    releases: Arc<AtomicUsize>,
}

unsafe extern "C" fn reentrant_poll_notify(user: *mut c_void, _flags: u32) {
    let c = unsafe { &*(user as *const ReentrantPollCookie) };
    let mut ev = IfxEventV1 { size: size_of::<IfxEventV1>() as u32, ..Default::default() };
    let rc = unsafe { imageflow_stream_job_next_v1(c.job, 0, &mut ev) };
    c.poll_rcs.lock().unwrap().push(rc);
}

unsafe extern "C" fn reentrant_poll_release(user: *mut c_void) {
    let boxed = unsafe { Box::from_raw(user as *mut ReentrantPollCookie) };
    boxed.releases.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn c_header_layout_matches_rust_wire_via_cc() {
    let header = concat!(env!("CARGO_MANIFEST_DIR"), "/include/imageflow_stream_v1.h");
    let c_src = format!(
        r#"
#include <stddef.h>
#include <stdio.h>
#include "{header}"

#define CHK(cond, msg) do {{ if (!(cond)) {{ fprintf(stderr, "FAIL: %s\n", msg); return 1; }} }} while (0)

int main(void) {{
    CHK(sizeof(IfxU64V1) == 8, "IfxU64V1 size");
    CHK(offsetof(IfxU64V1, lo) == 0 && offsetof(IfxU64V1, hi) == 4, "IfxU64V1 offsets");

    CHK(sizeof(IfxBindingV1) == 16, "IfxBindingV1 size");
    CHK(offsetof(IfxBindingV1, size) == 0 && offsetof(IfxBindingV1, io_id) == 4 &&
        offsetof(IfxBindingV1, flags) == 8 && offsetof(IfxBindingV1, reserved) == 12, "IfxBindingV1 offsets");

    CHK(sizeof(IfxEventV1) == 56, "IfxEventV1 size");
    CHK(offsetof(IfxEventV1, size) == 0 && offsetof(IfxEventV1, kind) == 4 &&
        offsetof(IfxEventV1, io_id) == 8 && offsetof(IfxEventV1, flags) == 12 &&
        offsetof(IfxEventV1, ticket) == 16 && offsetof(IfxEventV1, offset) == 24 &&
        offsetof(IfxEventV1, length) == 32 && offsetof(IfxEventV1, transfer_max) == 40 &&
        offsetof(IfxEventV1, status) == 44 && offsetof(IfxEventV1, reason) == 48 &&
        offsetof(IfxEventV1, reserved) == 52, "IfxEventV1 offsets");

    CHK(sizeof(IfxReplyV1) == 32, "IfxReplyV1 size");
    CHK(offsetof(IfxReplyV1, size) == 0 && offsetof(IfxReplyV1, code) == 4 &&
        offsetof(IfxReplyV1, flags) == 8 && offsetof(IfxReplyV1, reserved) == 12 &&
        offsetof(IfxReplyV1, ticket) == 16 && offsetof(IfxReplyV1, value) == 24, "IfxReplyV1 offsets");

    size_t ptr_sz = sizeof(void *);
    CHK(sizeof(IfxAsyncHooksV1) == (ptr_sz == 8 ? 32 : 20), "IfxAsyncHooksV1 size");
    CHK(sizeof(IfxReadAtV1) == (ptr_sz == 8 ? 40 : 24), "IfxReadAtV1 size");
    return 0;
}}
"#
    );
    let tmp_dir = std::env::var("HOME").unwrap_or_else(|_| ".".into()) + "/tmp/imageflow-abi-check";
    std::fs::create_dir_all(&tmp_dir).unwrap();
    let src_path = format!("{tmp_dir}/check_{}.c", std::process::id());
    let bin_path = format!("{tmp_dir}/check_{}", std::process::id());
    std::fs::write(&src_path, c_src).unwrap();

    let comp = Command::new("cc")
        .args(["-std=c99", "-Wall", "-Wextra", "-Werror", &src_path, "-o", &bin_path])
        .output()
        .expect("C compiler must be available");
    assert!(
        comp.status.success(),
        "C header check compile failed: {}",
        String::from_utf8_lossy(&comp.stderr)
    );
    let run = Command::new(&bin_path).output().expect("run C header check");
    let _ = std::fs::remove_file(&src_path);
    let _ = std::fs::remove_file(&bin_path);
    assert!(
        run.status.success(),
        "C header check failed: {}",
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn streaming_abi_manual_read_at_wrong_thread_and_async_lifecycle() {
    assert_eq!(imageflow_stream_abi_version_v1(), IFX_STREAM_ABI_VERSION_V1);
    assert_eq!(
        imageflow_stream_abi_features_v1(),
        IFX_STREAM_FEAT_MANUAL_V1 | IFX_STREAM_FEAT_ASYNC_V1
    );
    assert_eq!(unsafe { imageflow_stream_job_release_v1(ptr::null_mut()) }, IFX_OK);

    // 1. ReadAt registration: rejected on non-deferred context WITHOUT calling release_user!
    let ctx = imageflow_context_create(3, 2);
    assert!(!ctx.is_null());

    let releases = Arc::new(AtomicUsize::new(0));
    let raw_state = Box::into_raw(Box::new(ReadAtState {
        bytes: b"not-yet".to_vec(),
        releases: Arc::clone(&releases),
    }));
    let desc = IfxReadAtV1 {
        size: size_of::<IfxReadAtV1>() as u32,
        flags: 0,
        user: raw_state as *mut c_void,
        read_at: Some(test_read_at),
        length: Some(test_len),
        release_user: Some(test_release_read_at),
    };
    let reject_rc = unsafe { imageflow_context_add_input_read_at_v1(ctx, 0, &desc) };
    assert_eq!(reject_rc, IFX_INVALID_ARGUMENT);
    assert_eq!(
        releases.load(Ordering::SeqCst),
        0,
        "failed add_input_read_at must NOT invoke release_user"
    );

    // Enable deferred registration and retry: now succeeds and releases EXACTLY ONCE on destroy!
    let m_cfg = CString::new("v2/configure_context").unwrap();
    let cfg_body = br#"{"input_registration":"deferred","workspace_policy":"best_effort"}"#;
    let resp = unsafe {
        imageflow_context_send_json(ctx, m_cfg.as_ptr(), cfg_body.as_ptr(), cfg_body.len())
    };
    assert!(!resp.is_null());
    unsafe { imageflow_json_response_destroy(ctx, resp as *mut _) };

    let ok_rc = unsafe { imageflow_context_add_input_read_at_v1(ctx, 0, &desc) };
    assert_eq!(ok_rc, IFX_OK);

    // 2. Create manual job from ctx, then IMMEDIATELY destroy ctx (verifying zero borrowed ctx ptr!)
    let job_json = br#"{
        "inputs": {},
        "outputs": {"out": 1},
        "pipeline": {
            "steps": [
                {"canvas": {"width": 4, "height": 4, "fill": [20, 180, 90, 255]}},
                {"encode": {"output": "out", "format": "png"}}
            ]
        }
    }"#;
    let binding = IfxBindingV1 {
        size: size_of::<IfxBindingV1>() as u32,
        io_id: 1,
        flags: (1 << 1) | (1 << 3), // Sink + Incremental
        reserved: 0,
    };
    let mut job: *mut IfxJobV1 = ptr::null_mut();
    let c_rc = unsafe {
        imageflow_stream_job_create_v1(
            ctx,
            1, // Execute
            job_json.as_ptr(),
            job_json.len(),
            &binding,
            1,
            &mut job,
        )
    };
    assert_eq!(c_rc, IFX_OK);
    assert!(!job.is_null());

    // Destroy ctx right now — read_at releases exactly once, and `job` stays 100% valid!
    unsafe { imageflow_context_destroy(ctx) };
    assert_eq!(
        releases.load(Ordering::SeqCst),
        1,
        "context destroy must release ReadAt user cookie exactly once"
    );

    // Bind manual owner thread via first `next`, then verify foreign thread gets WRONG_THREAD!
    let mut ev = IfxEventV1 { size: size_of::<IfxEventV1>() as u32, ..Default::default() };
    assert_eq!(unsafe { imageflow_stream_job_next_v1(job, 16, &mut ev) }, IFX_OK);
    assert_eq!(ev.kind, IFX_EVENT_OUTPUT);

    let job_addr = job as usize;
    let foreign_rc = std::thread::spawn(move || {
        let p = job_addr as *mut IfxJobV1;
        let mut fev = IfxEventV1 { size: size_of::<IfxEventV1>() as u32, ..Default::default() };
        let r1 = unsafe { imageflow_stream_job_next_v1(p, 1, &mut fev) };
        let r2 = unsafe { imageflow_stream_job_release_v1(p) };
        (r1, r2)
    })
    .join()
    .unwrap();
    assert_eq!(foreign_rc, (IFX_WRONG_THREAD, IFX_WRONG_THREAD));

    // Finish manual job on owner thread!
    let mut out_png = Vec::new();
    loop {
        if ev.kind == IFX_EVENT_OUTPUT {
            let need = ev.length.to_u64() as usize;
            let mut chunk = vec![0u8; need];
            let mut written = 0usize;
            assert_eq!(
                unsafe {
                    imageflow_stream_job_copy_pending_v1(
                        job,
                        &ev.ticket,
                        chunk.as_mut_ptr(),
                        chunk.len(),
                        &mut written,
                    )
                },
                IFX_OK
            );
            out_png.extend_from_slice(&chunk[..written]);
            let rep = IfxReplyV1 {
                size: size_of::<IfxReplyV1>() as u32,
                code: IFX_REPLY_ACK,
                flags: 0,
                reserved: 0,
                ticket: ev.ticket,
                value: IfxU64V1::from_u64(written as u64),
            };
            assert_eq!(unsafe { imageflow_stream_job_reply_v1(job, &rep, ptr::null(), 0) }, IFX_OK);
        } else if ev.kind == IFX_EVENT_FLUSH {
            let rep = IfxReplyV1 {
                size: size_of::<IfxReplyV1>() as u32,
                code: IFX_REPLY_ACK,
                flags: 0,
                reserved: 0,
                ticket: ev.ticket,
                value: IfxU64V1::from_u64(0),
            };
            assert_eq!(unsafe { imageflow_stream_job_reply_v1(job, &rep, ptr::null(), 0) }, IFX_OK);
        } else if ev.kind == IFX_EVENT_COMPLETE {
            break;
        }
        assert_eq!(unsafe { imageflow_stream_job_next_v1(job, 16, &mut ev) }, IFX_OK);
    }
    assert!(out_png.starts_with(b"\x89PNG\r\n\x1a\n"));

    let mut needed = 0usize;
    assert_eq!(
        unsafe { imageflow_stream_job_result_json_v1(job, ptr::null_mut(), 0, &mut needed) },
        IFX_BUFFER_TOO_SMALL
    );
    let mut res_buf = vec![0u8; needed];
    assert_eq!(
        unsafe {
            imageflow_stream_job_result_json_v1(
                job,
                res_buf.as_mut_ptr(),
                res_buf.len(),
                &mut needed,
            )
        },
        IFX_OK
    );
    let res_val: serde_json::Value = serde_json::from_slice(&res_buf).unwrap();
    assert_eq!(res_val["version"], 2);
    assert_eq!(unsafe { imageflow_stream_job_release_v1(job) }, IFX_OK);

    // 3. Async FFI start + double-start rejection + exact-once release_user!
    let ctx2 = imageflow_context_create(3, 2);
    let mut ajob: *mut IfxJobV1 = ptr::null_mut();
    assert_eq!(
        unsafe {
            imageflow_stream_job_create_v1(
                ctx2,
                1,
                job_json.as_ptr(),
                job_json.len(),
                &binding,
                1,
                &mut ajob,
            )
        },
        IFX_OK
    );
    unsafe { imageflow_context_destroy(ctx2) };

    let async_releases = Arc::new(AtomicUsize::new(0));
    let cookie = Box::into_raw(Box::new(AsyncCookie {
        pair: (Mutex::new(0u32), Condvar::new()),
        releases: Arc::clone(&async_releases),
    }));
    let hooks = IfxAsyncHooksV1 {
        size: size_of::<IfxAsyncHooksV1>() as u32,
        flags: 0,
        user: cookie as *mut c_void,
        notify: Some(test_notify),
        release_user: Some(test_release_async),
    };
    assert_eq!(unsafe { imageflow_stream_job_start_async_v1(ajob, &hooks) }, IFX_OK);

    // Double start MUST fail without taking hook ownership!
    let dup_releases = Arc::new(AtomicUsize::new(0));
    let dup_cookie = Box::into_raw(Box::new(AsyncCookie {
        pair: (Mutex::new(0u32), Condvar::new()),
        releases: Arc::clone(&dup_releases),
    }));
    let dup_hooks = IfxAsyncHooksV1 {
        size: size_of::<IfxAsyncHooksV1>() as u32,
        flags: 0,
        user: dup_cookie as *mut c_void,
        notify: Some(test_notify),
        release_user: Some(test_release_async),
    };
    assert_eq!(
        unsafe { imageflow_stream_job_start_async_v1(ajob, &dup_hooks) },
        IFX_INVALID_ARGUMENT
    );
    assert_eq!(dup_releases.load(Ordering::SeqCst), 0);
    drop(unsafe { Box::from_raw(dup_cookie) });

    // Cancel async job and release handle from another thread (SafeHandle pattern);
    // release_user MUST fire EXACTLY ONCE on native quiescence!
    assert_eq!(unsafe { imageflow_stream_job_cancel_v1(ajob) }, IFX_OK);
    let ajob_addr = ajob as usize;
    std::thread::spawn(move || {
        assert_eq!(unsafe { imageflow_stream_job_release_v1(ajob_addr as *mut IfxJobV1) }, IFX_OK);
    })
    .join()
    .unwrap();

    let deadline = Instant::now() + Duration::from_secs(2);
    while async_releases.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(
        async_releases.load(Ordering::SeqCst),
        1,
        "async release_user must fire exactly once on quiescence"
    );

    // 4. Pre-cancelled job passed to start_async_v1 whose notify synchronously polls next_v1:
    //    must NEVER hit IFX_BUSY or deadlock (notifications activate strictly after unlock)!
    let ctx3 = imageflow_context_create(3, 2);
    let mut pre_job: *mut IfxJobV1 = ptr::null_mut();
    assert_eq!(
        unsafe {
            imageflow_stream_job_create_v1(
                ctx3,
                1,
                job_json.as_ptr(),
                job_json.len(),
                &binding,
                1,
                &mut pre_job,
            )
        },
        IFX_OK
    );
    unsafe { imageflow_context_destroy(ctx3) };
    assert_eq!(unsafe { imageflow_stream_job_cancel_v1(pre_job) }, IFX_OK);

    let poll_rcs = Arc::new(Mutex::new(Vec::new()));
    let pre_rel = Arc::new(AtomicUsize::new(0));
    let pre_cookie = Box::into_raw(Box::new(ReentrantPollCookie {
        job: pre_job,
        poll_rcs: Arc::clone(&poll_rcs),
        releases: Arc::clone(&pre_rel),
    }));
    let pre_hooks = IfxAsyncHooksV1 {
        size: size_of::<IfxAsyncHooksV1>() as u32,
        flags: 0,
        user: pre_cookie as *mut c_void,
        notify: Some(reentrant_poll_notify),
        release_user: Some(reentrant_poll_release),
    };
    assert_eq!(unsafe { imageflow_stream_job_start_async_v1(pre_job, &pre_hooks) }, IFX_OK);

    let dl3 = Instant::now() + Duration::from_secs(2);
    while pre_rel.load(Ordering::SeqCst) == 0 && Instant::now() < dl3 {
        std::thread::yield_now();
    }
    assert_eq!(pre_rel.load(Ordering::SeqCst), 1);
    let recorded = poll_rcs.lock().unwrap().clone();
    assert!(!recorded.is_empty(), "pre-stopped job must invoke notify at least once");
    for &rc in &recorded {
        assert!(
            rc == IFX_OK || rc == IFX_WOULD_BLOCK,
            "every pre-stopped notify->next_v1 call must avoid IFX_BUSY (recorded: {recorded:?})"
        );
    }
    assert_eq!(unsafe { imageflow_stream_job_release_v1(pre_job) }, IFX_OK);

    // 5. Abandoned in-flight IFX_EVENT_COMMIT via imageflow_stream_job_release_v1 reaches quiescence!
    let ctx4 = imageflow_context_create(3, 2);
    let cfg4_resp = unsafe {
        imageflow_context_send_json(ctx4, m_cfg.as_ptr(), cfg_body.as_ptr(), cfg_body.len())
    };
    assert!(!cfg4_resp.is_null());
    unsafe { imageflow_json_response_destroy(ctx4, cfg4_resp as *mut _) };
    let staged_binding = IfxBindingV1 {
        size: size_of::<IfxBindingV1>() as u32,
        io_id: 1,
        flags: 1 << 1, // Sink + Staged
        reserved: 0,
    };
    let mut commit_job: *mut IfxJobV1 = ptr::null_mut();
    assert_eq!(
        unsafe {
            imageflow_stream_job_create_v1(
                ctx4,
                1,
                job_json.as_ptr(),
                job_json.len(),
                &staged_binding,
                1,
                &mut commit_job,
            )
        },
        IFX_OK
    );
    unsafe { imageflow_context_destroy(ctx4) };

    let commit_rel = Arc::new(AtomicUsize::new(0));
    let c_cookie = Box::into_raw(Box::new(AsyncCookie {
        pair: (Mutex::new(0u32), Condvar::new()),
        releases: Arc::clone(&commit_rel),
    }));
    let c_hooks = IfxAsyncHooksV1 {
        size: size_of::<IfxAsyncHooksV1>() as u32,
        flags: 0,
        user: c_cookie as *mut c_void,
        notify: Some(test_notify),
        release_user: Some(test_release_async),
    };
    assert_eq!(unsafe { imageflow_stream_job_start_async_v1(commit_job, &c_hooks) }, IFX_OK);

    let mut cev = IfxEventV1 { size: size_of::<IfxEventV1>() as u32, ..Default::default() };
    loop {
        let rc = unsafe { imageflow_stream_job_next_v1(commit_job, 0, &mut cev) };
        if rc == IFX_WOULD_BLOCK {
            std::thread::yield_now();
            continue;
        }
        assert_eq!(rc, IFX_OK);
        if cev.kind == IFX_EVENT_OUTPUT {
            let rep = IfxReplyV1 {
                size: size_of::<IfxReplyV1>() as u32,
                code: IFX_REPLY_ACK,
                flags: 0,
                reserved: 0,
                ticket: cev.ticket,
                value: cev.length,
            };
            assert_eq!(
                unsafe { imageflow_stream_job_reply_v1(commit_job, &rep, ptr::null(), 0) },
                IFX_OK
            );
        } else if cev.kind == IFX_EVENT_FLUSH {
            let rep = IfxReplyV1 {
                size: size_of::<IfxReplyV1>() as u32,
                code: IFX_REPLY_ACK,
                flags: 0,
                reserved: 0,
                ticket: cev.ticket,
                value: IfxU64V1::from_u64(0),
            };
            assert_eq!(
                unsafe { imageflow_stream_job_reply_v1(commit_job, &rep, ptr::null(), 0) },
                IFX_OK
            );
        } else if cev.kind == IFX_EVENT_COMMIT {
            // Abandon commit handle directly via release_v1 WITHOUT replying!
            assert_eq!(unsafe { imageflow_stream_job_release_v1(commit_job) }, IFX_OK);
            break;
        } else {
            panic!("unexpected event kind {}", cev.kind);
        }
    }

    let dl4 = Instant::now() + Duration::from_secs(2);
    while commit_rel.load(Ordering::SeqCst) == 0 && Instant::now() < dl4 {
        std::thread::yield_now();
    }
    assert_eq!(
        commit_rel.load(Ordering::SeqCst),
        1,
        "abandoned Commit via C release_v1 must fire release_user exactly once"
    );
}

#[cfg(feature = "iqa")]
#[test]
fn c_abi_v2_capabilities_and_v2_compare_roundtrip() {
    use imageflow::{
        imageflow_context_add_input_buffer, imageflow_context_add_output_buffer,
        imageflow_context_get_output_buffer_by_id, imageflow_json_response_read,
    };

    let ctx = imageflow_context_create(3, 2);
    assert!(!ctx.is_null());

    let read_json = |resp: *const imageflow::JsonResponse| -> serde_json::Value {
        let mut status: i64 = 0;
        let mut ptr: *const u8 = ptr::null();
        let mut len: usize = 0;
        let ok =
            unsafe { imageflow_json_response_read(ctx, resp, &mut status, &mut ptr, &mut len) };
        assert!(ok, "imageflow_json_response_read must succeed");
        let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
        assert_eq!(
            status,
            200,
            "expected HTTP 200 status, body: {}",
            String::from_utf8_lossy(slice)
        );
        serde_json::from_slice(slice).unwrap()
    };

    // 1. v2/capabilities advertises live IQA scorers across C ABI.
    let m_cap = CString::new("v2/capabilities").unwrap();
    let cap_resp = unsafe { imageflow_context_send_json(ctx, m_cap.as_ptr(), b"{}".as_ptr(), 2) };
    assert!(!cap_resp.is_null());
    let cap_v = read_json(cap_resp);
    unsafe { imageflow_json_response_destroy(ctx, cap_resp as *mut _) };
    let vm: Vec<&str> = cap_v["data"]["verify_metrics"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(vm, vec!["zensim_a", "ssimulacra2", "butteraugli_max"]);
    assert_eq!(cap_v["data"]["iqa_metrics"].as_array().unwrap().len(), 3);

    // 2. Render PNG reference & Verify-encoded JPEG candidate via v2/execute, then score via v2/compare.
    assert!(unsafe { imageflow_context_add_output_buffer(ctx, 10) });
    assert!(unsafe { imageflow_context_add_output_buffer(ctx, 11) });
    let m_exec = CString::new("v2/execute").unwrap();
    let exec_req = br#"{
        "inputs": {},
        "outputs": {"png": 10, "jpg": 11},
        "pipeline": {
            "steps": [
                {"canvas": {"width": 32, "height": 32, "fill": [60, 140, 210, 255]}},
                {"encode": {"output": "png", "format": "png"}},
                {"encode": {"output": "jpg", "format": "jpeg", "coding": {"verify": {"metric": "zensim_a", "value": 85.0}}}}
            ]
        }
    }"#;
    let exec_resp = unsafe {
        imageflow_context_send_json(ctx, m_exec.as_ptr(), exec_req.as_ptr(), exec_req.len())
    };
    assert!(!exec_resp.is_null());
    let exec_v = read_json(exec_resp);
    unsafe { imageflow_json_response_destroy(ctx, exec_resp as *mut _) };
    assert_eq!(exec_v["data"]["outputs"].as_array().unwrap().len(), 2);

    let mut png_ptr: *const u8 = ptr::null();
    let mut png_len: usize = 0;
    assert!(unsafe {
        imageflow_context_get_output_buffer_by_id(ctx, 10, &mut png_ptr, &mut png_len)
    });
    let png_bytes = unsafe { std::slice::from_raw_parts(png_ptr, png_len) }.to_vec();

    let mut jpg_ptr: *const u8 = ptr::null();
    let mut jpg_len: usize = 0;
    assert!(unsafe {
        imageflow_context_get_output_buffer_by_id(ctx, 11, &mut jpg_ptr, &mut jpg_len)
    });
    let jpg_bytes = unsafe { std::slice::from_raw_parts(jpg_ptr, jpg_len) }.to_vec();

    let cmp_ctx = imageflow_context_create(3, 2);
    assert!(!cmp_ctx.is_null());
    assert!(unsafe {
        imageflow_context_add_input_buffer(
            cmp_ctx,
            0,
            png_bytes.as_ptr(),
            png_bytes.len(),
            imageflow::Lifetime::OutlivesFunctionCall,
        )
    });
    assert!(unsafe {
        imageflow_context_add_input_buffer(
            cmp_ctx,
            1,
            jpg_bytes.as_ptr(),
            jpg_bytes.len(),
            imageflow::Lifetime::OutlivesFunctionCall,
        )
    });
    let m_cmp = CString::new("v2/compare").unwrap();
    let cmp_req = br#"{"reference_io_id":0,"candidate_io_id":1,"metrics":["zensim_a","ssimulacra2","butteraugli_max"]}"#;
    let cmp_resp = unsafe {
        imageflow_context_send_json(cmp_ctx, m_cmp.as_ptr(), cmp_req.as_ptr(), cmp_req.len())
    };
    assert!(!cmp_resp.is_null());
    let mut st: i64 = 0;
    let mut bp: *const u8 = ptr::null();
    let mut bl: usize = 0;
    assert!(unsafe { imageflow_json_response_read(cmp_ctx, cmp_resp, &mut st, &mut bp, &mut bl) });
    assert_eq!(st, 200);
    let cmp_v: serde_json::Value =
        serde_json::from_slice(unsafe { std::slice::from_raw_parts(bp, bl) }).unwrap();
    assert_eq!(cmp_v["data"]["results"].as_array().unwrap().len(), 3);
    assert!(cmp_v["data"]["results"][0]["score"].as_f64().unwrap() >= 85.0);

    unsafe { imageflow_json_response_destroy(cmp_ctx, cmp_resp as *mut _) };
    unsafe { imageflow_context_destroy(cmp_ctx) };
    unsafe { imageflow_context_destroy(ctx) };
}
