//! Resource limit and robustness tests
//!
//! These tests verify that oversized or malformed images are rejected gracefully.

use imageflow_core::{Context, ErrorKind};
use imageflow_types as s;
use std::fs;
use std::path::PathBuf;

/// Helper to create a context
fn create_context() -> Box<Context> {
    Context::create().expect("Failed to create context")
}

/// Returns the imageflow repo root directory.
fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is imageflow_core/
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

// =============================================================================
// GIF dimension limit tests
// =============================================================================

/// Reading the logical screen descriptor must not allocate the screen: a header
/// is parsed, not a canvas.
#[test]
fn test_gif_screen_allocation_order() {
    // GIF dimensions are u16, so the screen descriptor can claim up to 65535 in
    // each axis. A 100x100 file is the control case for the two limit tests below.
    let valid_gif = create_valid_gif(100, 100);

    let mut ctx = create_context();
    ctx.add_copied_input_buffer(0, &valid_gif).expect("valid 100x100 GIF should be accepted");

    let info = ctx.get_unscaled_unrotated_image_info(0).expect("should get info for a valid GIF");
    assert_eq!((100, 100), (info.image_width, info.image_height));
    assert_eq!("image/gif", info.preferred_mime_type);
}

/// `get_unscaled_unrotated_image_info` reports the logical screen size straight
/// from the header. 8000 x 8000 is the largest screen the `gif` crate will
/// allocate for, and reporting it must not be gated on that allocation
/// succeeding — nothing is decoded at this stage.
#[test]
fn test_gif_at_memory_limit() {
    // 8000 * 8000 = 64,000,000, the gif crate's memory limit exactly.
    let gif = create_valid_gif(8000, 8000);

    let mut ctx = create_context();
    ctx.add_copied_input_buffer(0, &gif).expect("8000x8000 GIF buffer should be accepted");

    let info = ctx.get_unscaled_unrotated_image_info(0).expect("8000x8000 GIF info should succeed");
    assert_eq!((8000, 8000), (info.image_width, info.image_height));
}

/// Characterization: a logical screen *over* the `gif` crate's 64,000,000-pixel
/// limit is still reported by `get_unscaled_unrotated_image_info`, because that
/// call only parses the header.
///
/// This pins current behavior rather than asserting a preference. The header is
/// attacker-controlled and costs nothing to claim, so what actually bounds
/// allocation is the 16 MP frame-buffer cap applied during decode (see
/// `test_gif_frame_buffer_size_cap_rejects_oversized_frame`). If info-stage
/// dimension validation is ever added, this test is the one that has to change,
/// and it will say so instead of silently passing either way.
#[test]
fn test_gif_over_memory_limit() {
    // 8001 * 8001 = 64,016,001, just past the gif crate's limit.
    let gif = create_valid_gif(8001, 8001);

    let mut ctx = create_context();
    ctx.add_copied_input_buffer(0, &gif).expect("8001x8001 GIF buffer should be accepted");

    let info = ctx
        .get_unscaled_unrotated_image_info(0)
        .expect("info is header-only, so an oversized screen is currently reported, not rejected");
    assert_eq!((8001, 8001), (info.image_width, info.image_height));
}

