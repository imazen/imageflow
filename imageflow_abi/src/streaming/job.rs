//! Opt-in C ABI v1 streaming job exports (`§5.1`–`§5.3`).
//!
//! Exports manual sans-I/O job stepping (`streaming-ffi`) and optional Tokio async
//! start (`async-ffi`) without altering any legacy C ABI export or header.

use core::ffi::c_void;
use core::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread::ThreadId;

use imageflow_core::media::{
    BindingSpec, CopyPendingError, IoEvent, IoInput, IoTicket, JobStop, MediaSession,
    OperationKind, ProgressMode, ProgressObserver, ProgressStatus, Reply, SessionError, StopCause,
    WorkQuota, host_services_from_context, shared_modern_engine,
};
use imageflow_core::ThreadSafeContext;

use crate::streaming::wire::{
    IFX_BUFFER_TOO_SMALL, IFX_BUSY, IFX_CLOSED, IFX_EVENT_COMMIT, IFX_EVENT_COMPLETE,
    IFX_EVENT_FAILED, IFX_EVENT_FLAG_INCREMENTAL, IFX_EVENT_FLUSH, IFX_EVENT_NEED_AT_LEAST,
    IFX_EVENT_NEED_LEN, IFX_EVENT_NEED_RANGE, IFX_EVENT_OUTPUT, IFX_EVENT_PATCH, IFX_EVENT_YIELD,
    IFX_HOOK_PROGRESS_NOTIFY, IFX_HOOK_PROGRESS_PULL, IFX_INTERNAL, IFX_INVALID_ARGUMENT, IFX_OK,
    IFX_PROGRESS_ABANDONED, IFX_PROGRESS_CANCELLED, IFX_PROGRESS_DISABLED, IFX_PROGRESS_FAILED,
    IFX_PROGRESS_FLAG_HAS_FRACTION, IFX_PROGRESS_FLAG_TERMINAL, IFX_PROGRESS_NOT_STARTED,
    IFX_PROGRESS_RUNNING, IFX_PROGRESS_SUCCEEDED, IFX_REPLY_ABORTED, IFX_REPLY_ACK,
    IFX_REPLY_BYTES, IFX_REPLY_END, IFX_REPLY_FLAG_HAS_VALUE, IFX_REPLY_IO_ERROR, IFX_REPLY_LENGTH,
    IFX_REPLY_UNKNOWN_LENGTH, IFX_RESOURCE_LIMIT, IFX_STALE_TICKET, IFX_STREAM_ABI_VERSION_V1,
    IFX_STREAM_FEAT_ASYNC_V1, IFX_STREAM_FEAT_MANUAL_V1, IFX_STREAM_FEAT_PROGRESS_V1,
    IFX_UNSUPPORTED, IFX_WOULD_BLOCK, IFX_WRONG_THREAD, IfxBindingV1, IfxEventV1, IfxProgressV1,
    IfxReplyV1, IfxU64V1,
};

#[cfg(feature = "async-ffi")]
use crate::streaming::wire::IfxAsyncHooksV1;

enum JobState {
    Manual {
        session: MediaSession,
        stepped: bool,
    },
    #[cfg(feature = "async-ffi")]
    Async(imageflow_core::media::AsyncJobHandle),
}

/// Opaque C streaming job handle owned by the caller until `imageflow_stream_job_release_v1`.
pub struct IfxJobV1 {
    busy: AtomicBool,
    owner_thread: Mutex<Option<ThreadId>>,
    stop: JobStop,
    progress: OnceLock<ProgressObserver>,
    state: Mutex<JobState>,
}

struct BusyGuard<'a>(&'a AtomicBool);

impl<'a> BusyGuard<'a> {
    fn try_acquire(flag: &'a AtomicBool) -> Option<Self> {
        flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Self(flag))
    }
}

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Independent streaming ABI version (`0x00010000`).
#[unsafe(no_mangle)]
pub extern "C" fn imageflow_stream_abi_version_v1() -> u32 {
    IFX_STREAM_ABI_VERSION_V1
}

