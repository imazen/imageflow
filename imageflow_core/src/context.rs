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
/// 1025 bytes including 5 heap allocations as of Oct 2025. If on the stack, 312 bytes are taken up
pub struct Context {
    pub debug_job_id: i32,
    pub next_stable_node_id: i32,
    pub next_graph_version: i32,
    pub max_calc_flatten_execute_passes: i32,
    pub graph_recording: s::Build001GraphRecording,
    cancellation_token: CancellationToken,

    /// Codecs, which in turn connect to I/O instances.
    pub codecs: AddRemoveSet<CodecInstanceContainer>, // This loans out exclusive mutable references to items, bounding the ownership lifetime to Context
    /// A list of io_ids already in use
    pub io_id_list: RefCell<Vec<i32>>,

    pub enabled_codecs: EnabledCodecs,

    /// Context-scoped defaults applied when no inline job security block
    /// narrows them further. Initialized from `sane_defaults()` (or from a
    /// trusted policy, if one is later installed). **Never mutated by
    /// job-level `security` JSON** — job-level intent lives in
    /// `active_job_security` for the lifetime of the job.
    ///
    /// `max_json_bytes` is the one field read before the job's own
    /// `security` is parsed (it bounds the parse itself), so it stays
    /// Context-scoped.
    pub default_job_security: imageflow_types::ExecutionSecurity,

    /// Per-job effective security, set for the duration of a single
    /// `build`/`execute` call and cleared when the job finishes.
    ///
    /// Computed by `effective_security()` = default_job_security ∩
    /// inline-job-security. Read by the decode/encode dispatch paths and
    /// by the per-node limit checks so job intent never leaks across jobs
    /// on the same Context.
    ///
    /// Boxed to keep the idle-Context footprint small — only a pointer
    /// is inline; the `ExecutionSecurity` is allocated on first job.
    pub active_job_security: Option<Box<imageflow_types::ExecutionSecurity>>,

    pub bitmaps: RefCell<crate::graphics::bitmaps::BitmapsContainer>,

    pub allocations: RefCell<AllocationContainer>,

    /// Bitmap keys captured by CaptureBitmapKey nodes during graph execution.
    captured_bitmap_keys: Option<Box<std::collections::HashMap<i32, BitmapKey>>>,
}

// This token is the shared state.
#[derive(Default)]
struct CancellationToken {
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