/// Create a valid GIF with specified dimensions
fn create_valid_gif(width: u16, height: u16) -> Vec<u8> {
    let mut gif = Vec::new();

    // GIF89a header
    gif.extend_from_slice(b"GIF89a");

    // Logical screen descriptor
    gif.extend_from_slice(&width.to_le_bytes());
    gif.extend_from_slice(&height.to_le_bytes());
    // Flags: global color table (1 bit), color resolution (3 bits), sort flag (1 bit), GCT size (3 bits)
    // 0x80 = has global color table, 2^(0+1) = 2 colors
    gif.push(0x80);
    gif.push(0x00); // Background color index
    gif.push(0x00); // Pixel aspect ratio

    // Global color table (2 entries = 6 bytes)
    gif.extend_from_slice(&[0x00, 0x00, 0x00]); // Color 0: Black
    gif.extend_from_slice(&[0xFF, 0xFF, 0xFF]); // Color 1: White

    // Image descriptor
    gif.push(0x2C); // Image separator
    gif.extend_from_slice(&[0x00, 0x00]); // Left position
    gif.extend_from_slice(&[0x00, 0x00]); // Top position
    gif.extend_from_slice(&width.to_le_bytes());
    gif.extend_from_slice(&height.to_le_bytes());
    gif.push(0x00); // No local color table, not interlaced

    // Image data
    gif.push(0x02); // LZW minimum code size = 2 (for 2 colors, need 2 bits)

    // LZW compressed data for solid color 0
    // Clear code = 4 (2^2), EOI = 5
    // For a solid color image, we just need: clear + lots of 0s + EOI
    // This is a minimal valid LZW stream that decodes to all zeros
    gif.push(0x02); // Sub-block size = 2
    gif.extend_from_slice(&[0x4C, 0x01]); // Clear code + data + EOI compressed
    gif.push(0x00); // Block terminator

    // GIF trailer
    gif.push(0x3B);

    gif
}

// =============================================================================
// Canvas dimension limit tests
// =============================================================================

fn create_canvas_job(w: usize, h: usize) -> s::Build001 {
    s::Build001 {
        builder_config: None,
        io: vec![],
        framewise: s::Framewise::Steps(vec![s::Node::CreateCanvas {
            w,
            h,
            format: s::PixelFormat::Bgra32,
            color: s::Color::Srgb(s::ColorSrgb::Hex("ffffff".to_owned())),
        }]),
    }
}

/// The 100 MP default `max_frame_size` is inclusive: exactly 10000 x 10000 has
/// to build, or the limit is off by one and every "over the limit" test below
/// would pass for the wrong reason.
#[test]
fn test_bitmap_canvas_at_limit() {
    let mut ctx = create_context();

    let job = create_canvas_job(10000, 10000);

    ctx.build_1(job).expect("10000x10000 (exactly 100MP) is at the limit and must be accepted");
}

#[test]
fn test_bitmap_canvas_over_limit() {
    let mut ctx = create_context();

    // 10001x10001 = 100,020,001 which is over the 100MP limit
    let job = create_canvas_job(10001, 10001);

    // Asserting the kind matters: any setup failure would also produce an Err,
    // and a bare `is_err()` could not tell the megapixel guard from, say, a
    // color-parse failure that never reached the guard at all.
    let message = expect_error_kind(ctx.build_1(job), ErrorKind::InvalidCoordinates);
    assert!(
        message.contains("cannot exceed 100 megapixels"),
        "expected the megapixel guard to reject this, got: {}",
        message
    );
}

#[test]
fn test_bitmap_canvas_i32_overflow() {
    let mut ctx = create_context();

    // 46341 * 46341 = 2,147,488,281, which overflows i32. It is caught by the
    // same 100MP guard long before any product is computed — asserting the
    // message keeps that visible, so if the guard is ever narrowed this test
    // reports an overflow reaching further in rather than quietly passing.
    let job = create_canvas_job(46341, 46341);

    let message = expect_error_kind(ctx.build_1(job), ErrorKind::InvalidCoordinates);
    assert!(
        message.contains("cannot exceed 100 megapixels"),
        "expected the megapixel guard to reject this, got: {}",
        message
    );
}

// =============================================================================
// ICC profile and EXIF handling tests
// =============================================================================

/// Base URL of the shared test corpus — the same bucket the visual tests pull
/// from, cached under `.image-cache/sources/`.
const CORPUS_BASE: &str = "https://s3-us-west-2.amazonaws.com/imageflow-resources/";