/// Streaming ABI feature bitmask (`bit 0 = manual/read_at`, `bit 1 = async-ffi`, `bit 2 = progress`).
#[unsafe(no_mangle)]
pub extern "C" fn imageflow_stream_abi_features_v1() -> u32 {
    #[cfg(feature = "async-ffi")]
    {
        IFX_STREAM_FEAT_MANUAL_V1 | IFX_STREAM_FEAT_ASYNC_V1 | IFX_STREAM_FEAT_PROGRESS_V1
    }
    #[cfg(not(feature = "async-ffi"))]
    {
        IFX_STREAM_FEAT_MANUAL_V1 | IFX_STREAM_FEAT_PROGRESS_V1
    }
}

/// Create a frozen streaming job from `ctx` policy, operation kind, JSON envelope, and bindings.
///
/// Sets `*out_job = NULL` on error. Retains ZERO pointer to `ctx` after returning.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn imageflow_stream_job_create_v1(
    ctx: *mut ThreadSafeContext,
    operation: u32,
    json: *const u8,
    json_len: usize,
    bindings: *const IfxBindingV1,
    count: usize,
    out_job: *mut *mut IfxJobV1,
) -> i32 {
    if out_job.is_null() {
        return IFX_INVALID_ARGUMENT;
    }
    unsafe {
        *out_job = ptr::null_mut();
    }
    if ctx.is_null() || json.is_null() || json_len == 0 || json_len > (isize::MAX as usize) {
        return IFX_INVALID_ARGUMENT;
    }
    if count > 0 && (bindings.is_null() || count > 256) {
        return IFX_INVALID_ARGUMENT;
    }

    let res = catch_unwind(AssertUnwindSafe(|| {
        let Some(op) = OperationKind::from_wire(operation) else {
            return IFX_INVALID_ARGUMENT;
        };
        let json_slice = unsafe { std::slice::from_raw_parts(json, json_len) };
        let mut parsed_bindings = Vec::with_capacity(count);
        if count > 0 {
            let raw_bindings = unsafe { std::slice::from_raw_parts(bindings, count) };
            for b in raw_bindings {
                if b.size as usize != size_of::<IfxBindingV1>() {
                    return IFX_INVALID_ARGUMENT;
                }
                match BindingSpec::from_wire(b.io_id, b.flags, b.reserved) {
                    Ok(spec) => parsed_bindings.push(spec),
                    Err(_) => return IFX_INVALID_ARGUMENT,
                }
            }
        }

        // Borrow shared `&ThreadSafeContext` to prevent exclusive-reference aliasing against
        // concurrent `imageflow_context_request_cancellation` calls!
        let (services, base_timeout_ms) = {
            let c: &ThreadSafeContext = unsafe { &*ctx };
            let Ok(guard) = c.context_read_shared() else {
                return IFX_INTERNAL;
            };
            (host_services_from_context(&guard), guard.modern_config().timeout_ms)
        };
        let Ok(engine) = shared_modern_engine() else {
            return IFX_INTERNAL;
        };

        let stop = JobStop::new();
        let session = match MediaSession::create_with_stop_and_deadline(
            engine,
            services,
            op,
            json_slice,
            &parsed_bindings,
            stop.clone(),
            base_timeout_ms,
        ) {
            Ok(s) => s,
            Err(e) => return map_session_err(&e),
        };
        let boxed = Box::new(IfxJobV1 {
            busy: AtomicBool::new(false),
            owner_thread: Mutex::new(None),
            stop,
            progress: OnceLock::new(),
            state: Mutex::new(JobState::Manual { session, stepped: false }),
        });
        unsafe {
            *out_job = Box::into_raw(boxed);
        }
        IFX_OK
    }));
    res.unwrap_or(IFX_INTERNAL)
}

