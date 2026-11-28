#ifndef IMAGEFLOW_STREAM_V1_H
#define IMAGEFLOW_STREAM_V1_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#if defined(_WIN32)
#define IFX_CALL __cdecl
#else
#define IFX_CALL
#endif

/* Independent streaming ABI version (major 1, minor 0). */
#define IFX_STREAM_ABI_VERSION_V1 0x00010000u

/* Feature flags returned by imageflow_stream_abi_features_v1(). */
#define IFX_STREAM_FEAT_MANUAL_V1 (1u << 0)
#define IFX_STREAM_FEAT_ASYNC_V1  (1u << 1)

/* Operation selectors for imageflow_stream_job_create_v1(). */
#define IFX_OP_EXECUTE_V1  1u
#define IFX_OP_INSPECT_V1  2u
#define IFX_OP_ESTIMATE_V1 3u

/* Return status codes for streaming ABI v1 calls and synchronous ReadAt callbacks. */
#define IFX_OK               0
#define IFX_WOULD_BLOCK      1
#define IFX_BUFFER_TOO_SMALL 2
#define IFX_STALE_TICKET     3
#define IFX_CLOSED           4
#define IFX_WRONG_THREAD     5
#define IFX_BUSY             6
#define IFX_END              7
#define IFX_LENGTH_UNKNOWN   8
#define IFX_INVALID_ARGUMENT (-1)
#define IFX_UNSUPPORTED      (-2)
#define IFX_RESOURCE_LIMIT   (-3)
#define IFX_INTERNAL         (-4)
#define IFX_IO_ERROR         (-5)

/* IfxBindingV1::flags bitmask. */
#define IFX_BIND_SOURCE           (1u << 0)
#define IFX_BIND_SINK             (1u << 1)
#define IFX_BIND_SOURCE_RANGE     (1u << 2)
#define IFX_BIND_SOURCE_KNOWN_LEN (1u << 3)
#define IFX_BIND_SINK_PATCH       (1u << 2)
#define IFX_BIND_SINK_INCREMENTAL (1u << 3)

/* IfxEventV1::kind values. */
#define IFX_EVENT_NEED_AT_LEAST 1u
#define IFX_EVENT_NEED_RANGE    2u
#define IFX_EVENT_NEED_LEN      3u
#define IFX_EVENT_OUTPUT        4u
#define IFX_EVENT_PATCH         5u
#define IFX_EVENT_FLUSH         6u
#define IFX_EVENT_COMMIT        7u
#define IFX_EVENT_YIELD         8u
#define IFX_EVENT_COMPLETE      9u
#define IFX_EVENT_FAILED        10u

/* IfxEventV1::flags bitmask. */
#define IFX_EVENT_FLAG_INCREMENTAL (1u << 0)

/* IfxEventV1::reason values on IFX_EVENT_FAILED. */
#define IFX_STOP_NONE             0u
#define IFX_STOP_CALLER_CANCEL    1u
#define IFX_STOP_DEADLINE         2u
#define IFX_STOP_RUNTIME_SHUTDOWN 3u
#define IFX_STOP_HOST_ABORT       4u

/* IfxReplyV1::code values.
 *
 * ACK units (`IfxReplyV1::value`):
 *   - IFX_EVENT_OUTPUT / IFX_EVENT_PATCH: exact positive byte count accepted (`1..=event.length`).
 *   - IFX_EVENT_FLUSH / IFX_EVENT_COMMIT: must be `0`.
 *   - IFX_REPLY_IO_ERROR with IFX_REPLY_FLAG_HAS_VALUE: accepted byte prefix within the
 *     currently offered output/patch chunk (`0..=event.length`).
 */
#define IFX_REPLY_BYTES          1u
#define IFX_REPLY_END            2u
#define IFX_REPLY_LENGTH         3u
#define IFX_REPLY_UNKNOWN_LENGTH 4u
#define IFX_REPLY_ACK            5u
#define IFX_REPLY_ABORTED        6u
#define IFX_REPLY_IO_ERROR       7u