/// Fetches a corpus image by its path within the bucket.
///
/// The three tests below previously looked for fixtures under the repo root
/// (`examples/export_4_sizes/waterhouse.jpg`, `imageflow_core/tests/visuals/`).
/// Neither path has ever existed in this repo, so each test took its "not
/// found" branch, printed a line, and passed without decoding anything — on
/// every machine and in every CI run. Pulling from the corpus the rest of the
/// suite already uses means there is no missing-fixture branch to take.
fn corpus_bytes(relative: &str) -> Vec<u8> {
    crate::common::get_url_bytes_with_retry(&format!("{CORPUS_BASE}{relative}"))
        .unwrap_or_else(|e| panic!("fetching corpus image {relative}: {e}"))
}

/// Decodes `bytes` and re-encodes to PNG, returning the encoded output.
fn decode_to_png(bytes: &[u8], commands: Option<Vec<s::DecoderCommand>>) -> Vec<u8> {
    let mut ctx = create_context();
    ctx.add_copied_input_buffer(0, bytes).expect("add input buffer");
    ctx.add_output_buffer(1).expect("add output buffer");
    ctx.execute_1(s::Execute001 {
        graph_recording: None,
        security: None,
        job_options: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands },
            s::Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::Lodepng { maximum_deflate: None },
            },
        ]),
    })
    .expect("decode + PNG encode should succeed");
    ctx.take_output_buffer(1).expect("output buffer")
}

/// A JPEG carrying an ICC profile must actually be color-managed, not just
/// survive decoding.
///
/// Decoding the same Display-P3 file with and without `DiscardColorProfile` has
/// to produce different pixels: P3 primaries are wider than sRGB, so if the two
/// agree, the profile was never applied. Asserting only "it decoded" would pass
/// with color management removed entirely.
#[test]
fn test_icc_profile_handling() {
    let jpg = corpus_bytes("test_inputs/wide-gamut/display-p3/flickr_1b94e1228c32cb98.jpg");

    let mut ctx = create_context();
    ctx.add_copied_input_buffer(0, &jpg).expect("add input buffer");
    let info = ctx.get_unscaled_unrotated_image_info(0).expect("ICC-tagged JPEG info");
    assert_eq!("image/jpeg", info.preferred_mime_type);
    assert!(info.image_width > 0 && info.image_height > 0, "info: {:?}", info);

    let color_managed = decode_to_png(&jpg, None);
    let profile_discarded = decode_to_png(&jpg, Some(vec![s::DecoderCommand::DiscardColorProfile]));

    assert_ne!(
        color_managed, profile_discarded,
        "decoding a Display-P3 JPEG with and without DiscardColorProfile produced identical \
         output, which means the embedded ICC profile was never applied"
    );
}

// =============================================================================
// EXIF parsing
// =============================================================================

/// The EXIF orientation tag must be read back exactly, for every one of the
/// eight values.
///
/// The corpus names each file after the flag it carries, so the expected value
/// is known without hardcoding a magic number. The previous version of this test
/// timed a `get_unscaled_unrotated_image_info` call on a fixture that does not
/// exist and printed the elapsed milliseconds; it never looked at EXIF at all.
#[test]
fn test_exif_parsing_with_real_jpeg() {
    for flag in 1..=8i32 {
        let jpg = corpus_bytes(&format!("test_inputs/orientation/Landscape_{flag}.jpg"));

        let mut ctx = create_context();
        ctx.add_copied_input_buffer(0, &jpg).expect("add input buffer");

        let parsed = ctx.get_exif_rotation_flag(0).expect("reading the EXIF flag should succeed");
        assert_eq!(
            Some(flag),
            parsed,
            "Landscape_{flag}.jpg declares EXIF orientation {flag}, decoder reported {parsed:?}"
        );
    }
}

// =============================================================================
// Concurrent context creation (JOB_ID atomicity)
// =============================================================================