/// Step manual job (positive `work`) or poll async job (`work == 0`) for next event.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn imageflow_stream_job_next_v1(
    job: *mut IfxJobV1,
    work: u32,
    out_event: *mut IfxEventV1,
) -> i32 {
    if job.is_null() || out_event.is_null() {
        return IFX_INVALID_ARGUMENT;
    }
    let res = catch_unwind(AssertUnwindSafe(|| {
        let j = unsafe { &*job };
        let Some(_busy) = BusyGuard::try_acquire(&j.busy) else {
            return IFX_BUSY;
        };
        let caller_size = unsafe { (*out_event).size };
        if caller_size as usize != size_of::<IfxEventV1>() {
            return IFX_INVALID_ARGUMENT;
        }
        let Ok(mut state) = j.state.lock() else {
            return IFX_INTERNAL;
        };
        match &mut *state {
            JobState::Manual { session, stepped } => {
                if work == 0 {
                    return IFX_INVALID_ARGUMENT;
                }
                if !check_or_bind_owner_thread(&j.owner_thread) {
                    return IFX_WRONG_THREAD;
                }
                *stepped = true;
                match session.next(WorkQuota(work)) {
                    Ok(ev) => {
                        unsafe {
                            *out_event = encode_event(ev);
                        }
                        IFX_OK
                    }
                    Err(e) => map_session_err(&e),
                }
            }
            #[cfg(feature = "async-ffi")]
            JobState::Async(handle) => {
                if work != 0 {
                    return IFX_INVALID_ARGUMENT;
                }
                match handle.poll_event() {
                    Some(ev) => {
                        unsafe {
                            *out_event = encode_event(ev);
                        }
                        IFX_OK
                    }
                    None => IFX_WOULD_BLOCK,
                }
            }
        }
    }));
    res.unwrap_or(IFX_INTERNAL)
}

/// Copy pending output/patch bytes for `ticket` into caller buffer `dst`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn imageflow_stream_job_copy_pending_v1(
    job: *mut IfxJobV1,
    ticket: *const IfxU64V1,
    dst: *mut u8,
    cap: usize,
    written: *mut usize,
) -> i32 {
    if job.is_null() || ticket.is_null() || written.is_null() || cap > (isize::MAX as usize) {
        return IFX_INVALID_ARGUMENT;
    }
    if cap > 0 && dst.is_null() {
        return IFX_INVALID_ARGUMENT;
    }
    let res = catch_unwind(AssertUnwindSafe(|| {
        let j = unsafe { &*job };
        let Some(_busy) = BusyGuard::try_acquire(&j.busy) else {
            return IFX_BUSY;
        };
        let Some(t) = IoTicket::from_raw(unsafe { (*ticket).to_u64() }) else {
            return IFX_STALE_TICKET;
        };
        let dst_slice = if cap == 0 {
            &mut []
        } else {
            unsafe { std::slice::from_raw_parts_mut(dst, cap) }
        };
        let Ok(state) = j.state.lock() else {
            return IFX_INTERNAL;
        };
        let copy_res = match &*state {
            JobState::Manual { session, .. } => {
                if !check_owner_thread_if_bound(&j.owner_thread) {
                    return IFX_WRONG_THREAD;
                }
                session.copy_pending(t, dst_slice)
            }
            #[cfg(feature = "async-ffi")]
            JobState::Async(handle) => handle.copy_pending(t, dst_slice),
        };
        match copy_res {
            Ok(n) => {
                unsafe {
                    *written = n;
                }
                IFX_OK
            }
            Err(CopyPendingError::BufferTooSmall(need)) => {
                unsafe {
                    *written = need;
                }
                IFX_BUFFER_TOO_SMALL
            }
            Err(CopyPendingError::StaleTicket) => IFX_STALE_TICKET,
            Err(CopyPendingError::Closed) => IFX_CLOSED,
        }
    }));
    res.unwrap_or(IFX_INTERNAL)
}

