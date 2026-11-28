use crate::errors::OutwardErrorBuffer;
use crate::flow::definitions::Graph;
use crate::for_other_imageflow_crates::preludes::external_without_std::*;
use crate::io::IoProxy;
use crate::{ErrorKind, FlowError, JsonResponse, Result};
use enough::{Stop, StopReason};
use imageflow_types::collections::AddRemoveSet;
use imageflow_types::IoDirection;
use std::any::Any;
#[cfg(debug_assertions)]
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize};
use std::sync::Arc;
use std::sync::*;

use crate::allocation_container::AllocationContainer;
use crate::codecs::CodecInstanceContainer;
use crate::codecs::EnabledCodecs;
use crate::graphics::bitmaps::{Bitmap, BitmapKey, BitmapWindowMut, BitmapsContainer};
use imageflow_types::ImageInfo;
use itertools::Itertools;

/// Something of a god object (which is necessary for a reasonable FFI interface).
///
/// Size budget, enforced by `test_context_size` / `test_thread_safe_context_size` /
/// `test_calculate_context_heap_size` below: `Context` < 400 bytes on the stack,
/// `ThreadSafeContext` <= 560 (it is `Context` + 184 bytes of locks and the token),
/// and <= 1056 bytes / 6 allocations for stack + initial heap combined. Anything
/// added here — or to any type embedded here, notably
/// `imageflow_types::ExecutionSecurity` — is charged against that budget, so keep
/// cold/optional state boxed.
pub struct Context {
    pub debug_job_id: i32,
    pub(crate) next_stable_node_id: i32,
    pub(crate) next_graph_version: i32,
    pub(crate) max_calc_flatten_execute_passes: i32,
    pub(crate) graph_recording: s::Build001GraphRecording,
    cancellation_token: CancellationToken,

    /// Codecs, which in turn connect to I/O instances.
    pub(crate) codecs: AddRemoveSet<CodecInstanceContainer>, // This loans out exclusive mutable references to items, bounding the ownership lifetime to Context
    /// A list of io_ids already in use
    pub(crate) io_id_list: RefCell<Vec<i32>>,

    pub enabled_codecs: EnabledCodecs,

    pub security: imageflow_types::ExecutionSecurity,

    pub(crate) bitmaps: RefCell<crate::graphics::bitmaps::BitmapsContainer>,

    // NOTE: there is deliberately no `allocations` container here. Caller-visible
    // allocations (`imageflow_context_memory_allocate`/`_free`) are owned by
    // `ThreadSafeContext::allocations`; a second, never-read `RefCell<AllocationContainer>`
    // used to sit here and cost 32 bytes of the `Context` size budget above.
    /// Cold/optional per-context state boxed into a single 8-byte `Option<Box<..>>` slot
    /// so `Context` preserves its exact `< 400 B` stack budget and `<= 1056 B / 6 allocs`
    /// initial heap budget. Nothing is allocated until a CaptureBitmapKey node runs or
    /// modern registration/configuration is invoked.
    extras: Option<Box<ContextExtras>>,
}

/// Replayable input lease registered on a [`Context`] for modern Zen/media execution.
#[derive(Clone)]
pub(crate) enum ModernInputLease {
    Borrowed(&'static [u8]),
    Shared(Arc<Vec<u8>>),
    #[cfg(feature = "streaming")]
    ReadAt(Arc<dyn crate::media::ReadAtSource>),
}

/// Modern policy configuration frozen per job on [`Context`] via `v2/configure_context`.
#[derive(Clone, Debug, Default)]
pub struct ModernContextConfig {
    /// When `true`, `add_*_input_*` stores a replayable lease without eager legacy probing.
    pub input_registration_deferred: bool,
    /// When `true`, opts into `WorkspacePolicy::BestEffort`; default `false` is `Strict`.
    pub workspace_policy_best_effort: bool,
    /// Optional explicit live memory cap in bytes (intersected with `ExecutionSecurity`).
    pub max_memory_bytes: Option<u64>,
    /// Optional explicit output byte cap.
    pub max_output_bytes: Option<u64>,
    /// Optional explicit input byte cap.
    pub max_input_bytes: Option<u64>,
    /// Optional explicit frame count cap.
    pub max_frames: Option<u32>,
    /// Optional explicit max canvas width/height cap.
    pub max_dimension: Option<u32>,
    /// Optional route trace entry capacity.
    pub trace_capacity: Option<u32>,
    /// Optional wall-clock job deadline in milliseconds for async streaming starts.
    pub timeout_ms: Option<u64>,
}

#[derive(Default)]
struct ContextExtras {
    captured_bitmap_keys: std::collections::HashMap<i32, BitmapKey>,
    modern_inputs: std::collections::HashMap<i32, ModernInputLease>,
    modern_config: ModernContextConfig,
}

// This token is the shared state.
#[derive(Default)]
pub(crate) struct CancellationToken {
    flag: Arc<AtomicBool>,
    #[cfg(debug_assertions)]
    poll_countdown: Arc<AtomicI64>,
}

impl CancellationToken {
    // The blocking task will call this.
    #[cfg(debug_assertions)]
    #[inline]
    pub fn cancellation_requested(&self) -> bool {
        if self.flag.load(Ordering::Relaxed) {
            return true;
        }
        if self.poll_countdown.load(Ordering::Relaxed) == i64::MAX {
            return false; // No need to mutate, it's not been set
        }
        self.poll_countdown.fetch_sub(1, Ordering::Relaxed) < 1
    }

    #[inline]
    #[cfg(not(debug_assertions))]
    pub fn cancellation_requested(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }

    fn cancel_internal(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }

    #[cfg(not(debug_assertions))]
    pub fn new() -> CancellationToken {
        CancellationToken { flag: Arc::new(AtomicBool::new(false)) }
    }

    #[cfg(debug_assertions)]
    pub fn new() -> CancellationToken {
        CancellationToken {
            flag: Arc::new(AtomicBool::new(false)),
            poll_countdown: Arc::new(AtomicI64::new(i64::MAX)),
        }
    }
    #[cfg(debug_assertions)]
    pub fn request_cancellation_after_n_polls(&self, cancel_after_polls: i64) {
        self.poll_countdown.store(cancel_after_polls, Ordering::SeqCst);
        // eprintln!("Requesting cancellation after {} polls", cancel_after_polls);
        // eprintln!("Poll count remaining: {}", self.poll_countdown.load(Ordering::SeqCst));
    }
    #[cfg(debug_assertions)]
    pub fn request_cancellation_after_n_polls_remaining(&self) -> i64 {
        self.poll_countdown.load(Ordering::SeqCst)
    }
}

impl Clone for CancellationToken {
    fn clone(&self) -> Self {
        Self {
            flag: self.flag.clone(),
            #[cfg(debug_assertions)]
            poll_countdown: self.poll_countdown.clone(),
        }
    }
}

impl Stop for CancellationToken {
    #[inline]
    fn check(&self) -> std::result::Result<(), StopReason> {
        if self.cancellation_requested() {
            Err(StopReason::Cancelled)
        } else {
            Ok(())
        }
    }

