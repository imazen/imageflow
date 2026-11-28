//! Opt-in streaming ABI v1 module (`imageflow_abi/include/imageflow_stream_v1.h`).
//!
//! Marked `cbindgen:ignore` and explicitly excluded in `build.rs` so all legacy generated
//! headers in `bindings/headers/imageflow_*.h` remain byte-for-byte unchanged.

pub mod job;
pub mod read_at;
pub mod wire;

pub use job::{
    IfxJobV1, imageflow_stream_abi_features_v1, imageflow_stream_abi_version_v1,
    imageflow_stream_job_cancel_v1, imageflow_stream_job_copy_pending_v1,
    imageflow_stream_job_create_v1, imageflow_stream_job_next_v1, imageflow_stream_job_release_v1,
    imageflow_stream_job_reply_v1, imageflow_stream_job_result_json_v1,
};
#[cfg(feature = "async-ffi")]
pub use job::imageflow_stream_job_start_async_v1;
pub use read_at::imageflow_context_add_input_read_at_v1;
pub use wire::*;