/// Deliver a host reply for the active outstanding ticket.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn imageflow_stream_job_reply_v1(
    job: *mut IfxJobV1,
    reply: *const IfxReplyV1,
    bytes: *const u8,
    len: usize,
) -> i32 {
    if job.is_null() || reply.is_null() || len > (isize::MAX as usize) {
        return IFX_INVALID_ARGUMENT;
    }
    if len > 0 && bytes.is_null() {
        return IFX_INVALID_ARGUMENT;
    }
    let res = catch_unwind(AssertUnwindSafe(|| {
        let j = unsafe { &*job };
        let Some(_busy) = BusyGuard::try_acquire(&j.busy) else {
            return IFX_BUSY;
        };
        let raw = unsafe { *reply };
        if raw.size as usize != size_of::<IfxReplyV1>() || raw.reserved != 0 {
            return IFX_INVALID_ARGUMENT;
        }
        let Some(ticket) = IoTicket::from_raw(raw.ticket.to_u64()) else {
            return IFX_STALE_TICKET;
        };
        let slice = if len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(bytes, len) }
        };
        let parsed = match decode_reply(raw, slice) {
            Ok(r) => r,
            Err(code) => return code,
        };
        let Ok(mut state) = j.state.lock() else {
            return IFX_INTERNAL;
        };
        let reply_res = match &mut *state {
            JobState::Manual { session, .. } => {
                if !check_owner_thread_if_bound(&j.owner_thread) {
                    return IFX_WRONG_THREAD;
                }
                session.reply(ticket, parsed)
            }
            #[cfg(feature = "async-ffi")]
            JobState::Async(handle) => handle.reply(ticket, parsed),
        };
        match reply_res {
            Ok(()) => IFX_OK,
            Err(e) => map_session_err(&e),
        }
    }));
    res.unwrap_or(IFX_INTERNAL)
}

/// Copy finalized UTF-8 result or error JSON once terminal (`IFX_WOULD_BLOCK` before terminal).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn imageflow_stream_job_result_json_v1(
    job: *mut IfxJobV1,
    dst: *mut u8,
    cap: usize,
    needed: *mut usize,
) -> i32 {
    if job.is_null() || needed.is_null() || cap > (isize::MAX as usize) {
        return IFX_INVALID_ARGUMENT;
    }
    if cap > 0 && dst.is_null() {
        return IFX_INVALID_ARGUMENT;
    }
    let res = catch_unwind(AssertUnwindSafe(|| {
        let j = unsafe { &*job };
        let Some(_busy) = BusyGuard::try_acquire(&j.busy) else {
            return IFX_BUSY;
        };
        let Ok(state) = j.state.lock() else {
            return IFX_INTERNAL;
        };
        let json_opt: Option<Vec<u8>> = match &*state {
            JobState::Manual { session, .. } => {
                if !check_owner_thread_if_bound(&j.owner_thread) {
                    return IFX_WRONG_THREAD;
                }
                session.result_json().map(|b| b.to_vec())
            }
            #[cfg(feature = "async-ffi")]
            JobState::Async(handle) => handle.result_json_cloned(),
        };
        let Some(bytes) = json_opt else {
            unsafe {
                *needed = 0;
            }
            return IFX_WOULD_BLOCK;
        };
        unsafe {
            *needed = bytes.len();
        }
        if cap < bytes.len() {
            return IFX_BUFFER_TOO_SMALL;
        }
        if !bytes.is_empty() {
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len());
            }
        }
        IFX_OK
    }));
    res.unwrap_or(IFX_INTERNAL)
}

/// Thread-safe instant cancellation (may race any driver call and may be called from `notify`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn imageflow_stream_job_cancel_v1(job: *mut IfxJobV1) -> i32 {
    if job.is_null() {
        return IFX_INVALID_ARGUMENT;
    }
    let res = catch_unwind(AssertUnwindSafe(|| {
        let j = unsafe { &*job };
        let _ = j.stop.trip(StopCause::CallerCancel);
        IFX_OK
    }));
    res.unwrap_or(IFX_INTERNAL)
}

/// Consume and release `job`. `NULL` returns `IFX_OK`.
///
/// For manual jobs bound to an owner thread, releasing from a foreign thread returns
/// `IFX_WRONG_THREAD` without consuming the handle. Recovers poisoned locks solely for
/// cleanup so a prior unwind never leaks the allocation. For async jobs, release is safe
/// from any thread (e.g., .NET `SafeHandle`) and triggers non-blocking abandon + quiescence.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn imageflow_stream_job_release_v1(job: *mut IfxJobV1) -> i32 {
    if job.is_null() {
        return IFX_OK;
    }
    let res = catch_unwind(AssertUnwindSafe(|| {
        let j_ref = unsafe { &*job };
        let is_manual = match j_ref.state.lock() {
            Ok(state) => matches!(*state, JobState::Manual { .. }),
            Err(poisoned) => matches!(*poisoned.into_inner(), JobState::Manual { .. }),
        };
        if is_manual && !check_owner_thread_if_bound_recover(&j_ref.owner_thread) {
            return IFX_WRONG_THREAD;
        }
        // Safe to consume and reclaim handle now (even if a previous call panicked)!
        let boxed = unsafe { Box::from_raw(job) };
        let _ = boxed.stop.abandon();
        let inner = match boxed.state.into_inner() {
            Ok(s) => s,
            Err(p) => p.into_inner(),
        };
        match inner {
            JobState::Manual { session, .. } => drop(session),
            #[cfg(feature = "async-ffi")]
            JobState::Async(handle) => handle.release_caller_handle(),
        }
        IFX_OK
    }));
    res.unwrap_or(IFX_INTERNAL)
}

