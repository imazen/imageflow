//! One-shot migration: convert per-test TOML checksum files to `.checksums` v1 format.
//!
//! Run with: `cargo test -p imageflow_core --test integration migrate_toml_to_v2 -- --nocapture`
//!
//! Reads all `.toml` files in `tests/visuals/checksums/`, groups them by
//! module (using a function-name-to-module mapping), and writes one
//! `.checksums` file per module.
//!
//! Safe to re-run: overwrites existing `.checksums` files with fresh content.
//! Does NOT delete the TOML files — do that manually after verifying.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// Map from function name → test module file stem.
///
/// Built by examining which test functions live in which `visuals/*.rs` file.
fn function_to_module() -> HashMap<&'static str, &'static str> {
    [
        // scaling.rs
        ("test_scale_image", "scaling"),
        ("test_scale_rings", "scaling"),
        ("test_read_gif_and_scale", "scaling"),
        ("test_read_gif_and_vertical_distort", "scaling"),
        ("webp_lossless_alpha_decode_and_scale", "scaling"),
        ("webp_lossy_alpha_decode_and_scale", "scaling"),
        ("webp_lossy_noalpha_decode_and_scale", "scaling"),
        ("test_jpeg_icc2_color_profile", "scaling"),
        ("test_jpeg_icc4_color_profile", "scaling"),
        // canvas.rs
        ("test_fill_rect", "canvas"),
        ("test_fill_rect_original", "canvas"),
        ("test_expand_rect", "canvas"),
        ("test_crop", "canvas"),
        ("test_off_surface_region", "canvas"),
        ("test_transparent_canvas", "canvas"),
        ("test_partial_region", "canvas"),
        ("test_pixels_region", "canvas"),
        ("test_detect_whitespace", "canvas"),
        ("test_round_corners_large", "canvas"),
        ("test_round_corners_small", "canvas"),
        ("test_round_corners_custom_pixels", "canvas"),
        ("test_round_corners_custom_percent", "canvas"),
        ("test_round_corners_excessive_radius", "canvas"),
        ("test_round_corners_circle_wide_canvas", "canvas"),
        ("test_round_corners_circle_tall_canvas", "canvas"),
        ("test_round_image_corners_transparent", "canvas"),
        ("test_round_corners_command_string", "canvas"),
        // color.rs
        ("test_simple_filters", "color"),
        ("test_white_balance_image", "color"),
        ("test_white_balance_image_threshold_5", "color"),
        // orientation.rs
        ("test_jpeg_rotation", "orientation"),
        ("test_jpeg_rotation_cropped", "orientation"),
        ("test_crop_exif", "orientation"),
        ("test_fit_pad_exif", "orientation"),
        // watermark.rs
        ("test_watermark_image", "watermark"),
        ("test_watermark_image_command_string", "watermark"),
        ("test_watermark_image_command_string_with_bgcolor", "watermark"),
        ("test_watermark_image_small", "watermark"),
        ("test_watermark_image_pixel_margins", "watermark"),
        ("test_watermark_image_on_png", "watermark"),
        ("test_watermark_jpeg_over_pnga", "watermark"),
        // codec.rs
        ("test_encode_gradients", "codec"),
        ("test_transparent_png_to_png", "codec"),
        ("test_problematic_png_lossy", "codec"),
        ("test_transparent_png_to_png_rounded_corners", "codec"),
        ("test_transparent_png_to_jpeg", "codec"),
        ("test_transparent_png_to_jpeg_constrain", "codec"),
        ("test_matte_transparent_png", "codec"),
        ("test_branching_crop_whitespace", "codec"),
        ("test_transparent_webp_to_webp", "codec"),
        ("test_webp_to_webp_quality", "codec"),
        ("test_jpeg_simple", "codec"),
        ("test_jpeg_simple_rot_90", "codec"),
        ("test_rot_90_and_red_dot", "codec"),
        ("test_rot_90_and_red_dot_command_string", "codec"),
        ("test_negatives_in_command_string", "codec"),
        ("test_jpeg_crop", "codec"),
        ("decode_cmyk_jpeg", "codec"),
        ("decode_rgb_with_cmyk_profile_jpeg", "codec"),
        ("test_crop_with_preshrink", "codec"),
        // trim.rs
        ("test_trim_whitespace", "trim"),
        ("test_trim_whitespace_with_padding", "trim"),
        ("test_trim_resize_whitespace_with_padding", "trim"),
        ("test_trim_resize_whitespace_without_padding", "trim"),
        ("test_trim_whitespace_with_padding_no_resize", "trim"),
        // idct.rs
        ("test_idct_linear", "idct"),
        ("test_idct_spatial_no_gamma", "idct"),
    ]
    .into_iter()
    .collect()
}

/// Extract the function name from a TOML test name.
///
/// Convention: test names are either `"func_name detail"` (space) or
/// `"func_name/variant"` (slash, for loop tests). Returns the part
/// before the first separator.
fn extract_func_name(full_name: &str) -> &str {
    full_name
        .split(|c| c == ' ' || c == '/')
        .next()
        .unwrap_or(full_name)
}

/// Resolve the function name to a module, handling ambiguous prefix matches.
///
/// Some function names are prefixes of others (e.g., `test_fill_rect` vs
/// `test_fill_rect_original`). We need longest-match semantics.
fn resolve_module<'a>(func_name: &str, map: &'a HashMap<&str, &str>) -> Option<&'a str> {
    // Try exact match first
    if let Some(module) = map.get(func_name) {
        return Some(module);
    }
    // No match — function might have a suffix we don't recognize
    None
}

#[test]
fn migrate_toml_to_v2() {
    let visuals = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("visuals");
    let checksums_dir = visuals.join("checksums");

    if !checksums_dir.exists() {
        println!("No checksums/ directory found, nothing to migrate");
        return;
    }

    // Build the full name → module mapping by reading TOML files
    let func_map = function_to_module();
    let mut module_mapping: BTreeMap<String, String> = BTreeMap::new();
    let mut unmapped = Vec::new();

    // Read all TOML files to get their name fields
    let entries = std::fs::read_dir(&checksums_dir).unwrap();
    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }

        let toml = zensim_regress::checksum_file::TestChecksumFile::read_from(&path).unwrap();
        let func_name = extract_func_name(&toml.name);

        if let Some(module) = resolve_module(func_name, &func_map) {
            module_mapping.insert(toml.name.clone(), module.to_string());
        } else {
            unmapped.push(toml.name.clone());
        }
    }

    if !unmapped.is_empty() {
        println!("Warning: {} tests have no module mapping:", unmapped.len());
        for name in &unmapped {
            println!("  {name}");
        }
        println!("These will be placed in the 'tests' module.");
    }

    // Run the migration
    let result = zensim_regress::checksums_v2::migrate_toml_dir(&checksums_dir, &module_mapping)
        .expect("Migration failed");

    // Write the .checksums files
    let mut total_sections = 0;
    for (module_name, checksums_file) in &result {
        let path = checksums_dir.join(format!("{module_name}.checksums"));
        let content = checksums_file.format();
        std::fs::write(&path, &content).unwrap();

        let section_count = checksums_file.sections.len();
        total_sections += section_count;
        println!(
            "Wrote {}: {} sections, {} bytes",
            path.display(),
            section_count,
            content.len()
        );
    }

    println!("\n=== Migration complete ===");
    println!("Modules: {}", result.len());
    println!("Total sections: {total_sections}");
    println!("Unmapped (in 'tests'): {}", unmapped.len());
}