/// Every context gets its own `debug_job_id`, even when created concurrently.
///
/// The id comes from a single `NEXT_JOB_ID.fetch_add`, so ten contexts built in
/// parallel must come back with ten distinct ids — that is the whole point of
/// the counter being an `AtomicI32`. The previous version of this test created
/// ten contexts, dropped them, checked only that no thread panicked, and printed
/// "No crash observed"; a plain non-atomic `i32` counter would have satisfied it
/// just as well, which is precisely the bug it is named for.
///
/// This is a race detector of opportunity, not a guarantee — a lost update has
/// to actually happen on this run to be caught. ThreadSanitizer or MIRI remain
/// the tools for proving the absence of one.
#[test]
fn test_concurrent_context_creation() {
    use std::thread;

    let handles: Vec<_> =
        (0..10).map(|_| thread::spawn(|| create_context().debug_job_id)).collect();

    let ids: Vec<i32> = handles.into_iter().map(|h| h.join().expect("thread panicked")).collect();
    assert_eq!(10, ids.len());

    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        ids.len(),
        unique.len(),
        "two contexts were handed the same debug_job_id, so the counter dropped an update: {ids:?}"
    );
}

// =============================================================================
// GIF crafted-input helpers
// =============================================================================

/// Runs `Decode` -> `Encode(gif)` over `gif_bytes` on a fresh context.
///
/// The input and output buffers are registered on the context and the graph is
/// submitted with `Execute001`, which does *not* re-declare IO. An earlier shape
/// of these tests used `Build001` with `IoEnum::Placeholder` for the input;
/// `IoTranslator::add_all` rejects a placeholder outright, so every one of those
/// jobs failed with `GraphInvalid: Io Placeholder 0 was never substituted`
/// before a decoder was ever constructed. Since they only asserted "did not
/// panic", they passed while exercising nothing.
fn run_gif_decode_encode(
    gif_bytes: &[u8],
    select_frame: Option<i32>,
) -> imageflow_core::Result<s::JobResult> {
    let mut ctx = create_context();
    ctx.add_copied_input_buffer(0, gif_bytes).expect("add input buffer");
    ctx.add_output_buffer(1).expect("add output buffer");
    let commands = select_frame.map(|f| vec![s::DecoderCommand::SelectFrame(f)]);
    let payload = ctx.execute_1(s::Execute001 {
        graph_recording: None,
        security: None,
        job_options: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands },
            s::Node::Encode { io_id: 1, preset: s::EncoderPreset::Gif },
        ]),
    })?;
    match payload {
        s::ResponsePayload::JobResult(r) => Ok(r),
        other => panic!("expected JobResult, got {:?}", other),
    }
}

/// Same as [`run_gif_decode_encode`] but reads the input from `tests/crash_repro`.
fn run_gif_crash_repro(name: &str) -> imageflow_core::Result<s::JobResult> {
    let gif = fs::read(repo_root().join("imageflow_core/tests/crash_repro").join(name))
        .unwrap_or_else(|e| panic!("read fixture {}: {}", name, e));
    run_gif_decode_encode(&gif, None)
}

/// Asserts `result` failed with exactly `kind`, and returns the rendered error
/// message so the caller can pin the specific guard that fired.
///
/// Matching the kind (rather than `is_err()`, or a match arm broad enough to
/// swallow anything) is what keeps a test honest: a job that dies during setup
/// also returns `Err`, and a test that accepts any error cannot tell the failure
/// it was written for from one that never reached the code under test.
#[track_caller]
fn expect_error_kind<T: std::fmt::Debug>(
    result: imageflow_core::Result<T>,
    kind: ErrorKind,
) -> String {
    match result {
        Ok(ok) => panic!("expected error kind {:?}, but the call succeeded: {:?}", kind, ok),
        Err(e) => {
            assert_eq!(kind, e.kind, "wrong error kind; full error: {}", e);
            format!("{}", e)
        }
    }
}

/// Asserts the job failed with `kind`, and returns the rendered error message.
#[track_caller]
fn expect_gif_error(result: imageflow_core::Result<s::JobResult>, kind: ErrorKind) -> String {
    expect_error_kind(result, kind)
}