/// Opt in to lock-free atomic progress tracking on an un-stepped job before manual stepping or
/// async start. When omitted (and `hooks.flags == 0`), the job incurs zero progress overhead.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn imageflow_stream_job_enable_progress_v1(job: *mut IfxJobV1) -> i32 {
    if job.is_null() {
        return IFX_INVALID_ARGUMENT;
    }
    let res = catch_unwind(AssertUnwindSafe(|| {
        let j = unsafe { &*job };
        let Some(_busy) = BusyGuard::try_acquire(&j.busy) else {
            return IFX_BUSY;
        };
        let Ok(mut state) = j.state.lock() else {
            return IFX_INTERNAL;
        };
        match &mut *state {
            JobState::Manual { session, stepped: false } => {
                match session.enable_progress(ProgressMode::PullOnly) {
                    Ok(obs) => {
                        let _ = j.progress.set(obs);
                        IFX_OK
                    }
                    Err(e) => map_session_err(&e),
                }
            }
            _ => IFX_INVALID_ARGUMENT,
        }
    }));
    res.unwrap_or(IFX_INTERNAL)
}

/// Thread-safe, non-blocking progress summary read (safe to call concurrently with `next`/`reply`/`cancel`).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn imageflow_stream_job_progress_v1(
    job: *mut IfxJobV1,
    out_progress: *mut IfxProgressV1,
) -> i32 {
    if job.is_null() || out_progress.is_null() {
        return IFX_INVALID_ARGUMENT;
    }
    let res = catch_unwind(AssertUnwindSafe(|| {
        let caller_size = unsafe { (*out_progress).size };
        if caller_size as usize != size_of::<IfxProgressV1>() {
            return IFX_INVALID_ARGUMENT;
        }
        let j = unsafe { &*job };
        let Some(obs) = j.progress.get() else {
            unsafe {
                *out_progress = IfxProgressV1 {
                    size: size_of::<IfxProgressV1>() as u32,
                    status: IFX_PROGRESS_DISABLED,
                    flags: 0,
                    reserved: 0,
                    fraction: 0.0,
                    unresolved_fraction: 1.0,
                    generation: IfxU64V1::from_u64(0),
                };
            }
            return IFX_OK;
        };
        let Some(sum) = obs.try_summary() else {
            return IFX_WOULD_BLOCK;
        };
        let status = match sum.status {
            ProgressStatus::Disabled => IFX_PROGRESS_DISABLED,
            ProgressStatus::NotStarted => IFX_PROGRESS_NOT_STARTED,
            ProgressStatus::Running => IFX_PROGRESS_RUNNING,
            ProgressStatus::Succeeded => IFX_PROGRESS_SUCCEEDED,
            ProgressStatus::Failed => IFX_PROGRESS_FAILED,
            ProgressStatus::Cancelled => IFX_PROGRESS_CANCELLED,
            ProgressStatus::Abandoned => IFX_PROGRESS_ABANDONED,
        };
        let mut flags = 0u32;
        if sum.fraction.is_some() {
            flags |= IFX_PROGRESS_FLAG_HAS_FRACTION;
        }
        if sum.is_terminal {
            flags |= IFX_PROGRESS_FLAG_TERMINAL;
        }
        unsafe {
            *out_progress = IfxProgressV1 {
                size: size_of::<IfxProgressV1>() as u32,
                status,
                flags,
                reserved: 0,
                fraction: sum.fraction.unwrap_or(0.0),
                unresolved_fraction: sum.unresolved_fraction,
                generation: IfxU64V1::from_u64(sum.generation),
            };
        }
        IFX_OK
    }));
    res.unwrap_or(IFX_INTERNAL)
}

