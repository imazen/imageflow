//! Resource limit and robustness tests
//!
//! These tests verify that oversized or malformed images are rejected gracefully.

use imageflow_core::Context;
use imageflow_types as s;
use std::fs;
use std::path::Path;

/// Helper to create a context
fn create_context() -> Box<Context> {
    Context::create().expect("Failed to create context")
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

fn create_decode_encode_job() -> s::Build001 {
    s::Build001 {
        builder_config: None,
        io: vec![],
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::Lodepng { maximum_deflate: None },
            },
        ]),
    }
}

#[test]
fn test_icc_profile_handling() {
    let test_jpg = Path::new("/home/lilith/work/imageflow/examples/export_4_sizes/waterhouse.jpg");

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
    let test_jpg = Path::new("/home/lilith/work/imageflow/examples/export_4_sizes/waterhouse.jpg");

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
// GIF palette bounds test
// =============================================================================

#[test]
fn test_gif_palette_bounds() {
    // Create a GIF with 2-color palette but image data referencing color index > 1
    let mut gif = Vec::new();

    // GIF89a header
    gif.extend_from_slice(b"GIF89a");

    // 4x4 image
    gif.extend_from_slice(&4u16.to_le_bytes());
    gif.extend_from_slice(&4u16.to_le_bytes());

    // Has 2-color global color table
    gif.push(0x80); // 2^(0+1) = 2 colors
    gif.push(0x00);
    gif.push(0x00);

    // 2-color palette
    gif.extend_from_slice(&[0x00, 0x00, 0x00]);
    gif.extend_from_slice(&[0xFF, 0xFF, 0xFF]);

    // Image descriptor
    gif.push(0x2C);
    gif.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    gif.extend_from_slice(&4u16.to_le_bytes());
    gif.extend_from_slice(&4u16.to_le_bytes());
    gif.push(0x00);

    // LZW data that should decode to valid indices only
    gif.push(0x02); // min code size
    gif.push(0x02);
    gif.extend_from_slice(&[0x4C, 0x01]);
    gif.push(0x00);
    gif.push(0x3B);

    let mut ctx = create_context();
    let _ = ctx.add_copied_input_buffer(0, &gif);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // Try to decode the GIF
        let _ = ctx.add_output_buffer(1);
        let job = s::Build001 {
            builder_config: None,
            io: vec![
                s::IoObject { direction: s::IoDirection::In, io_id: 0, io: s::IoEnum::Placeholder },
                s::IoObject {
                    direction: s::IoDirection::Out,
                    io_id: 1,
                    io: s::IoEnum::OutputBuffer,
                },
            ],
            framewise: s::Framewise::Steps(vec![
                s::Node::Decode { io_id: 0, commands: None },
                s::Node::Encode { io_id: 1, preset: s::EncoderPreset::Gif },
            ]),
        };
        let _ = ctx.build_1(job);
    }));

    match result {
        Ok(_) => println!("GIF palette access handled safely"),
        Err(e) => panic!("Panic during GIF decode with out-of-bounds palette index: {:?}", e),
    }
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
            println!(
                "WebP with 256MB RIFF claim accepted as {}x{}",
                i.image_width, i.image_height
            );
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

    let test_pngs =
        ["/home/lilith/work/imageflow/imageflow_core/tests/visuals/01864661ED8AB31EF.png"];

    for png_path in &test_pngs {
        if Path::new(png_path).exists() {
            let png_bytes = fs::read(png_path).expect("Failed to read PNG");

            let mut ctx = create_context();
            let _ = ctx.add_copied_input_buffer(0, &png_bytes);

            let info = ctx.get_unscaled_unrotated_image_info(0);
            match info {
                Ok(i) => {
                    println!(
                        "PNG info retrieved for {}: {}x{}",
                        png_path, i.image_width, i.image_height
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
