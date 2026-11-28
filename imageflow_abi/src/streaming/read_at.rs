//! Opt-in C `ReadAt` input source registration (`imageflow_context_add_input_read_at_v1`).

use core::ffi::c_void;
use core::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

use imageflow_core::media::ReadAtSource;
use imageflow_core::{ErrorKind, Result, ThreadSafeContext, here, nerror};

use crate::streaming::wire::{
    IFX_END, IFX_INTERNAL, IFX_INVALID_ARGUMENT, IFX_LENGTH_UNKNOWN, IFX_OK, IfxLenFnV1,
    IfxReadAtFnV1, IfxReadAtV1, IfxReleaseUserV1, IfxU64V1,
};

struct FfiReadAtInner {
    user: *mut c_void,
    read_at: IfxReadAtFnV1,
    length: Option<IfxLenFnV1>,
    release_user: IfxReleaseUserV1,
    /// Only `true` after `Context::add_input_read_at` succeeds; guarantees `release_user`
    /// is NEVER called on failed registration!
    owned: bool,
}

// SAFETY: Access to the raw foreign callbacks and cookie is serialized through `Mutex`
// and invoked only during synchronous context execution per the C ABI contract (§5.2).
unsafe impl Send for FfiReadAtInner {}

impl Drop for FfiReadAtInner {
    fn drop(&mut self) {
        if self.owned {
            unsafe {
                (self.release_user)(self.user);
            }
        }
    }
}

struct FfiReadAtAdapter {
    inner: Mutex<FfiReadAtInner>,
}

impl ReadAtSource for FfiReadAtAdapter {
    fn read_at(&self, offset: u64, dst: &mut [u8]) -> Result<usize> {
        if dst.is_empty() {
            return Ok(0);
        }
        let guard = self.inner.lock().map_err(|_| {
            nerror!(ErrorKind::DecodingIoError, "FFI ReadAt source lock poisoned")
        })?;
        let off = IfxU64V1::from_u64(offset);
        let mut written: usize = 0;
        let rc = unsafe {
            (guard.read_at)(guard.user, &off, dst.as_mut_ptr(), dst.len(), &mut written)
        };
        match rc {
            IFX_OK => {
                if written == 0 || written > dst.len() {
                    return Err(nerror!(
                        ErrorKind::DecodingIoError,
                        "FFI ReadAt callback returned IFX_OK with invalid written count {} (cap {})",
                        written,
                        dst.len()
                    ));
                }
                Ok(written)
            }
            IFX_END => {
                if written != 0 {
                    return Err(nerror!(
                        ErrorKind::DecodingIoError,
                        "FFI ReadAt callback returned IFX_END with non-zero written {}",
                        written
                    ));
                }
                Ok(0)
            }
            other => Err(nerror!(
                ErrorKind::DecodingIoError,
                "FFI ReadAt callback failed with code {}",
                other
            )),
        }
    }

    fn length(&self) -> Result<Option<u64>> {
        let guard = self.inner.lock().map_err(|_| {
            nerror!(ErrorKind::DecodingIoError, "FFI ReadAt source lock poisoned")
        })?;
        let Some(len_fn) = guard.length else {
            return Ok(None);
        };
        let mut out = IfxU64V1::default();
        let rc = unsafe { (len_fn)(guard.user, &mut out) };
        match rc {
            IFX_OK => Ok(Some(out.to_u64())),
            IFX_LENGTH_UNKNOWN => Ok(None),
            other => Err(nerror!(
                ErrorKind::DecodingIoError,
                "FFI ReadAt length callback failed with code {}",
                other
            )),
        }
    }
}

/// Register a synchronous random-access `ReadAt` input on a deferred `imageflow_context`.
///
/// Copies `source` during the call and takes ownership of `source->user` + `release_user`
/// ONLY on returning `IFX_OK`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn imageflow_context_add_input_read_at_v1(
    ctx: *mut ThreadSafeContext,
    io_id: i32,
    source: *const IfxReadAtV1,
) -> i32 {
    if ctx.is_null() || source.is_null() {
        return IFX_INVALID_ARGUMENT;
    }
    let res = catch_unwind(AssertUnwindSafe(|| {
        let desc = unsafe { *source };
        if desc.size as usize != size_of::<IfxReadAtV1>() || desc.flags != 0 {
            return IFX_INVALID_ARGUMENT;
        }
        let Some(read_at) = desc.read_at else {
            return IFX_INVALID_ARGUMENT;
        };
        let Some(release_user) = desc.release_user else {
            return IFX_INVALID_ARGUMENT;
        };

        let adapter = Arc::new(FfiReadAtAdapter {
            inner: Mutex::new(FfiReadAtInner {
                user: desc.user,
                read_at,
                length: desc.length,
                release_user,
                owned: false,
            }),
        });

        // Borrow shared `&ThreadSafeContext` to avoid aliasing `&mut` against concurrent
        // `imageflow_context_request_cancellation` calls!
        let c: &ThreadSafeContext = unsafe { &*ctx };
        let Ok(mut guard) = c.context_write_shared() else {
            return IFX_INTERNAL;
        };
        match guard.add_input_read_at(io_id, Arc::clone(&adapter) as Arc<dyn ReadAtSource>) {
            Ok(()) => {
                if let Ok(mut inner) = adapter.inner.lock() {
                    inner.owned = true;
                }
                IFX_OK
            }
            Err(_) => IFX_INVALID_ARGUMENT,
        }
    }));
    res.unwrap_or(IFX_INTERNAL)
}