    #[inline]
    fn should_stop(&self) -> bool {
        self.cancellation_requested()
    }
}

static NEXT_JOB_ID: AtomicI32 = AtomicI32::new(0);

// We need this for ABI callers to ensure safety
pub struct ThreadSafeContext {
    context: std::sync::RwLock<Context>,
    /// Buffer for errors presented to users of an FFI interface; locked separately from the context
    outward_error: std::sync::RwLock<OutwardErrorBuffer>,
    /// Need to be able to cancel when other tasks are running
    cancellation_token: CancellationToken,
    allocations: std::sync::Mutex<AllocationContainer>,
}
impl ThreadSafeContext {
    pub fn create_can_panic() -> Result<Box<ThreadSafeContext>> {
        let cancellation_token = CancellationToken::new();
        Ok(Box::new(ThreadSafeContext {
            context: std::sync::RwLock::new(Context::create_can_panic_unboxed(
                cancellation_token.clone(),
            )?),
            outward_error: std::sync::RwLock::new(OutwardErrorBuffer::new()),
            allocations: std::sync::Mutex::new(AllocationContainer::new()),
            cancellation_token,
        }))
    }
    pub fn create_cant_panic() -> Result<Box<ThreadSafeContext>> {
        std::panic::catch_unwind(ThreadSafeContext::create_can_panic)
            .unwrap_or_else(|_| Err(err_oom!())) //err_oom because it doesn't allocate anything.
    }

    /// Request cancellation using a shared reference (sound under concurrent FFI cancellation
    /// while another thread holds the context lock).
    pub fn request_cancellation_shared(&self) {
        self.cancellation_token.cancel_internal();
        self.outward_error
            .write()
            .expect("OutwardErrorBuffer.write failed: lock poisoned from a panic")
            .try_set_error(nerror!(ErrorKind::OperationCancelled, "Cancellation was requested"));
    }

    pub fn request_cancellation(&mut self) {
        self.request_cancellation_shared();
    }
    #[cfg(debug_assertions)]
    pub fn request_cancellation_after_n_polls(&self, cancel_after_polls: i64) {
        self.cancellation_token.request_cancellation_after_n_polls(cancel_after_polls);
    }
    #[cfg(debug_assertions)]
    pub fn request_cancellation_after_n_polls_remaining(&self) -> i64 {
        self.cancellation_token.request_cancellation_after_n_polls_remaining()
    }

    pub fn outward_error(&self) -> RwLockReadGuard<'_, OutwardErrorBuffer> {
        self.outward_error
            .read()
            .expect("OutwardErrorBuffer.write failed: lock poisoned from a panic")
    }
    /// Acquire a write guard on the outward error buffer through a shared reference (avoids
    /// creating aliasing `&mut ThreadSafeContext` references at FFI boundaries).
    pub fn outward_error_write_shared(&self) -> RwLockWriteGuard<'_, OutwardErrorBuffer> {
        self.outward_error
            .write()
            .expect("OutwardErrorBuffer.write failed: lock poisoned from a panic")
    }

    pub fn outward_error_mut(&mut self) -> RwLockWriteGuard<'_, OutwardErrorBuffer> {
        self.outward_error_write_shared()
    }

    /// Acquire a read guard on the inner [`Context`] through a shared reference (avoids
    /// creating aliasing `&mut ThreadSafeContext` references at FFI boundaries).
    pub fn context_read_shared(&self) -> LockResult<RwLockReadGuard<'_, Context>> {
        self.context.read()
    }

    /// Acquire a write guard on the inner [`Context`] through a shared reference (avoids
    /// creating aliasing `&mut ThreadSafeContext` references at FFI boundaries).
    pub fn context_write_shared(&self) -> LockResult<RwLockWriteGuard<'_, Context>> {
        self.context.write()
    }

    /// Acquire write guards on both outward error buffer and inner [`Context`] through a
    /// shared reference (sound under concurrent `imageflow_context_request_cancellation`).
    pub fn context_and_error_write_shared(
        &self,
    ) -> (RwLockWriteGuard<'_, OutwardErrorBuffer>, LockResult<RwLockWriteGuard<'_, Context>>) {
        let error = self.outward_error_write_shared();
        let context = self.context.write();
        (error, context)
    }

    pub fn context_mut_or_poisoned(&mut self) -> LockResult<RwLockWriteGuard<'_, Context>> {
        self.context.write()
    }
    pub fn context_mut_and_error_or_poisoned(
        &mut self,
    ) -> (RwLockWriteGuard<'_, OutwardErrorBuffer>, LockResult<RwLockWriteGuard<'_, Context>>) {
        let error = self
            .outward_error
            .write()
            .expect("OutwardErrorBuffer.write failed: lock poisoned from a panic");
        let context = self.context.write();
        (error, context)
    }
    pub fn context_or_poisoned(&mut self) -> LockResult<RwLockWriteGuard<'_, Context>> {
        self.context.write()
    }
    /// Used by abi; should not panic
    pub fn abi_begin_terminate(&mut self) -> bool {
        if let Ok(mut result) = self.context_mut_or_poisoned() {
            let _ = result.destroy_without_drop();
        }
        true
    }

    /// Calculates the total size and count of all heap allocations in a new ThreadSafeContext
    /// Returns (total_bytes, num_allocations)
    ///
    /// This includes:
    /// - Initial heap allocations for collections (codecs, io_id_list, bitmaps, allocations)
    /// - Arc allocations for shared state
    ///
    /// Note: RwLock and Mutex store their contents inline, not on the heap
    pub(crate) fn calculate_heap_allocations() -> (usize, usize) {
        // Get Context's heap allocations (this is shared via RwLock but stored inline in ThreadSafeContext)
        let (context_bytes, context_allocs) = Context::calculate_heap_allocations();

        (
            context_bytes + std::mem::size_of::<ThreadSafeContext>()
                - std::mem::size_of::<Context>(),
            context_allocs,
        )
    }
    /// Allocates zeroed memory tracked by this context. The caller must ensure
    /// the returned pointer is eventually freed via `mem_free`.
    pub fn mem_calloc(
        &self,
        bytes: usize,
        alignment: usize,
        filename: Option<&str>,
        line: i32,
    ) -> Result<*mut u8> {
        let filename_str = filename.unwrap_or("[no filename provided]");

        let mut allocations = self.allocations.lock().map_err(|e| {
            nerror!(
                ErrorKind::FailedBorrow,
                "Cannot allocate due to a previous allocation failure on this Context - make a new Context and drop this one: {:?}\n{}:{}",
                e,
                filename_str,
                line
            )
        })?;

        let result = allocations.allocate(bytes, alignment).map_err(|e| {
            nerror!(
                ErrorKind::AllocationFailed,
                "Failed to allocate {} bytes with alignment {}: {:?}\n{}:{}",
                bytes,
                alignment,
                e,
                filename_str,
                line
            )
        })?;
        Ok(result)
    }

    /// Frees memory previously allocated by `mem_calloc` on this context.
    /// Returns true if the pointer was found and freed.
    pub fn mem_free(&self, ptr: *const u8) -> bool {
        self.allocations.lock().map(|mut list| list.free(ptr)).unwrap_or(false)
    }
}