    pub fn request_cancellation(&mut self) {
        self.cancellation_token.cancel_internal();
        self.outward_error_mut()
            .try_set_error(nerror!(ErrorKind::OperationCancelled, "Cancellation was requested"));
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
    pub fn outward_error_mut(&mut self) -> RwLockWriteGuard<'_, OutwardErrorBuffer> {
        self.outward_error
            .write()
            .expect("OutwardErrorBuffer.write failed: lock poisoned from a panic")
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
            default_job_security: imageflow_types::ExecutionSecurity::sane_defaults(),
            active_job_security: None,
            allocations: RefCell::new(AllocationContainer::new()),

            captured_bitmap_keys: None,
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
            default_job_security: imageflow_types::ExecutionSecurity::sane_defaults(),
            active_job_security: None,
            allocations: RefCell::new(AllocationContainer::new()),

            captured_bitmap_keys: None,
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
            default_job_security: imageflow_types::ExecutionSecurity::sane_defaults(),
            active_job_security: None,
            allocations: RefCell::new(AllocationContainer::new()),

            captured_bitmap_keys: None,
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

    pub fn message(&mut self, method: &str, json: &[u8]) -> (JsonResponse, Result<()>) {
        crate::json::invoke_with_json_error(self, method, json)
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
        self.captured_bitmap_keys.as_ref()?.get(&capture_id).copied()
    }

    /// Insert a captured BitmapKey (called by CaptureBitmapKey node during execution).
    pub fn insert_captured_bitmap_key(&mut self, capture_id: i32, key: BitmapKey) {
        self.captured_bitmap_keys
            .get_or_insert_with(|| Box::new(std::collections::HashMap::new()))
            .insert(capture_id, key);
    }

    pub fn add_file(&mut self, io_id: i32, direction: IoDirection, path: &str) -> Result<()> {
        let io =
            IoProxy::file_with_mode(self, io_id, path, direction).map_err(|e| e.at(here!()))?;
        self.add_io(io, io_id, direction).map_err(|e| e.at(here!()))
    }

    pub fn add_copied_input_buffer(&mut self, io_id: i32, bytes: &[u8]) -> Result<()> {
        let io = IoProxy::copy_slice(self, io_id, bytes).map_err(|e| e.at(here!()))?;

        self.add_io(io, io_id, IoDirection::In).map_err(|e| e.at(here!()))
    }
    pub fn add_input_vector(&mut self, io_id: i32, bytes: Vec<u8>) -> Result<()> {
        let io = IoProxy::read_vec(self, io_id, bytes).map_err(|e| e.at(here!()))?;

        self.add_io(io, io_id, IoDirection::In).map_err(|e| e.at(here!()))
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
        let io = IoProxy::read_slice(self, io_id, bytes).map_err(|e| e.at(here!()))?;

        self.add_io(io, io_id, IoDirection::In).map_err(|e| e.at(here!()))
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

        if let Some(exif_flag) = exif_maybe {
            if (5..=8).contains(&exif_flag) {
                std::mem::swap(&mut image_info.image_width, &mut image_info.image_height);
            }
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
                        annotations: None,
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

    pub fn get_codec(&self, io_id: i32) -> Result<RefMut<'_, CodecInstanceContainer>> {
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

        // Split the inline security block out *without* mutating
        // `Context.default_job_security`. Validate shape + layer-3 rules
        // up front; compute the effective security; install it as
        // `active_job_security` for the job's lifetime only.
        let inline_security = match parsed.builder_config {
            Some(s::Build001Config { graph_recording, security, .. }) => {
                if let Some(r) = graph_recording {
                    self.configure_graph_recording(r);
                }
                security
            }
            None => None,
        };
        if let Some(s) = inline_security.as_ref() {
            Self::validate_inline_job_security(s).map_err(|e| e.at(here!()))?;
        }
        let effective = self.effective_security(inline_security.as_ref());
        let snapshot = JobSecuritySnapshot::install(self, effective);

        let result = (|| -> Result<s::JobResult> {
            crate::parsing::IoTranslator {}.add_all(self, parsed.io.clone())?;
            let decodes = self.get_image_decodes();
            let mut engine = crate::flow::execution_engine::Engine::create(self, g);
            let perf = engine.execute_many().map_err(|e| e.at(here!()))?;
            Ok(s::JobResult {
                decodes,
                encodes: engine.collect_augmented_encode_results(&parsed.io),
                performance: Some(perf),
            })
        })();

        snapshot.restore(self);
        result
    }

    pub fn configure_graph_recording(&mut self, recording: s::Build001GraphRecording) {
        let r = if std::env::var("CI").map(|s| s.to_uppercase()) == Ok("TRUE".to_owned()) {
            s::Build001GraphRecording::off()
        } else {
            recording
        };
        self.graph_recording = r;
    }

    /// Validate an inline job-level security block (layer 3).
    ///
    /// Pure function — does not mutate `self`. Used by `effective_security`
    /// and by job-entry paths to reject malformed or "may only deny"
    /// requests before the job runs.
    pub fn validate_inline_job_security(request: &s::ExecutionSecurity) -> Result<()> {
        if let Some(formats) = &request.formats {
            formats
                .validate()
                .map_err(|e| nerror!(ErrorKind::InvalidArgument, "invalid killbits: {}", e))?;
            formats
                .validate_job_level()
                .map_err(|e| nerror!(ErrorKind::InvalidArgument, "{}", e))?;
        }
        if let Some(codecs) = &request.codecs {
            codecs
                .validate()
                .map_err(|e| nerror!(ErrorKind::InvalidArgument, "invalid codec killbits: {}", e))?;
            codecs
                .validate_job_level()
                .map_err(|e| nerror!(ErrorKind::InvalidArgument, "{}", e))?;
        }
        Ok(())
    }

    /// Compute the effective security for a single job.
    ///
    /// Pure — does not mutate `self`. Result is
    /// `default_job_security ∩ inline`. Fields that were `None` on the
    /// narrower layer fall through to the wider layer.
    ///
    /// Callers that need the value for the duration of a job should also
    /// store it into `active_job_security` via [`JobSecuritySnapshot::install`]
    /// so decode/encode dispatch and per-node checks can read it through
    /// [`Self::current_security`].
    pub fn effective_security(
        &self,
        inline: Option<&s::ExecutionSecurity>,
    ) -> s::ExecutionSecurity {
        let mut effective = self.default_job_security.clone();
        if let Some(request) = inline {
            effective = intersect_job_security(&effective, request);
        }
        effective
    }

    /// Return the effective security currently in force for the job, or
    /// the Context default if no job is active. This is the single read
    /// point for per-node limit checks (`max_decode_size`,
    /// `max_frame_size`, `max_encode_size`, `max_total_file_pixels`,
    /// `max_input_file_bytes`).
    ///
    /// The returned reference is valid until the job exits (which drops
    /// `active_job_security`).
    pub fn current_security(&self) -> &s::ExecutionSecurity {
        self.active_job_security.as_deref().unwrap_or(&self.default_job_security)
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
        let inline_security = what.security;
        if let Some(s) = inline_security.as_ref() {
            Self::validate_inline_job_security(s).map_err(|e| e.at(here!()))?;
        }
        let effective = self.effective_security(inline_security.as_ref());
        let snapshot = JobSecuritySnapshot::install(self, effective);

        let result = (|| -> Result<s::JobResult> {
            let decodes = self.get_image_decodes();
            let mut engine = crate::flow::execution_engine::Engine::create(self, g);
            let perf = engine.execute_many().map_err(|e| e.at(here!()))?;
            Ok(s::JobResult {
                decodes,
                encodes: engine.collect_encode_results(),
                performance: Some(perf),
            })
        })();

        snapshot.restore(self);
        result
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

/// Intersect two job-level security blocks: pick the more restrictive of
/// each scalar limit; combine the `formats` / `codecs` killbits lists.
///
/// Pure function, private to this module. Used by
/// [`Context::effective_security`]. Trusted-policy intersection is layered
/// on top of this by a later commit.
fn intersect_job_security(
    default: &s::ExecutionSecurity,
    job: &s::ExecutionSecurity,
) -> s::ExecutionSecurity {
    let max_decode_size = min_optional_frame(&default.max_decode_size, &job.max_decode_size);
    let max_frame_size = min_optional_frame(&default.max_frame_size, &job.max_frame_size);
    let max_encode_size = min_optional_frame(&default.max_encode_size, &job.max_encode_size);
    let max_input_file_bytes =
        min_optional(default.max_input_file_bytes, job.max_input_file_bytes);
    let max_json_bytes = min_optional(default.max_json_bytes, job.max_json_bytes);
    let max_total_file_pixels =
        min_optional(default.max_total_file_pixels, job.max_total_file_pixels);

    // Job-level may only `deny_*`; intersection of two lists is the union
    // of their denies.
    let formats = match (&default.formats, &job.formats) {
        (None, None) => None,
        (Some(t), None) => Some(t.clone()),
        (None, Some(j)) => Some(j.clone()),
        (Some(t), Some(j)) => Some(Box::new(imageflow_types::FormatKillbits::intersect(t, j))),
    };
    let codecs = match (&default.codecs, &job.codecs) {
        (None, None) => None,
        (Some(t), None) => Some(t.clone()),
        (None, Some(j)) => Some(j.clone()),
        (Some(t), Some(j)) => Some(Box::new(imageflow_types::CodecKillbits::intersect(t, j))),
    };

    let mut out = s::ExecutionSecurity::unspecified();
    out.max_decode_size = max_decode_size;
    out.max_frame_size = max_frame_size;
    out.max_encode_size = max_encode_size;
    out.max_input_file_bytes = max_input_file_bytes;
    out.max_json_bytes = max_json_bytes;
    out.max_total_file_pixels = max_total_file_pixels;
    out.formats = formats;
    out.codecs = codecs;
    out
}

fn min_optional<T: Ord + Copy>(a: Option<T>, b: Option<T>) -> Option<T> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (Some(x), None) => Some(x),
        (None, Some(y)) => Some(y),
        (None, None) => None,
    }
}

fn min_optional_frame(
    a: &Option<s::FrameSizeLimit>,
    b: &Option<s::FrameSizeLimit>,
) -> Option<s::FrameSizeLimit> {
    match (a, b) {
        (Some(x), Some(y)) => Some(s::FrameSizeLimit {
            w: x.w.min(y.w),
            h: x.h.min(y.h),
            megapixels: x.megapixels.min(y.megapixels),
        }),
        (Some(x), None) => Some(x.clone()),
        (None, Some(y)) => Some(y.clone()),
        (None, None) => None,
    }
}

/// Scope guard that installs an effective `ExecutionSecurity` on the
/// Context on construction and restores the prior value on drop.
///
/// Holds no reference back to the Context — the caller retains &mut
/// access for the rest of the job. Drop reads the stashed previous value
/// and writes it back through a raw-pointer-free helper that borrows
/// the Context again through a `Restore` wrapper. In practice we just
/// store the old value and let the caller restore it with
/// `finish(&mut ctx)`; this avoids RAII tying up the borrow.
pub(crate) struct JobSecuritySnapshot {
    previous: Option<Box<imageflow_types::ExecutionSecurity>>,
}

impl JobSecuritySnapshot {
    /// Install `effective` as `ctx.active_job_security` and return a
    /// snapshot of the prior value. Call [`Self::restore`] when the
    /// job exits.
    pub(crate) fn install(
        ctx: &mut Context,
        effective: imageflow_types::ExecutionSecurity,
    ) -> Self {
        let previous = ctx.active_job_security.take();
        ctx.active_job_security = Some(Box::new(effective));
        JobSecuritySnapshot { previous }
    }

    /// Restore the prior `active_job_security` that existed before
    /// [`Self::install`] was called. Must be called once per install;
    /// calling more than once leaves a stale prior value installed.
    pub(crate) fn restore(self, ctx: &mut Context) {
        ctx.active_job_security = self.previous;
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
    // Context holds:
    //   - trusted_policy:       Option<Box<ExecutionSecurity>> (layer 2)
    //   - default_job_security: ExecutionSecurity              (layer 2 scalar fold)
    //   - active_job_security:  Option<Box<ExecutionSecurity>> (per-job effective)
    // The two boxes keep the idle stack footprint small; the inline
    // `default_job_security` carries the Context's scalar limit baseline.
    assert!(std::mem::size_of::<Context>() < 424);
}

#[test]
fn test_thread_safe_context_size() {
    println!("std::mem::sizeof(ThreadSafeContext) = {}", std::mem::size_of::<ThreadSafeContext>());
    eprintln!("std::mem::sizeof(ThreadSafeContext) = {}", std::mem::size_of::<ThreadSafeContext>());
    assert!(std::mem::size_of::<ThreadSafeContext>() <= 592);
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
    // Windows has larger RwLock/Mutex, so allow a few extra bytes.
    // Codec-level killbits added `Option<Box<CodecKillbits>>` to the
    // embedded `ExecutionSecurity`s on Context; account for the extra
    // 16 bytes here.
    assert!(context_allocs <= 6);
    assert!(context_bytes <= 1088);

    assert!(context_allocs <= 6);
    assert!(thread_safe_bytes <= 1280);
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
