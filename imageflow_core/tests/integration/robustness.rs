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

/// Direct test of Screen::new with large dimensions
/// This tests if the gif crate's memory limit is applied BEFORE Screen::new
#[test]
fn test_gif_screen_allocation_order() {
    // Create a valid minimal GIF with maximum allowed dimensions
    // GIF dimensions are u16, so max is 65535x65535
    // But gif crate has a memory limit of 8000*8000 = 64MB

    // Test with dimensions that fit in memory limit
    let valid_gif = create_valid_gif(100, 100);

    let mut ctx = create_context();
    let result = ctx.add_copied_input_buffer(0, &valid_gif);
    assert!(result.is_ok(), "Valid 100x100 GIF should be accepted");

    let info = ctx.get_unscaled_unrotated_image_info(0);
    assert!(info.is_ok(), "Should get info for valid GIF");
}

/// Test GIF with dimensions just at the memory limit
#[test]
fn test_gif_at_memory_limit() {
    // 8000 * 8000 = 64,000,000 which is at the gif crate's limit
    // This tests if the limit is inclusive or exclusive
    let gif = create_valid_gif(8000, 8000);

    let mut ctx = create_context();
    let result = ctx.add_copied_input_buffer(0, &gif);

    // Note: This should either succeed (if limit is inclusive) or fail gracefully
    println!("GIF 8000x8000 result: {:?}", result);

    if result.is_ok() {
        let info = ctx.get_unscaled_unrotated_image_info(0);
        match info {
            Ok(i) => println!(
                "GIF 8000x8000 accepted: {}x{} = {} pixels",
                i.image_width,
                i.image_height,
                i.image_width as i64 * i.image_height as i64
            ),
            Err(e) => println!("GIF 8000x8000 info error: {:?}", e),
        }
    }
}