// impl drop for ThreadSafeContext and try to lock on allocations, context, and error to avoid bad references
// We don't care about panics, that should be handled.
impl Drop for ThreadSafeContext {
    fn drop(&mut self) {
        drop(self.allocations.lock());
        drop(self.context.write());
        drop(self.outward_error.write());
    }
}

impl Context {
    pub fn create() -> Result<Box<Context>> {
        Context::create_cant_panic()
    }

    pub fn create_can_panic() -> Result<Box<Context>> {
        Ok(Box::new(Context {
            debug_job_id: NEXT_JOB_ID.fetch_add(1, Ordering::Relaxed),
            next_graph_version: 0,
            next_stable_node_id: 0,
            max_calc_flatten_execute_passes: 40,
            graph_recording: s::Build001GraphRecording::off(),
            codecs: AddRemoveSet::with_capacity(Self::default_codecs_capacity()),
            io_id_list: RefCell::new(Vec::with_capacity(Self::default_codecs_capacity())),
            cancellation_token: CancellationToken::new(),
            enabled_codecs: EnabledCodecs::default(),
            bitmaps: RefCell::new(
                crate::graphics::bitmaps::BitmapsContainer::with_default_capacity(),
            ),
            security: imageflow_types::ExecutionSecurity::sane_defaults(),
            extras: None,
        }))
    }
    fn default_codecs_capacity() -> usize {
        2
    }
    fn create_can_panic_unboxed(cancellation_token: CancellationToken) -> Result<Context> {
        Ok(Context {
            debug_job_id: NEXT_JOB_ID.fetch_add(1, Ordering::Relaxed),
            next_graph_version: 0,
            next_stable_node_id: 0,
            max_calc_flatten_execute_passes: 40,
            graph_recording: s::Build001GraphRecording::off(),
            codecs: AddRemoveSet::with_capacity(Self::default_codecs_capacity()),
            io_id_list: RefCell::new(Vec::with_capacity(Self::default_codecs_capacity())),
            cancellation_token,
            enabled_codecs: EnabledCodecs::default(),
            bitmaps: RefCell::new(
                crate::graphics::bitmaps::BitmapsContainer::with_default_capacity(),
            ),
            security: imageflow_types::ExecutionSecurity::sane_defaults(),
            extras: None,
        })
    }
    fn create_with_cancellation_token_and_can_panic(
        cancellation_token: CancellationToken,
    ) -> Result<Box<Context>> {
        Ok(Box::new(Context {
            debug_job_id: NEXT_JOB_ID.fetch_add(1, Ordering::Relaxed),
            next_graph_version: 0,
            next_stable_node_id: 0,
            max_calc_flatten_execute_passes: 40,
            graph_recording: s::Build001GraphRecording::off(),
            codecs: AddRemoveSet::with_capacity(Self::default_codecs_capacity()),
            io_id_list: RefCell::new(Vec::with_capacity(Self::default_codecs_capacity())),
            cancellation_token,
            enabled_codecs: EnabledCodecs::default(),
            bitmaps: RefCell::new(
                crate::graphics::bitmaps::BitmapsContainer::with_default_capacity(),
            ),
            security: imageflow_types::ExecutionSecurity::sane_defaults(),
            extras: None,
        }))
    }

    pub fn create_cant_panic() -> Result<Box<Context>> {
        std::panic::catch_unwind(|| {
            // Upgrade backtraces
            // Disable backtraces for debugging across the FFI boundary
            //imageflow_helpers::debug::upgrade_panic_hook_once_if_backtraces_wanted();

            Context::create_can_panic()
        })
        .unwrap_or_else(|_| Err(err_oom!())) //err_oom because it doesn't allocate anything.
    }
    pub fn destroy_without_drop(&mut self) -> Result<()> {
        self.codecs.mut_clear();
        Ok(())
    }

    pub fn destroy(mut self) -> Result<()> {
        self.codecs.mut_clear();
        Ok(())
    }

    #[inline]
    pub fn stop(&self) -> &dyn Stop {
        &self.cancellation_token
    }