/// Asserts the job decoded and re-encoded to exactly `w` x `h`.
fn expect_gif_size(result: imageflow_core::Result<s::JobResult>, w: i32, h: i32) {
    let job = result.expect("expected the GIF pipeline to succeed");
    assert_eq!(1, job.decodes.len(), "expected one decode result: {:?}", job);
    assert_eq!((w, h), (job.decodes[0].w, job.decodes[0].h), "decoded size: {:?}", job);
    assert_eq!(1, job.encodes.len(), "expected one encode result: {:?}", job);
    assert_eq!((w, h), (job.encodes[0].w, job.encodes[0].h), "encoded size: {:?}", job);
}

/// Minimum LZW code size used by [`craft_gif_frame`].
const CRAFT_MIN_CODE_SIZE: u8 = 8;
/// Clear code for an 8-bit minimum code size.
const CRAFT_CLEAR_CODE: u16 = 256;
/// End-of-information code for an 8-bit minimum code size.
const CRAFT_EOI_CODE: u16 = 257;
/// Fixed code width; the decoder starts at `CRAFT_MIN_CODE_SIZE + 1` bits.
const CRAFT_CODE_BITS: u32 = 9;
/// How often to emit a clear code. A GIF decoder adds one dictionary entry per
/// code after the first following a clear, so at 128 literals per run the next
/// free code never passes 385 and the code width never has to grow past 9 bits.
const CRAFT_CLEAR_INTERVAL: usize = 128;

fn craft_push_code(code: u16, stream: &mut Vec<u8>, acc: &mut u32, acc_bits: &mut u32) {
    *acc |= u32::from(code) << *acc_bits;
    *acc_bits += CRAFT_CODE_BITS;
    while *acc_bits >= 8 {
        stream.push((*acc & 0xFF) as u8);
        *acc >>= 8;
        *acc_bits -= 8;
    }
}

/// Encodes `pixels` as GIF image data (minimum-code-size byte, sub-blocks, then
/// the block terminator) using literal-only LZW at a fixed 9-bit code width.
/// Every palette index round-trips exactly, so a crafted fixture can declare any
/// frame size and any index values without a real compressor.
fn craft_lzw_literals(pixels: &[u8]) -> Vec<u8> {
    let mut stream: Vec<u8> = Vec::new();
    let mut acc: u32 = 0;
    let mut acc_bits: u32 = 0;
    for (i, px) in pixels.iter().enumerate() {
        if i % CRAFT_CLEAR_INTERVAL == 0 {
            craft_push_code(CRAFT_CLEAR_CODE, &mut stream, &mut acc, &mut acc_bits);
        }
        craft_push_code(u16::from(*px), &mut stream, &mut acc, &mut acc_bits);
    }
    craft_push_code(CRAFT_EOI_CODE, &mut stream, &mut acc, &mut acc_bits);
    if acc_bits > 0 {
        stream.push((acc & 0xFF) as u8);
    }

    let mut out = vec![CRAFT_MIN_CODE_SIZE];
    for chunk in stream.chunks(255) {
        out.push(chunk.len() as u8);
        out.extend_from_slice(chunk);
    }
    out.push(0x00);
    out
}

/// GIF89a header, logical screen descriptor, and a grayscale global color table
/// with `palette_entries` (a power of two, 2..=256) entries.
fn craft_gif_header(screen_w: u16, screen_h: u16, palette_entries: usize) -> Vec<u8> {
    assert!(palette_entries.is_power_of_two() && (2..=256).contains(&palette_entries));
    let size_bits = (palette_entries.trailing_zeros() - 1) as u8;
    let mut b = Vec::new();
    b.extend_from_slice(b"GIF89a");
    b.extend_from_slice(&screen_w.to_le_bytes());
    b.extend_from_slice(&screen_h.to_le_bytes());
    // Global color table present | 8-bit color resolution | table size 2^(n+1).
    b.push(0x80 | 0x70 | size_bits);
    b.push(0x00); // background color index
    b.push(0x00); // pixel aspect ratio
    for i in 0..palette_entries {
        let v = (i * 255 / palette_entries) as u8;
        b.extend_from_slice(&[v, v, v]);
    }
    b
}