/* IfxReplyV1::flags bitmask. */
#define IFX_REPLY_FLAG_HAS_VALUE (1u << 0)

/* IfxNotifyV1 flags bitmask. */
#define IFX_NOTIFY_EVENT (1u << 0)
#define IFX_NOTIFY_STOP  (1u << 1)

typedef struct IfxJobV1 IfxJobV1;
struct imageflow_context;

/* Word-split unsigned 64-bit integer (alignment == 4 on all 32-bit and 64-bit targets). */
typedef struct IfxU64V1 {
    uint32_t lo;
    uint32_t hi;
} IfxU64V1;

typedef struct IfxBindingV1 {
    uint32_t size;
    int32_t io_id;
    uint32_t flags;
    uint32_t reserved;
} IfxBindingV1;

typedef struct IfxEventV1 {
    uint32_t size;
    uint32_t kind;
    int32_t io_id;
    uint32_t flags;
    IfxU64V1 ticket;
    IfxU64V1 offset;
    IfxU64V1 length;
    uint32_t transfer_max;
    uint32_t status;
    uint32_t reason;
    uint32_t reserved;
} IfxEventV1;

typedef struct IfxReplyV1 {
    uint32_t size;
    uint32_t code;
    uint32_t flags;
    uint32_t reserved;
    IfxU64V1 ticket;
    IfxU64V1 value;
} IfxReplyV1;

/* Host async callbacks (transferred ONLY on IFX_OK from imageflow_stream_job_start_async_v1).
 *
 * Concurrency & quiescence contract:
 *   - `notify` runs serially on a dedicated runtime thread with NO internal job mutex held;
 *     it may safely call `imageflow_stream_job_cancel_v1`, `next_v1`, `copy_pending_v1`,
 *     or `reply_v1` without deadlocking, and MUST NOT unwind/throw across FFI.
 *   - `release_user` is invoked EXACTLY ONCE after the worker exits and the final `notify`
 *     call has returned; it is the sole legal point to free `user` after a successful start.
 */
typedef void (IFX_CALL *IfxNotifyV1)(void *user, uint32_t flags);
typedef void (IFX_CALL *IfxReleaseUserV1)(void *user);

typedef struct IfxAsyncHooksV1 {
    uint32_t size;
    uint32_t flags;
    void *user;
    IfxNotifyV1 notify;
    IfxReleaseUserV1 release_user;
} IfxAsyncHooksV1;

/* Synchronous random-access ReadAt callbacks for deferred contexts.
 *
 * `read_at` must return IFX_OK with `1 <= *written <= capacity`, IFX_END with `*written == 0`
 * on EOF, or a negative status code on error. `length` is optional (NULL means unknown length);
 * when non-NULL it returns IFX_OK with `*length` set, IFX_LENGTH_UNKNOWN, or negative error.
 * Neither callback may unwind/throw across FFI.
 */
typedef int32_t (IFX_CALL *IfxReadAtFnV1)(
    void *user,
    const IfxU64V1 *offset,
    uint8_t *dst,
    size_t capacity,
    size_t *written);
typedef int32_t (IFX_CALL *IfxLenFnV1)(void *user, IfxU64V1 *length);

typedef struct IfxReadAtV1 {
    uint32_t size;
    uint32_t flags;
    void *user;
    IfxReadAtFnV1 read_at;
    IfxLenFnV1 length;
    IfxReleaseUserV1 release_user;
} IfxReadAtV1;

uint32_t IFX_CALL imageflow_stream_abi_version_v1(void);
uint32_t IFX_CALL imageflow_stream_abi_features_v1(void);

/* Register a ReadAt input on a deferred `ctx` (context must stay alive for the duration of
 * this call; synchronized via shared interior lock so concurrent `request_cancellation` is
 * sound). Copies `*source` during the call; takes ownership of `source->user` + `release_user`
 * ONLY on returning IFX_OK (invoking `release_user` once when `ctx` is destroyed).
 */
int32_t IFX_CALL imageflow_context_add_input_read_at_v1(
    struct imageflow_context *ctx,
    int32_t io_id,
    const IfxReadAtV1 *source);