/// Thread-safe, non-blocking export of the versioned (`schema_version: 2`) phase-tree snapshot JSON.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn imageflow_stream_job_progress_json_v1(
    job: *mut IfxJobV1,
    dst: *mut u8,
    cap: usize,
    needed: *mut usize,
) -> i32 {
    if job.is_null() || needed.is_null() || cap > (isize::MAX as usize) {
        return IFX_INVALID_ARGUMENT;
    }
    if cap > 0 && dst.is_null() {
        return IFX_INVALID_ARGUMENT;
    }
    let res = catch_unwind(AssertUnwindSafe(|| {
        let j = unsafe { &*job };
        let Some(obs) = j.progress.get() else {
            return IFX_UNSUPPORTED;
        };
        let Some(json) = obs.try_snapshot_json() else {
            return IFX_WOULD_BLOCK;
        };
        let bytes = json.as_bytes();
        unsafe {
            *needed = bytes.len();
        }
        if cap < bytes.len() {
            return IFX_BUFFER_TOO_SMALL;
        }
        if !bytes.is_empty() {
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len());
            }
        }
        IFX_OK
    }));
    res.unwrap_or(IFX_INTERNAL)
}

/// Transition a freshly created un-stepped job onto the Tokio async driver (`async-ffi`).
///
/// Takes ownership of `hooks->user` + `release_user` ONLY on returning `IFX_OK`.
/// Host notifications are activated ONLY AFTER releasing internal job locks & busy guard,
/// so even a pre-cancelled job's `notify` callback can safely invoke `next`/`cancel` without
/// `IFX_BUSY` or deadlock.
#[cfg(feature = "async-ffi")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn imageflow_stream_job_start_async_v1(
    job: *mut IfxJobV1,
    hooks: *const IfxAsyncHooksV1,
) -> i32 {
    if job.is_null() || hooks.is_null() {
        return IFX_INVALID_ARGUMENT;
    }
    let res = catch_unwind(AssertUnwindSafe(|| {
        let j = unsafe { &*job };
        let activator = {
            let Some(_busy) = BusyGuard::try_acquire(&j.busy) else {
                return IFX_BUSY;
            };
            let raw_hooks = unsafe { *hooks };
            let allowed_hook_flags = IFX_HOOK_PROGRESS_PULL | IFX_HOOK_PROGRESS_NOTIFY;
            if raw_hooks.size as usize != size_of::<IfxAsyncHooksV1>()
                || (raw_hooks.flags & !allowed_hook_flags) != 0
            {
                return IFX_INVALID_ARGUMENT;
            }
            let progress_mode = if (raw_hooks.flags & IFX_HOOK_PROGRESS_NOTIFY) != 0 {
                imageflow_core::media::AsyncProgressMode::PacedNotify { min_interval_ms: 50 }
            } else if (raw_hooks.flags & IFX_HOOK_PROGRESS_PULL) != 0 {
                imageflow_core::media::AsyncProgressMode::PullOnly
            } else {
                imageflow_core::media::AsyncProgressMode::Disabled
            };
            let Some(notify_fn) = raw_hooks.notify else {
                return IFX_INVALID_ARGUMENT;
            };
            let Some(release_fn) = raw_hooks.release_user else {
                return IFX_INVALID_ARGUMENT;
            };

            let Ok(mut state) = j.state.lock() else {
                return IFX_INTERNAL;
            };
            // Reject double-start or starting after manual stepping WITHOUT taking hook ownership!
            match &*state {
                JobState::Manual { stepped: false, .. } => {}
                JobState::Manual { stepped: true, .. } | JobState::Async(_) => {
                    return IFX_INVALID_ARGUMENT;
                }
            }

            // Convert cookie pointer to usize to cross thread boundary safely under C contract (§5.3).
            let cookie_addr = raw_hooks.user as usize;
            let rust_hooks = imageflow_core::media::AsyncJobHooks {
                notify: Box::new(move |flags| unsafe {
                    (notify_fn)(cookie_addr as *mut c_void, flags);
                }),
                release: Box::new(move || unsafe {
                    (release_fn)(cookie_addr as *mut c_void);
                }),
            };

            // Temporarily move session out of JobState::Manual to start paused async worker.
            let placeholder_stop = j.stop.clone();
            let old = std::mem::replace(
                &mut *state,
                JobState::Manual {
                    session: MediaSession::create_with_stop(
                        shared_modern_engine().expect("engine initialized at create"),
                        Default::default(),
                        OperationKind::Inspect,
                        br#"{"inputs":{"_p":0},"input":"_p"}"#,
                        &[BindingSpec::forward_source(0, false)],
                        placeholder_stop,
                    )
                    .expect("placeholder"),
                    stepped: true,
                },
            );
            let JobState::Manual { session, .. } = old else {
                unreachable!();
            };

            match imageflow_core::media::AsyncJobHandle::start_paused_with_progress(
                session,
                rust_hooks,
                None,
                progress_mode,
            ) {
                Ok((handle, activator)) => {
                    if let Some(obs) = handle.progress_observer() {
                        let _ = j.progress.set(obs.clone());
                    }
                    *state = JobState::Async(handle);
                    activator
                }
                Err((err, returned_session, _returned_hooks)) => {
                    *state = JobState::Manual { session: returned_session, stepped: false };
                    return match err {
                        imageflow_core::media::AsyncStartError::QueueFull => IFX_RESOURCE_LIMIT,
                        imageflow_core::media::AsyncStartError::AlreadyStepped => {
                            IFX_INVALID_ARGUMENT
                        }
                        imageflow_core::media::AsyncStartError::RuntimeUnavailable(_) => {
                            IFX_INTERNAL
                        }
                    };
                }
            }
            // Both `state` MutexGuard AND `_busy` BusyGuard drop right here!
        };

        // Activate host notifications strictly OUTSIDE all job locks!
        activator.activate();
        IFX_OK
    }));
    res.unwrap_or(IFX_INTERNAL)
}