/// Test GIF just over the memory limit — should be rejected by dimension validation
#[test]
fn test_gif_over_memory_limit() {
    // 8001 * 8001 = 64,016,001 which is just over the 64MB limit
    let gif = create_valid_gif(8001, 8001);

    let mut ctx = create_context();
    let result = ctx.add_copied_input_buffer(0, &gif);

    if result.is_ok() {
        let info = ctx.get_unscaled_unrotated_image_info(0);
        match info {
            Ok(i) => {
                // If we get here, dimension validation didn't reject it before allocation.
                // The allocation is short-lived and freed quickly, but we'd prefer to reject early.
                println!(
                    "GIF over memory limit accepted: {}x{} ({} pixels, {} MB)",
                    i.image_width,
                    i.image_height,
                    i.image_width as i64 * i.image_height as i64,
                    (i.image_width as i64 * i.image_height as i64 * 4) / 1024 / 1024
                );
            }
            Err(e) => println!("GIF 8001x8001 rejected at info stage: {:?}", e),
        }
    } else {
        println!("GIF 8001x8001 rejected at buffer stage");
    }
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

#[test]
fn test_bitmap_canvas_at_limit() {
    let mut ctx = create_context();

    // max_frame_size default is 100 megapixels (10000x10000)
    let job = create_canvas_job(10000, 10000);

    let result = ctx.build_1(job);
    match result {
        Ok(_) => {
            println!("10000x10000 canvas: accepted (100MP, at limit)");
        }
        Err(e) => {
            println!("10000x10000 canvas rejected: {:?}", e);
        }
    }
}

#[test]
fn test_bitmap_canvas_over_limit() {
    let mut ctx = create_context();

    // 10001x10001 = 100,020,001 which is over 100MP limit
    let job = create_canvas_job(10001, 10001);

    let result = ctx.build_1(job);
    match result {
        Ok(_) => {
            panic!("10001x10001 canvas accepted (should be over limit)");
        }
        Err(e) => {
            println!("10001x10001 canvas properly rejected: {:?}", e);
        }
    }
}

#[test]
fn test_bitmap_canvas_i32_overflow() {
    let mut ctx = create_context();

    // 46341 * 46341 = 2,147,488,281 which overflows i32
    let job = create_canvas_job(46341, 46341);

    let result = ctx.build_1(job);
    match result {
        Ok(_) => {
            panic!("46341x46341 canvas accepted (would overflow i32 in product)");
        }
        Err(e) => {
            println!("46341x46341 canvas properly rejected: {:?}", e);
        }
    }
}

// =============================================================================
// ICC profile and EXIF handling tests
// =============================================================================

#[test]
fn test_icc_profile_handling() {
    let test_jpg = repo_root().join("examples/export_4_sizes/waterhouse.jpg");

    if test_jpg.exists() {
        let jpg_bytes = fs::read(test_jpg).expect("Failed to read test JPEG");

        let mut ctx = create_context();
        let _ = ctx.add_copied_input_buffer(0, &jpg_bytes);

        let info = ctx.get_unscaled_unrotated_image_info(0);
        match info {
            Ok(i) => {
                println!("Test JPEG: {}x{}", i.image_width, i.image_height);
                // ICC profile parsing happens during get_unscaled_unrotated_image_info
                // Issues would manifest under valgrind/ASAN if ICC handling is broken
            }
            Err(e) => {
                println!("Test JPEG info failed: {:?}", e);
            }
        }
    } else {
        println!("Test JPEG not found, skipping ICC tests");
    }
}

// =============================================================================
// EXIF parsing timing test
// =============================================================================

#[test]
fn test_exif_parsing_with_real_jpeg() {
    let test_jpg = repo_root().join("examples/export_4_sizes/waterhouse.jpg");

    if test_jpg.exists() {
        let jpg_bytes = fs::read(test_jpg).expect("Failed to read test JPEG");

        let mut ctx = create_context();
        let _ = ctx.add_copied_input_buffer(0, &jpg_bytes);

        let start = std::time::Instant::now();
        let info = ctx.get_unscaled_unrotated_image_info(0);
        let elapsed = start.elapsed();

        match info {
            Ok(i) => {
                println!(
                    "Normal JPEG info took {}ms: {}x{}",
                    elapsed.as_millis(),
                    i.image_width,
                    i.image_height
                );
            }
            Err(e) => {
                println!("Normal JPEG info failed in {}ms: {:?}", elapsed.as_millis(), e);
            }
        }
    }
}

// =============================================================================
// Concurrent context creation (JOB_ID atomicity)
// =============================================================================

#[test]
fn test_concurrent_context_creation() {
    use std::thread;

    // Create multiple contexts in parallel to exercise AtomicI32 JOB_ID
    let handles: Vec<_> = (0..10)
        .map(|_| {
            thread::spawn(|| {
                let ctx = create_context();
                drop(ctx);
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    println!("No crash observed in multi-threaded context creation");
    println!("Note: Use ThreadSanitizer or MIRI for proper race detection");
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

/// Asserts the job failed with `kind`, and returns the rendered error message.
fn expect_gif_error(result: imageflow_core::Result<s::JobResult>, kind: ErrorKind) -> String {
    match result {
        Ok(ok) => panic!("expected error kind {:?}, but the job succeeded: {:?}", kind, ok),
        Err(e) => {
            assert_eq!(kind, e.kind, "wrong error kind; full error: {}", e);
            format!("{}", e)
        }
    }
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

/// The committed crash repro. Its frame descriptor declares 65527 x 65535, so
/// the 16 MP cap is what stops it.
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
    let _ = ctx.add_copied_input_buffer(0, &webp);

    let result = ctx.get_unscaled_unrotated_image_info(0);
    match result {
        Ok(i) => {
            println!("WebP with 256MB RIFF claim accepted as {}x{}", i.image_width, i.image_height);
        }
        Err(e) => {
            println!("WebP rejected: {:?}", e);
        }
    }
}

// =============================================================================
// PNG ICC profile lifetime test
// =============================================================================

#[test]
fn test_png_icc_lifetime() {
    // Test PNG ICC profile handling — the ICC buffer must remain valid for the
    // duration of processing. Run under valgrind/ASAN for full detection.

    let visuals_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/visuals");
    let test_pngs = [visuals_dir.join("01864661ED8AB31EF.png")];

    for png_path in &test_pngs {
        if png_path.exists() {
            let png_bytes = fs::read(png_path).expect("Failed to read PNG");

            let mut ctx = create_context();
            let _ = ctx.add_copied_input_buffer(0, &png_bytes);

            let info = ctx.get_unscaled_unrotated_image_info(0);
            match info {
                Ok(i) => {
                    println!(
                        "PNG info retrieved for {}: {}x{}",
                        png_path.display(),
                        i.image_width,
                        i.image_height
                    );
                    println!("Note: Use valgrind/ASAN to detect lifetime issues");
                }
                Err(e) => {
                    println!("PNG info failed: {:?}", e);
                }
            }

            return; // Test one file
        }
    }

    println!("No test PNG found");
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