/* Create a frozen streaming job from `ctx` policy, JSON envelope, and bindings.
 * `ctx`, `json`, and `bindings` need only stay valid for the duration of this call; the
 * created `IfxJobV1` retains ZERO pointer to `ctx` and safely outlives `imageflow_context_destroy`.
 * Sets `*out_job = NULL` on error.
 */
int32_t IFX_CALL imageflow_stream_job_create_v1(
    struct imageflow_context *ctx,
    uint32_t operation,
    const uint8_t *json,
    size_t json_len,
    const IfxBindingV1 *bindings,
    size_t count,
    IfxJobV1 **out_job);

/* Step a manual job (`work > 0`, thread-affine after first step; foreign threads receive
 * IFX_WRONG_THREAD) or poll an async job (`work == 0`, returns IFX_WOULD_BLOCK when no new
 * event is visible). Driver calls (`next`, `copy_pending`, `reply`, `result_json`,
 * `start_async`) are non-reentrant per job (concurrent overlap returns IFX_BUSY), while
 * `cancel_v1` may overlap ANY driver call concurrently.
 */
int32_t IFX_CALL imageflow_stream_job_next_v1(
    IfxJobV1 *job,
    uint32_t work,
    IfxEventV1 *out_event);

/* Copy pending output/patch bytes for `ticket` into caller buffer `dst` (valid for call
 * duration only; idempotent until `ticket` is ACKed). On IFX_BUFFER_TOO_SMALL, `*written`
 * receives the exact required byte count.
 */
int32_t IFX_CALL imageflow_stream_job_copy_pending_v1(
    IfxJobV1 *job,
    const IfxU64V1 *ticket,
    uint8_t *dst,
    size_t cap,
    size_t *written);

/* Deliver a host reply for the active outstanding ticket. `bytes[0..len]` is copied/consumed
 * synchronously during this call and needs no lifetime beyond return. In async mode, blocks
 * only briefly for worker state-machine validation (zero codec compute runs before returning).
 */
int32_t IFX_CALL imageflow_stream_job_reply_v1(
    IfxJobV1 *job,
    const IfxReplyV1 *reply,
    const uint8_t *bytes,
    size_t len);

/* Copy finalized UTF-8 result or structured error JSON once terminal (`IFX_WOULD_BLOCK` before
 * terminal). On IFX_BUFFER_TOO_SMALL, `*needed` receives exact required byte count.
 */
int32_t IFX_CALL imageflow_stream_job_result_json_v1(
    IfxJobV1 *job,
    uint8_t *dst,
    size_t cap,
    size_t *needed);

/* Thread-safe instant cancellation; may safely overlap any live driver call on `job` (but
 * MUST NOT race a consuming `imageflow_stream_job_release_v1`).
 */
int32_t IFX_CALL imageflow_stream_job_cancel_v1(IfxJobV1 *job);

/* Consume and free `job` (NULL returns IFX_OK). Caller MUST ensure NO other call (including
 * `cancel_v1`) overlaps a consuming release. Manual jobs bound to an owner thread return
 * IFX_WRONG_THREAD without consuming `job` when called from a foreign thread. Async jobs may
 * be released from any thread (e.g. .NET SafeHandle); releasing abandons any unfinished work
 * (including an in-flight Commit) and triggers non-blocking quiescence + single `release_user`.
 */
int32_t IFX_CALL imageflow_stream_job_release_v1(IfxJobV1 *job);

/* Present only when built with feature `async-ffi`.
 * Transitions an un-stepped manual job to Tokio async execution. Takes ownership of
 * `hooks->user` + `hooks->release_user` ONLY on returning IFX_OK; on any error the caller
 * retains ownership and neither callback is invoked.
 */
int32_t IFX_CALL imageflow_stream_job_start_async_v1(
    IfxJobV1 *job,
    const IfxAsyncHooksV1 *hooks);

#ifdef __cplusplus
}
#endif

#endif /* IMAGEFLOW_STREAM_V1_H */