/// Appends an image descriptor at (0, 0) whose declared width and height are
/// independent of the logical screen size. `pixels`, when present, must be
/// exactly `w * h` palette indices. `None` writes the minimum-code-size byte and
/// an empty data block, which is enough for `next_frame_info()` to report the
/// frame's declared size without any of its pixels being decodable.
fn craft_gif_frame(b: &mut Vec<u8>, w: u16, h: u16, pixels: Option<&[u8]>) {
    b.push(0x2C); // image separator
    b.extend_from_slice(&0u16.to_le_bytes()); // left
    b.extend_from_slice(&0u16.to_le_bytes()); // top
    b.extend_from_slice(&w.to_le_bytes());
    b.extend_from_slice(&h.to_le_bytes());
    b.push(0x00); // no local color table, not interlaced
    match pixels {
        Some(px) => {
            assert_eq!(px.len(), usize::from(w) * usize::from(h), "pixel count must equal w*h");
            b.extend_from_slice(&craft_lzw_literals(px));
        }
        None => {
            b.push(CRAFT_MIN_CODE_SIZE);
            b.push(0x00); // empty data block
        }
    }
}

/// A deterministic `w * h` ramp of palette indices.
fn craft_pixels(w: u16, h: u16) -> Vec<u8> {
    (0..usize::from(w) * usize::from(h)).map(|i| (i % 251) as u8).collect()
}

// =============================================================================
// GIF palette bounds test
// =============================================================================

/// Pixel data may reference palette entries that do not exist. `Screen::blit`
/// has to treat those as transparent rather than indexing the palette directly.
#[test]
fn test_gif_palette_bounds() {
    // 2-entry global color table, every pixel referencing index 200.
    let mut gif = craft_gif_header(4, 4, 2);
    craft_gif_frame(&mut gif, 4, 4, Some(&[200u8; 16]));
    gif.push(0x3B);

    expect_gif_size(run_gif_decode_encode(&gif, None), 4, 4);
}

/// The same, mixing in-range and out-of-range indices in one frame.
#[test]
fn test_gif_palette_bounds_mixed_indices() {
    let mut gif = craft_gif_header(4, 4, 4);
    let pixels: Vec<u8> = (0..16u32).map(|i| if i % 2 == 0 { 1 } else { 250 }).collect();
    craft_gif_frame(&mut gif, 4, 4, Some(&pixels));
    gif.push(0x3B);

    expect_gif_size(run_gif_decode_encode(&gif, None), 4, 4);
}

// =============================================================================
// GIF frame bounds clipping (frames extending beyond canvas)
// =============================================================================

// The fixtures below are hand-built crash repros whose LZW streams stop short of
// the pixel count their descriptors promise. They must reach the decoder and be
// rejected with a decoder error — never a panic, and never a graph error that
// would mean the decoder was skipped.

#[test]
fn test_gif_frame_exceeds_canvas() {
    expect_gif_error(
        run_gif_crash_repro("gif_frame_exceeds_canvas.gif"),
        ErrorKind::GifDecodingError,
    );
}

#[test]
fn test_gif_overflow_frame_position() {
    expect_gif_error(
        run_gif_crash_repro("gif_overflow_frame_pos.gif"),
        ErrorKind::GifDecodingError,
    );
}

#[test]
fn test_gif_zero_size_frame() {
    expect_gif_error(run_gif_crash_repro("gif_zero_size_frame.gif"), ErrorKind::GifDecodingError);
}

// =============================================================================
// GIF invalid background color index
// =============================================================================

#[test]
fn test_gif_bad_bg_index() {
    expect_gif_error(run_gif_crash_repro("gif_bad_bg_index.gif"), ErrorKind::GifDecodingError);
}

// =============================================================================
// GIF out-of-bounds palette index in pixel data
// =============================================================================