    /// An owned, cloneable cancellation token for codec jobs that take ownership
    /// of a stop signal (zencodec `set_pulse` stores a `SharedPulse` for the
    /// duration of decode/encode). Shares the same flag as [`Context::stop`].
    #[cfg(any(feature = "bmp", feature = "zen-media", feature = "streaming"))]
    #[inline]
    pub(crate) fn cancellation_token(&self) -> impl Stop + Clone + 'static {
        self.cancellation_token.clone()
    }

    pub fn message(&mut self, method: &str, json: &[u8]) -> (JsonResponse, Result<()>) {
        match crate::modern_endpoints::try_invoke(self, method, json) {
            Ok(Some(response)) => (response, Ok(())),
            Err(e) => (JsonResponse::from_flow_error(&e), Err(e)),
            Ok(None) => crate::json::invoke_with_json_error(self, method, json),
        }
    }

    pub fn borrow_bitmaps_mut(&self) -> Result<RefMut<'_, BitmapsContainer>> {
        return_if_cancelled!(self);
        self.bitmaps.try_borrow_mut().map_err(|e| {
            nerror!(ErrorKind::FailedBorrow, "Failed to mutably borrow bitmaps collection: {:?}", e)
        })
    }
    pub fn borrow_bitmaps(&self) -> Result<Ref<'_, BitmapsContainer>> {
        return_if_cancelled!(self);
        self.bitmaps.try_borrow().map_err(|e| {
            nerror!(ErrorKind::FailedBorrow, "Failed to borrow bitmaps collection: {:?}", e)
        })
    }

    pub fn io_id_present(&self, io_id: i32) -> bool {
        self.io_id_list.borrow().contains(&io_id)
    }

    fn add_io(&self, io: IoProxy, io_id: i32, direction: IoDirection) -> Result<()> {
        self.io_id_list.borrow_mut().push(io_id);

        let codec_value = CodecInstanceContainer::create(self, io, io_id, direction)
            .map_err(|e| e.at(here!()))?;
        let mut codec = self.codecs.add_mut(codec_value);
        if let Ok(d) = codec.get_decoder() {
            d.initialize(self).map_err(|e| e.at(here!()))?;
        }
        Ok(())
    }

    /// Move the output buffer out as an owned `Vec<u8>`, avoiding any copy.
    /// After this call, the buffer is consumed — further access will error.
    pub fn take_output_buffer(&mut self, io_id: i32) -> Result<Vec<u8>> {
        let mut codec = self.get_codec(io_id).map_err(|e| e.at(here!()))?;
        codec.take_output_buffer().map_err(|e| e.at(here!()))
    }

    /// Return raw pointer + length to the output buffer for C ABI use.
    /// The buffer transitions to `Lent` state — kept alive, but `take` is blocked.
    ///
    /// The returned pointer is valid as long as this `Context` is alive and
    /// `take_output_buffer` is not called for this `io_id`.
    /// Dereferencing the pointer is the caller's responsibility (unsafe at use site).
    pub fn get_output_buffer_ptr(&mut self, io_id: i32) -> Result<(*const u8, usize)> {
        let mut codec = self.get_codec(io_id).map_err(|e| e.at(here!()))?;
        codec.output_buffer_raw_parts().map_err(|e| e.at(here!()))
    }

    /// Retrieve a BitmapKey captured by a CaptureBitmapKey node during execution.
    pub fn get_captured_bitmap_key(&self, capture_id: i32) -> Option<BitmapKey> {
        self.extras.as_ref()?.captured_bitmap_keys.get(&capture_id).copied()
    }

    /// Insert a captured BitmapKey (called by CaptureBitmapKey node during execution).
    pub(crate) fn insert_captured_bitmap_key(&mut self, capture_id: i32, key: BitmapKey) {
        self.extras
            .get_or_insert_with(|| Box::new(ContextExtras::default()))
            .captured_bitmap_keys
            .insert(capture_id, key);
    }

    /// Enable or disable deferred input registration (for modern Zen/media sources
    /// such as MP4/WebM/IVF that lack a legacy v1 magic-byte prober).
    pub fn set_input_registration_deferred(&mut self, deferred: bool) {
        self.extras
            .get_or_insert_with(|| Box::new(ContextExtras::default()))
            .modern_config
            .input_registration_deferred = deferred;
    }

    /// Whether deferred input registration is active on this [`Context`].
    pub fn is_input_registration_deferred(&self) -> bool {
        self.extras
            .as_ref()
            .is_some_and(|e| e.modern_config.input_registration_deferred)
    }

    /// Snapshot modern policy configuration on this [`Context`].
    pub fn modern_config(&self) -> ModernContextConfig {
        self.extras
            .as_ref()
            .map(|e| e.modern_config.clone())
            .unwrap_or_default()
    }

    /// Mutably access modern policy configuration on this [`Context`].
    pub fn modern_config_mut(&mut self) -> &mut ModernContextConfig {
        &mut self
            .extras
            .get_or_insert_with(|| Box::new(ContextExtras::default()))
            .modern_config
    }

    /// Clone this context's live cancellation token (`enough::Stop`).
    #[cfg(feature = "zen-media")]
    pub(crate) fn cancellation_stop(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    /// Borrow a replayable input lease registered under `io_id`, if present.
    pub(crate) fn modern_input(&self, io_id: i32) -> Option<&ModernInputLease> {
        self.extras.as_ref()?.modern_inputs.get(&io_id)
    }

    /// Validate that an output slot is ready for atomic modern publication.
    pub(crate) fn can_commit_modern_output(&self, io_id: i32) -> Result<()> {
        let codec = self.get_codec(io_id).map_err(|e| e.at(here!()))?;
        codec.can_commit_modern_output().map_err(|e| e.at(here!()))
    }

    /// Atomically commit finalized modern output bytes into a registered output slot.
    pub(crate) fn commit_modern_output(&mut self, io_id: i32, data: Vec<u8>) -> Result<()> {
        let mut codec = self.get_codec(io_id).map_err(|e| e.at(here!()))?;
        codec.commit_modern_output(data).map_err(|e| e.at(here!()))
    }

    /// Register a seekable `ReadAt` input source under `io_id` on a deferred `Context`.
    #[cfg(feature = "streaming")]
    pub fn add_input_read_at(
        &mut self,
        io_id: i32,
        source: Arc<dyn crate::media::ReadAtSource>,
    ) -> Result<()> {
        if !self.is_input_registration_deferred() {
            return Err(nerror!(
                ErrorKind::InvalidState,
                "add_input_read_at requires deferred input registration (call v2/configure_context first)"
            ));
        }
        if self.io_id_present(io_id) {
            return Err(nerror!(
                ErrorKind::DuplicateIoId,
                "io_id {} is already in use on this context",
                io_id
            ));
        }
        self.io_id_list.borrow_mut().push(io_id);
        self.extras
            .get_or_insert_with(|| Box::new(ContextExtras::default()))
            .modern_inputs
            .insert(io_id, ModernInputLease::ReadAt(source));
        Ok(())
    }

    fn record_modern_input(&mut self, io_id: i32, lease: ModernInputLease) {
        self.extras
            .get_or_insert_with(|| Box::new(ContextExtras::default()))
            .modern_inputs
            .insert(io_id, lease);
    }

    pub fn add_file(&mut self, io_id: i32, direction: IoDirection, path: &str) -> Result<()> {
        if direction == IoDirection::In && self.is_input_registration_deferred() {
            if self.io_id_present(io_id) {
                return Err(nerror!(
                    ErrorKind::DuplicateIoId,
                    "io_id {} is already in use on this context",
                    io_id
                ));
            }
            #[cfg(feature = "streaming")]
            {
                let file = std::fs::File::open(path).map_err(|e| {
                    nerror!(ErrorKind::DecodingIoError, "Failed to open input file: {}", e)
                })?;
                let src = Arc::new(crate::media::FileReadAtSource::new(file));
                return self.add_input_read_at(io_id, src);
            }
            #[cfg(not(feature = "streaming"))]
            {
                let bytes = std::fs::read(path).map_err(|e| {
                    nerror!(ErrorKind::DecodingIoError, "Failed to read input file: {}", e)
                })?;
                self.io_id_list.borrow_mut().push(io_id);
                self.record_modern_input(io_id, ModernInputLease::Shared(Arc::new(bytes)));
                return Ok(());
            }
        }
        let io =
            IoProxy::file_with_mode(self, io_id, path, direction).map_err(|e| e.at(here!()))?;
        self.add_io(io, io_id, direction).map_err(|e| e.at(here!()))
    }

    pub fn add_copied_input_buffer(&mut self, io_id: i32, bytes: &[u8]) -> Result<()> {
        if self.is_input_registration_deferred() {
            if self.io_id_present(io_id) {
                return Err(nerror!(
                    ErrorKind::DuplicateIoId,
                    "io_id {} is already in use on this context",
                    io_id
                ));
            }
            let shared = Arc::new(bytes.to_vec());
            self.io_id_list.borrow_mut().push(io_id);
            self.record_modern_input(io_id, ModernInputLease::Shared(shared));
            return Ok(());
        }
        #[cfg(feature = "zen-media")]
        {
            let shared = Arc::new(bytes.to_vec());
            let io = IoProxy::read_shared_vec(self, io_id, Arc::clone(&shared))
                .map_err(|e| e.at(here!()))?;
            self.add_io(io, io_id, IoDirection::In).map_err(|e| e.at(here!()))?;
            self.record_modern_input(io_id, ModernInputLease::Shared(shared));
            Ok(())
        }
        #[cfg(not(feature = "zen-media"))]
        {
            let io = IoProxy::read_vec(self, io_id, bytes.to_vec()).map_err(|e| e.at(here!()))?;
            self.add_io(io, io_id, IoDirection::In).map_err(|e| e.at(here!()))
        }
    }
    pub fn add_input_vector(&mut self, io_id: i32, bytes: Vec<u8>) -> Result<()> {
        if self.is_input_registration_deferred() {
            if self.io_id_present(io_id) {
                return Err(nerror!(
                    ErrorKind::DuplicateIoId,
                    "io_id {} is already in use on this context",
                    io_id
                ));
            }
            let shared = Arc::new(bytes);
            self.io_id_list.borrow_mut().push(io_id);
            self.record_modern_input(io_id, ModernInputLease::Shared(shared));
            return Ok(());
        }
        #[cfg(feature = "zen-media")]
        {
            let shared = Arc::new(bytes);
            let io = IoProxy::read_shared_vec(self, io_id, Arc::clone(&shared))
                .map_err(|e| e.at(here!()))?;
            self.add_io(io, io_id, IoDirection::In).map_err(|e| e.at(here!()))?;
            self.record_modern_input(io_id, ModernInputLease::Shared(shared));
            Ok(())
        }
        #[cfg(not(feature = "zen-media"))]
        {
            let io = IoProxy::read_vec(self, io_id, bytes).map_err(|e| e.at(here!()))?;
            self.add_io(io, io_id, IoDirection::In).map_err(|e| e.at(here!()))
        }
    }

    /// Zero-copy: borrows `bytes` without copying.
    /// The `'static` lifetime means callers must guarantee the data outlives the Context.
    pub fn add_input_bytes(&mut self, io_id: i32, bytes: &'static [u8]) -> Result<()> {
        self.add_input_buffer(io_id, bytes)
    }

    /// Zero-copy: borrows `bytes` without copying.
    /// The `'static` lifetime means callers must guarantee the data outlives the Context.
    /// In practice, the ABI layer (imageflow_abi) uses transmute to erase the real lifetime.
    pub fn add_input_buffer(&mut self, io_id: i32, bytes: &'static [u8]) -> Result<()> {
        if self.is_input_registration_deferred() {
            if self.io_id_present(io_id) {
                return Err(nerror!(
                    ErrorKind::DuplicateIoId,
                    "io_id {} is already in use on this context",
                    io_id
                ));
            }
            self.io_id_list.borrow_mut().push(io_id);
            self.record_modern_input(io_id, ModernInputLease::Borrowed(bytes));
            return Ok(());
        }
        let io = IoProxy::read_slice(self, io_id, bytes).map_err(|e| e.at(here!()))?;

        self.add_io(io, io_id, IoDirection::In).map_err(|e| e.at(here!()))?;
        #[cfg(feature = "zen-media")]
        self.record_modern_input(io_id, ModernInputLease::Borrowed(bytes));
        Ok(())
    }

    pub fn add_output_buffer(&mut self, io_id: i32) -> Result<()> {
        let io = IoProxy::create_output_buffer(self, io_id).map_err(|e| e.at(here!()))?;

        self.add_io(io, io_id, IoDirection::Out).map_err(|e| e.at(here!()))
    }

    fn swap_dimensions_by_exif(&mut self, io_id: i32, image_info: &mut ImageInfo) -> Result<()> {
        let exif_maybe = self
            .get_codec(io_id)
            .map_err(|e| e.at(here!()))?
            .get_decoder()
            .map_err(|e| e.at(here!()))?
            .get_exif_rotation_flag(self)
            .map_err(|e| e.at(here!()))?;

        if let Some(exif_flag) = exif_maybe
            && (5..=8).contains(&exif_flag)
        {
            std::mem::swap(&mut image_info.image_width, &mut image_info.image_height);
        }
        Ok(())
    }

    pub fn get_unscaled_unrotated_image_info(&self, io_id: i32) -> Result<s::ImageInfo> {
        self.get_codec(io_id)
            .map_err(|e| e.at(here!()))?
            .get_decoder()
            .map_err(|e| e.at(here!()))?
            .get_unscaled_image_info(self)
            .map_err(|e| e.at(here!()))
    }

    pub fn get_unscaled_rotated_image_info(&mut self, io_id: i32) -> Result<s::ImageInfo> {
        let mut image_info =
            self.get_unscaled_unrotated_image_info(io_id).map_err(|e| e.at(here!()))?;

        self.swap_dimensions_by_exif(io_id, &mut image_info)?;
        Ok(image_info)
    }

    pub fn get_image_decodes(&mut self) -> Vec<s::DecodeResult> {
        let io_ids = self.io_id_list.borrow().to_vec();

        io_ids
            .iter()
            .filter_map(|io_id| {
                if let Ok(info) = self.get_unscaled_rotated_image_info(*io_id) {
                    Some(imageflow_types::DecodeResult {
                        io_id: *io_id,
                        preferred_extension: info.preferred_extension,
                        preferred_mime_type: info.preferred_mime_type,
                        w: info.image_width,
                        h: info.image_height,
                    })
                } else {
                    None
                }
            })
            .sorted_by_key(|r| r.io_id)
            .collect_vec()
    }

    pub fn get_scaled_unrotated_image_info(&mut self, io_id: i32) -> Result<s::ImageInfo> {
        self.get_codec(io_id)
            .map_err(|e| e.at(here!()))?
            .get_decoder()
            .map_err(|e| e.at(here!()))?
            .get_scaled_image_info(self)
            .map_err(|e| e.at(here!()))
    }

    pub fn get_scaled_rotated_image_info(&mut self, io_id: i32) -> Result<s::ImageInfo> {
        let mut image_info =
            self.get_scaled_unrotated_image_info(io_id).map_err(|e| e.at(here!()))?;

        self.swap_dimensions_by_exif(io_id, &mut image_info)?;
        Ok(image_info)
    }

    pub fn tell_decoder(&mut self, io_id: i32, tell: s::DecoderCommand) -> Result<()> {
        self.get_codec(io_id)
            .map_err(|e| e.at(here!()))?
            .get_decoder()
            .map_err(|e| e.at(here!()))?
            .tell_decoder(self, tell)
            .map_err(|e| e.at(here!()))
    }

    pub fn get_exif_rotation_flag(&mut self, io_id: i32) -> Result<Option<i32>> {
        self.get_codec(io_id)
            .map_err(|e| e.at(here!()))?
            .get_decoder()
            .map_err(|e| e.at(here!()))?
            .get_exif_rotation_flag(self)
            .map_err(|e| e.at(here!()))
    }

    pub(crate) fn get_codec(&self, io_id: i32) -> Result<RefMut<'_, CodecInstanceContainer>> {
        let mut borrow_errors = 0;
        for item_result in self.codecs.iter_mut() {
            if let Ok(container) = item_result {
                if container.io_id == io_id {
                    return Ok(container);
                }
            } else {
                borrow_errors += 1;
            }
        }
        if borrow_errors > 0 {
            Err(nerror!(ErrorKind::FailedBorrow, "Could not locate codec by io_id {}; some codecs were exclusively borrowed by another scope.", io_id))
        } else {
            Err(nerror!(
                ErrorKind::IoIdNotFound,
                "No codec with io_id {}; all codecs searched.",
                io_id
            ))
        }
    }

    pub fn build_1(&mut self, parsed: s::Build001) -> Result<s::ResponsePayload> {
        let job_result = self.build_inner(parsed).map_err(|e| e.at(here!()))?;
        Ok(s::ResponsePayload::BuildResult(job_result))
    }

    /// For executing a complete job
    pub(crate) fn build_inner(&mut self, parsed: s::Build001) -> Result<s::JobResult> {
        let g = crate::parsing::GraphTranslator::new()
            .translate_framewise(parsed.framewise)
            .map_err(|e| e.at(here!()))?;

        if let Some(s::Build001Config { graph_recording, security, .. }) = parsed.builder_config {
            if let Some(r) = graph_recording {
                self.configure_graph_recording(r);
            }
            if let Some(s) = security {
                self.configure_security(s);
            }
        }

        crate::parsing::IoTranslator {}.add_all(self, parsed.io.clone())?;

        let decodes = self.get_image_decodes();

        let mut engine = crate::flow::execution_engine::Engine::create(self, g);

        let perf = engine.execute_many().map_err(|e| e.at(here!()))?;

        Ok(s::JobResult {
            decodes,
            encodes: engine.collect_augmented_encode_results(&parsed.io),
            performance: Some(perf),
        })
    }

    pub fn configure_graph_recording(&mut self, recording: s::Build001GraphRecording) {
        let r = if std::env::var("CI").map(|s| s.to_uppercase()) == Ok("TRUE".to_owned()) {
            s::Build001GraphRecording::off()
        } else {
            recording
        };
        self.graph_recording = r;
    }

    pub fn configure_security(&mut self, s: s::ExecutionSecurity) {
        if let Some(decode) = s.max_decode_size {
            self.security.max_decode_size = Some(decode);
        }
        if let Some(frame) = s.max_frame_size {
            self.security.max_frame_size = Some(frame);
        }
        if let Some(encode) = s.max_encode_size {
            self.security.max_encode_size = Some(encode);
        }
        if s.max_input_file_bytes.is_some() {
            self.security.max_input_file_bytes = s.max_input_file_bytes;
        }
        if s.max_json_bytes.is_some() {
            self.security.max_json_bytes = s.max_json_bytes;
        }
        if s.max_total_file_pixels.is_some() {
            self.security.max_total_file_pixels = s.max_total_file_pixels;
        }
        if s.max_threads.is_some() {
            self.security.max_threads = s.max_threads;
        }
        if s.mem_budget_policy.is_some() {
            self.security.mem_budget_policy = s.mem_budget_policy;
        }
    }

    /// For executing an operation graph (assumes you have already configured the context with IO sources/destinations as needed)
    pub fn execute_1(&mut self, what: s::Execute001) -> Result<s::ResponsePayload> {
        let job_result = self.execute_inner(what).map_err(|e| e.at(here!()))?;
        Ok(s::ResponsePayload::JobResult(job_result))
    }
    pub(crate) fn execute_inner(&mut self, what: s::Execute001) -> Result<s::JobResult> {
        let g = crate::parsing::GraphTranslator::new()
            .translate_framewise(what.framewise)
            .map_err(|e| e.at(here!()))?;
        if let Some(r) = what.graph_recording {
            self.configure_graph_recording(r);
        }
        if let Some(s) = what.security {
            self.configure_security(s);
        }

        let decodes = self.get_image_decodes();

        let mut engine = crate::flow::execution_engine::Engine::create(self, g);

        let perf = engine.execute_many().map_err(|e| e.at(here!()))?;

        Ok(s::JobResult {
            decodes,
            encodes: engine.collect_encode_results(),
            performance: Some(perf),
        })
    }

    pub fn get_version_info(&self) -> Result<s::VersionInfo> {
        Context::get_version_info_static()
    }
    pub(crate) fn get_version_info_static() -> Result<s::VersionInfo> {
        Ok(s::VersionInfo {
            long_version_string: imageflow_types::version::one_line_version().to_string(),
            last_git_commit: imageflow_types::version::last_commit().to_string(),
            dirty_working_tree: imageflow_types::version::dirty(),
            build_date: imageflow_types::version::get_build_date().to_string(),
            git_tag: imageflow_types::version::get_build_env_value("GIT_OPTIONAL_TAG")
                .to_owned()
                .map(|s| s.to_string()),
            git_describe_always: imageflow_types::version::get_build_env_value(
                "GIT_DESCRIBE_ALWAYS",
            )
            .or(Some(""))
            .unwrap()
            .to_owned(),
        })
    }

    /// Calculates the total size and count of all stack andheap allocations in a new Context
    /// Returns (total_bytes, num_allocations)
    ///
    /// This includes:
    /// - Initial capacity allocations for collections (codecs, io_id_list, bitmaps, allocations)
    /// - Arc allocation for shared state (cancellation_token)
    ///
    /// Note: RefCell stores its contents inline, not on the heap
    ///
    /// `enabled_codecs` deliberately contributes nothing: both of its lists are
    /// `SmallVec`s whose inline capacity holds every codec the build registers,
    /// so no allocation happens. That is not free by accident — the decoder list
    /// used to spill on every context, an allocation this function never
    /// modelled. `codecs::enabled_codecs_size_tests::enabled_codecs_fit_inline_unless_zen_codecs`
    /// keeps it inline.
    pub(crate) fn calculate_heap_allocations() -> (usize, usize) {
        let mut total_bytes = 0;
        let mut num_allocations = 0;

        total_bytes += std::mem::size_of::<Context>();
        // AddRemoveSet<CodecInstanceContainer> with capacity 4
        // This is typically backed by a Vec, so 1 allocation for the buffer
        if std::mem::size_of::<CodecInstanceContainer>() * Self::default_codecs_capacity() > 0 {
            total_bytes +=
                std::mem::size_of::<CodecInstanceContainer>() * Self::default_codecs_capacity();
            num_allocations += 1;
        }

        // Vec<i32> with capacity 2 (inside RefCell, but RefCell is inline)
        if std::mem::size_of::<i32>() * Self::default_codecs_capacity() > 0 {
            total_bytes += std::mem::size_of::<i32>() * Self::default_codecs_capacity();
            num_allocations += 1;
        }

        // DenseSlotMap in BitmapsContainer with capacity 16
        // DenseSlotMap typically uses 2 Vec allocations (one for slots, one for keys)
        let slot_size = std::mem::size_of::<RefCell<crate::graphics::bitmaps::Bitmap>>();
        let key_size = std::mem::size_of::<crate::graphics::bitmaps::BitmapKey>();
        if slot_size * crate::graphics::bitmaps::BitmapsContainer::default_capacity() > 0 {
            total_bytes +=
                slot_size * crate::graphics::bitmaps::BitmapsContainer::default_capacity();
            num_allocations += 1;
        }
        if key_size * crate::graphics::bitmaps::BitmapsContainer::default_capacity() > 0 {
            total_bytes +=
                key_size * crate::graphics::bitmaps::BitmapsContainer::default_capacity();
            num_allocations += 1;
        }

        // Arc<AtomicBool> for cancellation_token - 1 heap allocation
        total_bytes += std::mem::size_of::<AtomicBool>();
        num_allocations += 1;

        (total_bytes, num_allocations)
    }
}

