//! Opt-in C ABI v1 wire records, constants, and compile-time layout proofs (`§5.1`–`§5.2`).
//!
//! Matches `imageflow_abi/include/imageflow_stream_v1.h` byte-for-byte on both
//! 32-bit (i686 Linux / Windows x86) and 64-bit (x86_64 / aarch64) targets.

use core::ffi::c_void;
use core::mem::{offset_of, size_of};

pub const IFX_STREAM_ABI_VERSION_V1: u32 = 0x0001_0000;
pub const IFX_STREAM_FEAT_MANUAL_V1: u32 = 1 << 0;
pub const IFX_STREAM_FEAT_ASYNC_V1: u32 = 1 << 1;
pub const IFX_STREAM_FEAT_PROGRESS_V1: u32 = 1 << 2;

pub const IFX_HOOK_PROGRESS_PULL: u32 = 1 << 0;
pub const IFX_HOOK_PROGRESS_NOTIFY: u32 = 1 << 1;

pub const IFX_NOTIFY_EVENT: u32 = 1 << 0;
pub const IFX_NOTIFY_STOP: u32 = 1 << 1;
pub const IFX_NOTIFY_PROGRESS: u32 = 1 << 2;

pub const IFX_PROGRESS_DISABLED: u32 = 0;
pub const IFX_PROGRESS_NOT_STARTED: u32 = 1;
pub const IFX_PROGRESS_RUNNING: u32 = 2;
pub const IFX_PROGRESS_SUCCEEDED: u32 = 3;
pub const IFX_PROGRESS_FAILED: u32 = 4;
pub const IFX_PROGRESS_CANCELLED: u32 = 5;
pub const IFX_PROGRESS_ABANDONED: u32 = 6;

pub const IFX_PROGRESS_FLAG_HAS_FRACTION: u32 = 1 << 0;
pub const IFX_PROGRESS_FLAG_TERMINAL: u32 = 1 << 1;

pub const IFX_OK: i32 = 0;
pub const IFX_WOULD_BLOCK: i32 = 1;
pub const IFX_BUFFER_TOO_SMALL: i32 = 2;
pub const IFX_STALE_TICKET: i32 = 3;
pub const IFX_CLOSED: i32 = 4;
pub const IFX_WRONG_THREAD: i32 = 5;
pub const IFX_BUSY: i32 = 6;
pub const IFX_END: i32 = 7;
pub const IFX_LENGTH_UNKNOWN: i32 = 8;
pub const IFX_INVALID_ARGUMENT: i32 = -1;
pub const IFX_UNSUPPORTED: i32 = -2;
pub const IFX_RESOURCE_LIMIT: i32 = -3;
pub const IFX_INTERNAL: i32 = -4;
pub const IFX_IO_ERROR: i32 = -5;

pub const IFX_EVENT_NEED_AT_LEAST: u32 = 1;
pub const IFX_EVENT_NEED_RANGE: u32 = 2;
pub const IFX_EVENT_NEED_LEN: u32 = 3;
pub const IFX_EVENT_OUTPUT: u32 = 4;
pub const IFX_EVENT_PATCH: u32 = 5;
pub const IFX_EVENT_FLUSH: u32 = 6;
pub const IFX_EVENT_COMMIT: u32 = 7;
pub const IFX_EVENT_YIELD: u32 = 8;
pub const IFX_EVENT_COMPLETE: u32 = 9;
pub const IFX_EVENT_FAILED: u32 = 10;

pub const IFX_EVENT_FLAG_INCREMENTAL: u32 = 1 << 0;

pub const IFX_REPLY_BYTES: u32 = 1;
pub const IFX_REPLY_END: u32 = 2;
pub const IFX_REPLY_LENGTH: u32 = 3;
pub const IFX_REPLY_UNKNOWN_LENGTH: u32 = 4;
pub const IFX_REPLY_ACK: u32 = 5;
pub const IFX_REPLY_ABORTED: u32 = 6;
pub const IFX_REPLY_IO_ERROR: u32 = 7;

pub const IFX_REPLY_FLAG_HAS_VALUE: u32 = 1 << 0;

/// Word-split 64-bit unsigned integer (`alignment == 4` on every target, endian-independent).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IfxU64V1 {
    pub lo: u32,
    pub hi: u32,
}

impl IfxU64V1 {
    #[inline]
    pub const fn from_u64(v: u64) -> Self {
        Self { lo: v as u32, hi: (v >> 32) as u32 }
    }