#[test]
fn test_gif_oob_palette_in_pixels() {
    let message = expect_gif_error(
        run_gif_crash_repro("gif_oob_palette_index.gif"),
        ErrorKind::GifDecodingError,
    );
    assert!(message.contains("LZW"), "expected an LZW decode failure, got: {}", message);
}

// =============================================================================
// GIF frame buffer sizing (crash repro + the guard it exists for)
// =============================================================================

// `gif::Reader::buffer_size()` is derived from the *frame's* image descriptor,
// not from the logical screen descriptor, and the two are unrelated in a crafted
// file. Sizing the reusable frame buffer as `screen_w * screen_h` and then
// slicing it to `buffer_size()` therefore panicked on out-of-range slice
// indices for any frame larger than the screen (fixed in b2808b73). The tests
// below drive each branch of the replacement: allocate at `required`, grow to
// `required` when a later frame needs more, and refuse anything over 16 MP.

/// The committed crash repro: a 34-byte GIF87a whose logical screen is 0 x 256
/// but whose image descriptor declares a 65535 x 65519 frame at (18746, 65535).
/// 65535 * 65519 = 4,293,787,665 bytes, so the 16 MP cap is what stops it.
#[test]
fn test_gif_frame_buffer_oob() {
    let message = expect_gif_error(
        run_gif_crash_repro("gif_frame_buffer_oob.gif"),
        ErrorKind::SizeLimitExceeded,
    );
    assert!(
        message.contains("GIF frame buffer_size 4293787665 exceeds 16MP limit"),
        "unexpected error message: {}",
        message
    );
}

/// A frame larger than the logical screen, well under the 16 MP cap: the buffer
/// must be allocated at the frame's size, and `Screen::blit` clips to the canvas.
#[test]
fn test_gif_frame_larger_than_logical_screen_decodes() {
    let mut gif = craft_gif_header(4, 4, 256);
    craft_gif_frame(&mut gif, 64, 64, Some(&craft_pixels(64, 64)));
    gif.push(0x3B);

    expect_gif_size(run_gif_decode_encode(&gif, None), 4, 4);
}

/// A small frame followed by a larger one. The buffer is allocated for the first
/// frame, so the second has to grow it before use.
#[test]
fn test_gif_frame_buffer_grows_between_frames() {
    let mut gif = craft_gif_header(4, 4, 256);
    craft_gif_frame(&mut gif, 4, 4, Some(&craft_pixels(4, 4)));
    craft_gif_frame(&mut gif, 64, 64, Some(&craft_pixels(64, 64)));
    gif.push(0x3B);

    // Selecting frame 1 composites frame 0 first, so both frames go through the
    // buffer: the small one allocates it, the large one must resize it.
    expect_gif_size(run_gif_decode_encode(&gif, Some(1)), 4, 4);
}

/// A frame declaring 5000 x 5000 = 25,000,000 bytes, over the 16 MP cap. No
/// pixel data is needed — the cap is checked before anything is decoded.
#[test]
fn test_gif_frame_buffer_size_cap_rejects_oversized_frame() {
    let mut gif = craft_gif_header(4, 4, 256);
    craft_gif_frame(&mut gif, 5000, 5000, None);
    gif.push(0x3B);

    let message = expect_gif_error(run_gif_decode_encode(&gif, None), ErrorKind::SizeLimitExceeded);
    assert!(
        message.contains("GIF frame buffer_size 25000000 exceeds 16MP limit"),
        "unexpected error message: {}",
        message
    );
}

/// The cap also has to hold for frames that are only composited on the way to a
/// requested frame, which is a separate copy of the check.
#[test]
fn test_gif_frame_buffer_size_cap_applies_to_skipped_frames() {
    let mut gif = craft_gif_header(4, 4, 256);
    craft_gif_frame(&mut gif, 5000, 5000, None);
    craft_gif_frame(&mut gif, 4, 4, Some(&craft_pixels(4, 4)));
    gif.push(0x3B);

    let message =
        expect_gif_error(run_gif_decode_encode(&gif, Some(1)), ErrorKind::SizeLimitExceeded);
    assert!(
        message.contains("GIF frame buffer_size 25000000 exceeds 16MP limit"),
        "unexpected error message: {}",
        message
    );
}