fn check_or_bind_owner_thread(slot: &Mutex<Option<ThreadId>>) -> bool {
    let cur = std::thread::current().id();
    let Ok(mut g) = slot.lock() else {
        return false;
    };
    match *g {
        None => {
            *g = Some(cur);
            true
        }
        Some(tid) => tid == cur,
    }
}

fn check_owner_thread_if_bound(slot: &Mutex<Option<ThreadId>>) -> bool {
    let cur = std::thread::current().id();
    let Ok(g) = slot.lock() else {
        return false;
    };
    match *g {
        None => true,
        Some(tid) => tid == cur,
    }
}

fn check_owner_thread_if_bound_recover(slot: &Mutex<Option<ThreadId>>) -> bool {
    let cur = std::thread::current().id();
    let g = match slot.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    match *g {
        None => true,
        Some(tid) => tid == cur,
    }
}

fn encode_event(ev: IoEvent) -> IfxEventV1 {
    let mut out = IfxEventV1 { size: size_of::<IfxEventV1>() as u32, ..Default::default() };
    match ev {
        IoEvent::NeedInput { ticket, io_id, request } => {
            out.io_id = io_id;
            out.ticket = IfxU64V1::from_u64(ticket.raw());
            match request {
                IoInput::AtLeast { next_offset, min_remaining, max_transfer } => {
                    out.kind = IFX_EVENT_NEED_AT_LEAST;
                    out.offset = IfxU64V1::from_u64(next_offset);
                    out.length = IfxU64V1::from_u64(min_remaining);
                    out.transfer_max = max_transfer;
                }
                IoInput::Range { offset, len } => {
                    out.kind = IFX_EVENT_NEED_RANGE;
                    out.offset = IfxU64V1::from_u64(offset);
                    out.length = IfxU64V1::from_u64(u64::from(len));
                    out.transfer_max = len;
                }
                IoInput::Len => {
                    out.kind = IFX_EVENT_NEED_LEN;
                }
            }
        }
        IoEvent::Output { ticket, io_id, offset, len, incremental } => {
            out.kind = IFX_EVENT_OUTPUT;
            out.io_id = io_id;
            out.flags = if incremental { IFX_EVENT_FLAG_INCREMENTAL } else { 0 };
            out.ticket = IfxU64V1::from_u64(ticket.raw());
            out.offset = IfxU64V1::from_u64(offset);
            out.length = IfxU64V1::from_u64(u64::from(len));
            out.transfer_max = len;
        }
        IoEvent::Patch { ticket, io_id, offset, len, incremental } => {
            out.kind = IFX_EVENT_PATCH;
            out.io_id = io_id;
            out.flags = if incremental { IFX_EVENT_FLAG_INCREMENTAL } else { 0 };
            out.ticket = IfxU64V1::from_u64(ticket.raw());
            out.offset = IfxU64V1::from_u64(offset);
            out.length = IfxU64V1::from_u64(u64::from(len));
            out.transfer_max = len;
        }
        IoEvent::Flush { ticket, io_id } => {
            out.kind = IFX_EVENT_FLUSH;
            out.io_id = io_id;
            out.ticket = IfxU64V1::from_u64(ticket.raw());
        }
        IoEvent::Commit { ticket } => {
            out.kind = IFX_EVENT_COMMIT;
            out.ticket = IfxU64V1::from_u64(ticket.raw());
        }
        IoEvent::Yield => {
            out.kind = IFX_EVENT_YIELD;
        }
        IoEvent::Complete => {
            out.kind = IFX_EVENT_COMPLETE;
        }
        IoEvent::Failed { category, cause } => {
            out.kind = IFX_EVENT_FAILED;
            out.status = category.to_outward_error_code() as u32;
            out.reason = cause.as_u32();
        }
    }
    out
}