#[test]
fn test_take_output_buffer_wrong_type_error() {
    // 1x1 RGBA PNG
    let png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    let mut context = Context::create().unwrap();
    context.add_input_vector(0, png_bytes).unwrap();

    assert_eq!(ErrorKind::InvalidArgument, context.take_output_buffer(0).err().unwrap().kind);
}

#[test]
fn test_get_ptr_on_decoder_returns_invalid_operation() {
    let png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    let mut context = Context::create().unwrap();
    context.add_input_vector(0, png_bytes).unwrap();

    let err = context.get_output_buffer_ptr(0).err().unwrap();
    assert_eq!(ErrorKind::InvalidArgument, err.kind);
}

#[test]
fn test_take_output_before_encode_returns_empty_vec() {
    let mut context = Context::create().unwrap();
    context.add_output_buffer(0).unwrap();

    let bytes = context.take_output_buffer(0).unwrap();
    assert!(bytes.is_empty());
}

#[test]
fn test_take_twice_returns_invalid_state() {
    let mut context = Context::create().unwrap();
    context.add_output_buffer(0).unwrap();

    let _ = context.take_output_buffer(0).unwrap();
    let err = context.take_output_buffer(0).err().unwrap();
    assert_eq!(ErrorKind::InvalidArgument, err.kind);
    assert!(err.message.contains("already been taken"));
}