    #[inline]
    pub const fn to_u64(self) -> u64 {
        ((self.hi as u64) << 32) | (self.lo as u64)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IfxBindingV1 {
    pub size: u32,
    pub io_id: i32,
    pub flags: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IfxEventV1 {
    pub size: u32,
    pub kind: u32,
    pub io_id: i32,
    pub flags: u32,
    pub ticket: IfxU64V1,
    pub offset: IfxU64V1,
    pub length: IfxU64V1,
    pub transfer_max: u32,
    pub status: u32,
    pub reason: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IfxReplyV1 {
    pub size: u32,
    pub code: u32,
    pub flags: u32,
    pub reserved: u32,
    pub ticket: IfxU64V1,
    pub value: IfxU64V1,
}

/// Fixed-layout non-blocking progress snapshot (40 bytes; identical offsets on 32-bit & 64-bit).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IfxProgressV1 {
    pub size: u32,
    pub status: u32,
    pub flags: u32,
    pub reserved: u32,
    pub fraction: f64,
    pub unresolved_fraction: f64,
    pub generation: IfxU64V1,
}

pub type IfxNotifyV1 = unsafe extern "C" fn(*mut c_void, u32);
pub type IfxReleaseUserV1 = unsafe extern "C" fn(*mut c_void);
pub type IfxReadAtFnV1 =
    unsafe extern "C" fn(*mut c_void, *const IfxU64V1, *mut u8, usize, *mut usize) -> i32;
pub type IfxLenFnV1 = unsafe extern "C" fn(*mut c_void, *mut IfxU64V1) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IfxAsyncHooksV1 {
    pub size: u32,
    pub flags: u32,
    pub user: *mut c_void,
    pub notify: Option<IfxNotifyV1>,
    pub release_user: Option<IfxReleaseUserV1>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IfxReadAtV1 {
    pub size: u32,
    pub flags: u32,
    pub user: *mut c_void,
    pub read_at: Option<IfxReadAtFnV1>,
    pub length: Option<IfxLenFnV1>,
    pub release_user: Option<IfxReleaseUserV1>,
}

// Compile-time ABI layout proofs matching §5.1 table verbatim on both 32-bit and 64-bit targets.
const _: () = {
    assert!(size_of::<IfxU64V1>() == 8);
    assert!(offset_of!(IfxU64V1, lo) == 0);
    assert!(offset_of!(IfxU64V1, hi) == 4);

    assert!(size_of::<IfxBindingV1>() == 16);
    assert!(offset_of!(IfxBindingV1, size) == 0);
    assert!(offset_of!(IfxBindingV1, io_id) == 4);
    assert!(offset_of!(IfxBindingV1, flags) == 8);
    assert!(offset_of!(IfxBindingV1, reserved) == 12);

    assert!(size_of::<IfxEventV1>() == 56);
    assert!(offset_of!(IfxEventV1, size) == 0);
    assert!(offset_of!(IfxEventV1, kind) == 4);
    assert!(offset_of!(IfxEventV1, io_id) == 8);
    assert!(offset_of!(IfxEventV1, flags) == 12);
    assert!(offset_of!(IfxEventV1, ticket) == 16);
    assert!(offset_of!(IfxEventV1, offset) == 24);
    assert!(offset_of!(IfxEventV1, length) == 32);
    assert!(offset_of!(IfxEventV1, transfer_max) == 40);
    assert!(offset_of!(IfxEventV1, status) == 44);
    assert!(offset_of!(IfxEventV1, reason) == 48);
    assert!(offset_of!(IfxEventV1, reserved) == 52);

    assert!(size_of::<IfxReplyV1>() == 32);
    assert!(offset_of!(IfxReplyV1, size) == 0);
    assert!(offset_of!(IfxReplyV1, code) == 4);
    assert!(offset_of!(IfxReplyV1, flags) == 8);
    assert!(offset_of!(IfxReplyV1, reserved) == 12);
    assert!(offset_of!(IfxReplyV1, ticket) == 16);
    assert!(offset_of!(IfxReplyV1, value) == 24);

    assert!(size_of::<IfxProgressV1>() == 40);
    assert!(offset_of!(IfxProgressV1, size) == 0);
    assert!(offset_of!(IfxProgressV1, status) == 4);
    assert!(offset_of!(IfxProgressV1, flags) == 8);
    assert!(offset_of!(IfxProgressV1, reserved) == 12);
    assert!(offset_of!(IfxProgressV1, fraction) == 16);
    assert!(offset_of!(IfxProgressV1, unresolved_fraction) == 24);
    assert!(offset_of!(IfxProgressV1, generation) == 32);

    #[cfg(target_pointer_width = "64")]
    {
        assert!(size_of::<IfxAsyncHooksV1>() == 32);
        assert!(offset_of!(IfxAsyncHooksV1, size) == 0);
        assert!(offset_of!(IfxAsyncHooksV1, flags) == 4);
        assert!(offset_of!(IfxAsyncHooksV1, user) == 8);
        assert!(offset_of!(IfxAsyncHooksV1, notify) == 16);
        assert!(offset_of!(IfxAsyncHooksV1, release_user) == 24);

        assert!(size_of::<IfxReadAtV1>() == 40);
        assert!(offset_of!(IfxReadAtV1, size) == 0);
        assert!(offset_of!(IfxReadAtV1, flags) == 4);
        assert!(offset_of!(IfxReadAtV1, user) == 8);
        assert!(offset_of!(IfxReadAtV1, read_at) == 16);
        assert!(offset_of!(IfxReadAtV1, length) == 24);
        assert!(offset_of!(IfxReadAtV1, release_user) == 32);
    }

    #[cfg(target_pointer_width = "32")]
    {
        assert!(size_of::<IfxAsyncHooksV1>() == 20);
        assert!(offset_of!(IfxAsyncHooksV1, size) == 0);
        assert!(offset_of!(IfxAsyncHooksV1, flags) == 4);
        assert!(offset_of!(IfxAsyncHooksV1, user) == 8);
        assert!(offset_of!(IfxAsyncHooksV1, notify) == 12);
        assert!(offset_of!(IfxAsyncHooksV1, release_user) == 16);

        assert!(size_of::<IfxReadAtV1>() == 24);
        assert!(offset_of!(IfxReadAtV1, size) == 0);
        assert!(offset_of!(IfxReadAtV1, flags) == 4);
        assert!(offset_of!(IfxReadAtV1, user) == 8);
        assert!(offset_of!(IfxReadAtV1, read_at) == 12);
        assert!(offset_of!(IfxReadAtV1, length) == 16);
        assert!(offset_of!(IfxReadAtV1, release_user) == 20);
    }
};