fn decode_reply<'a>(raw: IfxReplyV1, bytes: &'a [u8]) -> Result<Reply<'a>, i32> {
    if (raw.flags & !IFX_REPLY_FLAG_HAS_VALUE) != 0 {
        return Err(IFX_INVALID_ARGUMENT);
    }
    let has_val = (raw.flags & IFX_REPLY_FLAG_HAS_VALUE) != 0;
    match raw.code {
        IFX_REPLY_BYTES => {
            if has_val {
                return Err(IFX_INVALID_ARGUMENT);
            }
            Ok(Reply::Bytes(bytes))
        }
        IFX_REPLY_END => {
            if !bytes.is_empty() {
                return Err(IFX_INVALID_ARGUMENT);
            }
            Ok(Reply::End { total_len: has_val.then(|| raw.value.to_u64()) })
        }
        IFX_REPLY_LENGTH => {
            if !bytes.is_empty() {
                return Err(IFX_INVALID_ARGUMENT);
            }
            Ok(Reply::Length(raw.value.to_u64()))
        }
        IFX_REPLY_UNKNOWN_LENGTH => {
            if !bytes.is_empty() || has_val {
                return Err(IFX_INVALID_ARGUMENT);
            }
            Ok(Reply::UnknownLength)
        }
        IFX_REPLY_ACK => {
            if !bytes.is_empty() || has_val {
                return Err(IFX_INVALID_ARGUMENT);
            }
            Ok(Reply::Ack { bytes: raw.value.to_u64() })
        }
        IFX_REPLY_ABORTED => {
            if !bytes.is_empty() || has_val {
                return Err(IFX_INVALID_ARGUMENT);
            }
            Ok(Reply::Aborted)
        }
        IFX_REPLY_IO_ERROR => Ok(Reply::IoError {
            accepted_prefix: has_val.then(|| raw.value.to_u64()),
            diagnostic: bytes,
        }),
        _ => Err(IFX_INVALID_ARGUMENT),
    }
}

fn map_session_err(err: &SessionError) -> i32 {
    match err {
        SessionError::InvalidArgument(_) => IFX_INVALID_ARGUMENT,
        SessionError::Unsupported(_) => IFX_UNSUPPORTED,
        SessionError::ResourceLimit(_) => IFX_RESOURCE_LIMIT,
        SessionError::StaleTicket => IFX_STALE_TICKET,
        SessionError::Closed => IFX_CLOSED,
        SessionError::BufferTooSmall(_) => IFX_BUFFER_TOO_SMALL,
        SessionError::Internal(_) => IFX_INTERNAL,
    }
}