#[test]
fn test_get_ptr_then_take_returns_invalid_state() {
    let mut context = Context::create().unwrap();
    context.add_output_buffer(0).unwrap();

    let (ptr, len) = context.get_output_buffer_ptr(0).unwrap();
    assert!(!ptr.is_null());

    let err = context.take_output_buffer(0).err().unwrap();
    assert_eq!(ErrorKind::InvalidArgument, err.kind);
    assert!(err.message.contains("raw pointer"));
}

#[test]
fn test_get_ptr_idempotent() {
    let mut context = Context::create().unwrap();
    context.add_output_buffer(0).unwrap();

    let (ptr1, len1) = context.get_output_buffer_ptr(0).unwrap();
    let (ptr2, len2) = context.get_output_buffer_ptr(0).unwrap();
    assert_eq!(ptr1, ptr2);
    assert_eq!(len1, len2);
}

#[test]
fn test_get_ptr_after_take_returns_invalid_state() {
    let mut context = Context::create().unwrap();
    context.add_output_buffer(0).unwrap();

    let _ = context.take_output_buffer(0).unwrap();
    let err = context.get_output_buffer_ptr(0).err().unwrap();
    assert_eq!(ErrorKind::InvalidArgument, err.kind);
    assert!(err.message.contains("already been taken"));
}