// =============================================================================
// WebP with oversized RIFF claim
// =============================================================================

#[test]
fn test_webp_oversized_riff_claim() {
    // Create a minimal WebP that claims a very large size in its RIFF header
    let mut webp = Vec::new();

    // RIFF header
    webp.extend_from_slice(b"RIFF");
    // File size - 8 (claim huge size)
    webp.extend_from_slice(&0x10000000u32.to_le_bytes()); // 256MB
    webp.extend_from_slice(b"WEBP");

    // VP8 chunk (minimal)
    webp.extend_from_slice(b"VP8 ");
    webp.extend_from_slice(&20u32.to_le_bytes());

    // VP8 bitstream header
    webp.extend_from_slice(&[0x9D, 0x01, 0x2A]); // signature
    webp.extend_from_slice(&8u16.to_le_bytes()); // width
    webp.extend_from_slice(&8u16.to_le_bytes()); // height
    webp.extend_from_slice(&[0x00; 12]); // padding

    let mut ctx = create_context();
    ctx.add_copied_input_buffer(0, &webp).expect("adding the buffer must succeed");

    // The RIFF header claims 256 MB but only ~30 bytes follow. The decoder must
    // reject the truncated bitstream rather than trusting the size claim — and
    // it must reject it as a *decoding* error, which is only observable if the
    // buffer actually reached the WebP decoder.
    expect_error_kind(ctx.get_unscaled_unrotated_image_info(0), ErrorKind::ImageDecodingError);
}

// =============================================================================
// PNG ICC profile lifetime test
// =============================================================================

/// The ICC buffer taken from a PNG's `iCCP` chunk has to stay valid for the
/// whole transform, not just until the chunk reader is dropped.
///
/// Decoding with and without `DiscardColorProfile` must differ — that is what
/// proves the profile was read *and* used. A lifetime bug shows up here as a
/// crash, or (under valgrind/ASAN) as a use-after-free; run it under those for
/// full detection. The previous version pointed at
/// `imageflow_core/tests/visuals/01864661ED8AB31EF.png`, a directory that does
/// not exist, and printed "No test PNG found".
#[test]
fn test_png_icc_lifetime() {
    let png = corpus_bytes(
        "test_inputs/repro-icc/sharp/1323_115925293-3319d700-a481-11eb-8083-66b5188ee1da.png",
    );

    let mut ctx = create_context();
    ctx.add_copied_input_buffer(0, &png).expect("add input buffer");
    let info = ctx.get_unscaled_unrotated_image_info(0).expect("ICC-tagged PNG info");
    assert_eq!("image/png", info.preferred_mime_type);
    assert!(info.image_width > 0 && info.image_height > 0, "info: {:?}", info);

    let color_managed = decode_to_png(&png, None);
    let profile_discarded = decode_to_png(&png, Some(vec![s::DecoderCommand::DiscardColorProfile]));

    assert_ne!(
        color_managed, profile_discarded,
        "decoding an iCCP-tagged PNG with and without DiscardColorProfile produced identical \
         output, which means the embedded ICC profile was never applied"
    );
}

// =============================================================================
// Summary
// =============================================================================

#[test]
fn run_robustness_summary() {
    println!("\n============================================================");
    println!("ROBUSTNESS TEST SUMMARY");
    println!("============================================================\n");

    println!("For deeper analysis:");
    println!("1. Run with: RUST_BACKTRACE=1 cargo test --release");
    println!("2. Run with AddressSanitizer: RUSTFLAGS='-Zsanitizer=address' cargo +nightly test");
    println!("3. Run with ThreadSanitizer for race conditions");
    println!("4. Use valgrind for memory analysis");
    println!("\nNote: Many issues only manifest under specific conditions");
    println!("(large allocations, memory pressure, specific file contents)");
}