#[test]
fn test_take_on_nonexistent_io_id() {
    let mut context = Context::create().unwrap();
    let err = context.take_output_buffer(42).err().unwrap();
    assert_eq!(ErrorKind::IoIdNotFound, err.kind);
}

#[test]
fn test_take_after_encode_returns_data() {
    use imageflow_types as s;
    let png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, png_bytes).unwrap();
    ctx.add_output_buffer(1).unwrap();

    let execute = s::Execute001 {
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: None,
        job_options: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::Libpng {
                    depth: None,
                    matte: None,
                    zlib_compression: None,
                },
            },
        ]),
    };
    ctx.execute_1(execute).unwrap();

    let bytes = ctx.take_output_buffer(1).unwrap();
    assert!(!bytes.is_empty());
    assert!(bytes.starts_with(b"\x89PNG"));
}

#[test]
fn test_get_ptr_after_encode_then_take_blocked() {
    use imageflow_types as s;
    let png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, png_bytes).unwrap();
    ctx.add_output_buffer(1).unwrap();

    let execute = s::Execute001 {
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: None,
        job_options: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::Libpng {
                    depth: None,
                    matte: None,
                    zlib_compression: None,
                },
            },
        ]),
    };
    ctx.execute_1(execute).unwrap();

    // Lend the pointer (C ABI path)
    let (ptr, len) = ctx.get_output_buffer_ptr(1).unwrap();
    assert!(!ptr.is_null());
    assert!(len > 0);

    // take should be blocked
    let err = ctx.take_output_buffer(1).err().unwrap();
    assert_eq!(ErrorKind::InvalidArgument, err.kind);
    assert!(err.message.contains("raw pointer"));

    // get_ptr again should be idempotent
    let (ptr2, len2) = ctx.get_output_buffer_ptr(1).unwrap();
    assert_eq!(ptr, ptr2);
    assert_eq!(len, len2);
}

impl Drop for Context {
    /// Used by abi; should not panic
    fn drop(&mut self) {
        if let Err(e) = self.codecs.clear() {
            //TODO: log issue somewhere?
            eprintln!("Error clearing codecs in Context::drop: {:?}", e);
        }
        self.codecs.mut_clear(); // Dangerous, because there's no prohibition on dangling references.
    }
}

#[test]
fn test_context_size() {
    eprintln!("std::mem::sizeof(Context) = {}", std::mem::size_of::<Context>());
    assert!(std::mem::size_of::<Context>() < 400);
}

#[test]
fn test_thread_safe_context_size() {
    println!("std::mem::sizeof(ThreadSafeContext) = {}", std::mem::size_of::<ThreadSafeContext>());
    eprintln!("std::mem::sizeof(ThreadSafeContext) = {}", std::mem::size_of::<ThreadSafeContext>());
    assert!(std::mem::size_of::<ThreadSafeContext>() <= 560);
}

#[test]
fn test_calculate_context_heap_size() {
    let (context_bytes, context_allocs) = Context::calculate_heap_allocations();
    let (thread_safe_bytes, thread_safe_allocs) = ThreadSafeContext::calculate_heap_allocations();

    eprintln!(
        "Context::calculate_heap_allocations() = ({} bytes, {} allocations)",
        context_bytes, context_allocs
    );
    eprintln!(
        "ThreadSafeContext::calculate_heap_allocations() = ({} bytes, {} allocations)",
        thread_safe_bytes, thread_safe_allocs
    );

    // ThreadSafeContext and Context share the same heap allocations (Context is inside RwLock)
    assert!(thread_safe_bytes > context_bytes);
    assert!(thread_safe_allocs >= context_allocs);

    // Sanity check: should have some allocations
    assert!(context_allocs > 0);
    assert!(context_bytes > 0);

    // Fail if this grows so we can notice it
    // Windows has larger RwLock/Mutex, so allow a few extra bytes
    assert!(context_allocs <= 6);
    assert!(context_bytes <= 1056);

    assert!(context_allocs <= 6);
    assert!(thread_safe_bytes <= 1248);
}

#[test]
fn test_cancellation_token_implements_stop() {
    let token = CancellationToken::new();

    // Before cancellation: check() returns Ok, should_stop() returns false
    assert_eq!(token.check(), Ok(()));
    assert!(!token.should_stop());

    // After cancellation: check() returns Err(Cancelled), should_stop() returns true
    token.cancel_internal();
    assert_eq!(token.check(), Err(StopReason::Cancelled));
    assert!(token.should_stop());
}

#[cfg(debug_assertions)]
#[test]
fn test_cancellation_token_stop_with_poll_countdown() {
    let token = CancellationToken::new();

    // fetch_sub returns the *previous* value, so countdown=2 means:
    // poll 1: prev=2, now=1, 2 >= 1 → Ok
    // poll 2: prev=1, now=0, 1 >= 1 → Ok
    // poll 3: prev=0, now=-1, 0 < 1 → Cancelled
    token.request_cancellation_after_n_polls(2);

    assert_eq!(token.check(), Ok(()));
    assert_eq!(token.check(), Ok(()));
    assert_eq!(token.check(), Err(StopReason::Cancelled));
    assert!(token.should_stop());
}

#[test]
fn test_configure_security_populates_all_fields_from_json_execute() {
    let png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    let execute_json = r#"{
        "security": {
            "max_decode_size": { "w": 4096, "h": 4096, "megapixels": 16.0 },
            "max_frame_size": { "w": 2048, "h": 2048, "megapixels": 4.0 },
            "max_encode_size": { "w": 1024, "h": 1024, "megapixels": 1.0 },
            "max_input_file_bytes": 1048576,
            "max_json_bytes": 524288,
            "max_total_file_pixels": 50000000,
            "max_threads": 2,
            "mem_budget_policy": {
                "require_est_bytes_below": 33554432,
                "require_est_max_bytes_below": 67108864,
                "require_tracked_bytes_below": 134217728
            }
        },
        "framewise": {
            "steps": [
                { "decode": { "io_id": 0 } },
                { "encode": { "io_id": 1, "preset": { "lodepng": {} } } }
            ]
        }
    }"#;

    let parsed: s::Execute001 = serde_json::from_str(execute_json).unwrap();
    let mut ctx = Context::create().unwrap();
    ctx.add_input_vector(0, png_bytes).unwrap();
    ctx.add_output_buffer(1).unwrap();
    ctx.execute_1(parsed).unwrap();

    assert_eq!(
        ctx.security.max_decode_size,
        Some(s::FrameSizeLimit { w: 4096, h: 4096, megapixels: 16.0 })
    );
    assert_eq!(
        ctx.security.max_frame_size,
        Some(s::FrameSizeLimit { w: 2048, h: 2048, megapixels: 4.0 })
    );
    assert_eq!(
        ctx.security.max_encode_size,
        Some(s::FrameSizeLimit { w: 1024, h: 1024, megapixels: 1.0 })
    );
    assert_eq!(ctx.security.max_input_file_bytes, Some(1_048_576));
    assert_eq!(ctx.security.max_json_bytes, Some(524_288));
    assert_eq!(ctx.security.max_total_file_pixels, Some(50_000_000));
    assert_eq!(ctx.security.max_threads, Some(2));

    let policy = ctx
        .security
        .mem_budget_policy
        .as_ref()
        .expect("mem_budget_policy must be populated by configure_security");
    assert_eq!(policy.require_est_bytes_below, Some(33_554_432));
    assert_eq!(policy.require_est_max_bytes_below, Some(67_108_864));
    assert_eq!(policy.require_tracked_bytes_below, Some(134_217_728));
}
