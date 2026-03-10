//! Upload sync and verification for CI pipelines.
//!
//! Run with:
//! ```sh
//! UPLOAD_REFERENCES=1 cargo test -p imageflow_core --test integration sync_and_verify_uploads -- --ignored
//! ```
//!
//! This will:
//! 1. Upload all reference images not yet in uploaded.log
//! 2. Upload .checksums files to S3
//! 3. Update uploaded.log
//! 4. **Fail** if any referenced image couldn't be uploaded

use crate::common::upload_tracker;
use std::path::Path;

/// Sync all reference images to S3 and fail if any are missing.
///
/// This test is `#[ignore]`d so it doesn't run during normal `cargo test`.
/// CI pipelines should run it explicitly after the main test suite.
#[test]
#[ignore]
fn sync_and_verify_uploads() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let checksums_dir = manifest.join("tests/integration/visuals");
    let cache_dir = checksums_dir.join(".remote-cache");

    let upload_prefix = std::env::var("REGRESS_UPLOAD_PREFIX")
        .ok()
        .and_then(|v| if v.is_empty() { None } else { Some(v) })
        .or_else(|| Some("s3://imageflow-resources/visual_test_checksums".to_string()));
    let upload_enabled = std::env::var("UPLOAD_REFERENCES").is_ok_and(|v| v == "1" || v == "true");

    let storage = zensim_regress::remote::ReferenceStorage::new(
        "https://s3-us-west-2.amazonaws.com/imageflow-resources/visual_test_checksums",
        upload_prefix,
        upload_enabled,
        &cache_dir,
    );

    if !storage.uploads_configured() {
        eprintln!(
            "WARNING: Uploads not configured (need UPLOAD_REFERENCES=1 and REGRESS_UPLOAD_PREFIX).\n\
             Running in verify-only mode — will check uploaded.log but cannot upload."
        );
    }

    let result = upload_tracker::sync_and_verify(&checksums_dir, &storage);

    println!("\n=== Upload Sync Summary ===");
    println!("  Referenced in .checksums: {}", result.total_referenced);
    println!("  Already uploaded:         {}", result.already_uploaded);
    println!("  Newly uploaded:           {}", result.newly_uploaded);
    println!("  Not uploaded:             {}", result.failed.len());

    if !result.failed.is_empty() {
        eprintln!("\nNOT UPLOADED:");
        for name in &result.failed {
            eprintln!("  - {name}");
        }
        panic!(
            "{} reference image(s) not uploaded. \
             Run tests first to generate local images, then re-run with \
             UPLOAD_REFERENCES=1 REGRESS_UPLOAD_PREFIX=s3://imageflow-resources/visual_test_checksums",
            result.failed.len()
        );
    }

    println!("\nAll {} reference images verified.", result.total_referenced);
}

/// Backfill diff stats on auto-accepted entries that are missing them.
///
/// Downloads both images (actual + baseline) from S3, runs zensim comparison,
/// and updates the `.checksums` entry with the diff summary.
///
/// Run with:
/// ```sh
/// cargo test -p imageflow_core --test integration backfill_diff_stats -- --ignored --nocapture
/// ```
#[test]
#[ignore]
fn backfill_diff_stats() {
    use zensim_regress::checksums::{ChecksumsFile, EntryKind};
    use zensim_regress::diff_summary::format_diff_summary;
    use zensim_regress::testing::{check_regression, RegressionTolerance};

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let checksums_dir = manifest.join("tests/integration/visuals");
    let cache_dir = checksums_dir.join(".remote-cache");
    let base_url = "https://s3-us-west-2.amazonaws.com/imageflow-resources/visual_test_checksums";

    let zensim = zensim::Zensim::new(zensim::ZensimProfile::latest());
    let mut total = 0;
    let mut updated = 0;
    let mut failed = 0;

    for entry in std::fs::read_dir(&checksums_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "checksums") {
            let mut file = ChecksumsFile::read_from(&path).unwrap();
            let mut modified = false;

            for section in &mut file.sections {
                for entry in &mut section.entries {
                    // Only process auto-accepted entries without diff stats that have a vs_ref
                    if entry.kind != EntryKind::AutoAccepted
                        || entry.diff_summary.is_some()
                        || entry.vs_ref.is_none()
                    {
                        continue;
                    }
                    // Skip entries already marked as "too small"
                    if entry.reason.contains("too small") {
                        continue;
                    }
                    // Skip entries that already have old-format diff stats in vs_ref
                    if entry
                        .vs_ref
                        .as_ref()
                        .is_some_and(|v| v.contains("diff ") || v.contains("(zensim:"))
                    {
                        continue;
                    }

                    total += 1;
                    let actual_petname = &entry.name_hash;
                    let baseline_petname = entry.vs_ref.as_ref().unwrap();

                    // Download both images
                    let actual_path = download_to_cache(base_url, actual_petname, &cache_dir);
                    let baseline_path = download_to_cache(base_url, baseline_petname, &cache_dir);

                    let (actual_path, baseline_path) = match (actual_path, baseline_path) {
                        (Some(a), Some(b)) => (a, b),
                        _ => {
                            eprintln!(
                                "SKIP {actual_petname} vs {baseline_petname}: image not available"
                            );
                            failed += 1;
                            continue;
                        }
                    };

                    // Decode images (guess format from content, not extension)
                    let actual_img = match load_image_guessing_format(&actual_path) {
                        Ok(img) => img,
                        Err(e) => {
                            eprintln!("SKIP {actual_petname}: decode error: {e}");
                            failed += 1;
                            continue;
                        }
                    };
                    let baseline_img = match load_image_guessing_format(&baseline_path) {
                        Ok(img) => img,
                        Err(e) => {
                            eprintln!("SKIP {baseline_petname}: decode error: {e}");
                            failed += 1;
                            continue;
                        }
                    };

                    let (aw, ah) = actual_img.dimensions();
                    let (bw, bh) = baseline_img.dimensions();

                    if aw != bw || ah != bh {
                        eprintln!(
                            "SKIP {actual_petname}: dimension mismatch ({aw}x{ah} vs {bw}x{bh})"
                        );
                        failed += 1;
                        continue;
                    }

                    // Convert to zensim pixel format
                    let actual_pixels: Vec<[u8; 4]> = actual_img
                        .as_raw()
                        .chunks_exact(4)
                        .map(|c| [c[0], c[1], c[2], c[3]])
                        .collect();
                    let baseline_pixels: Vec<[u8; 4]> = baseline_img
                        .as_raw()
                        .chunks_exact(4)
                        .map(|c| [c[0], c[1], c[2], c[3]])
                        .collect();

                    let actual_src =
                        zensim::RgbaSlice::new(&actual_pixels, aw as usize, ah as usize);
                    let baseline_src =
                        zensim::RgbaSlice::new(&baseline_pixels, bw as usize, bh as usize);

                    // Run comparison with permissive tolerance (we just want the report)
                    let tol = RegressionTolerance::off_by_one()
                        .with_max_delta(255)
                        .with_min_similarity(0.0)
                        .with_max_alpha_delta(255);

                    match check_regression(&zensim, &baseline_src, &actual_src, &tol) {
                        Ok(report) => {
                            let diff = format_diff_summary(&report);
                            println!("{actual_petname} vs {baseline_petname}: {diff}");
                            entry.diff_summary = Some(diff);
                            modified = true;
                            updated += 1;
                        }
                        Err(zensim::ZensimError::ImageTooSmall) => {
                            eprintln!("SKIP {actual_petname}: image too small for zensim");
                            entry.reason = "auto-accepted (image too small for zensim)".to_string();
                            modified = true;
                            updated += 1;
                        }
                        Err(e) => {
                            eprintln!("ERROR {actual_petname}: {e}");
                            failed += 1;
                        }
                    }
                }
            }

            if modified {
                file.write_to(&path).unwrap();
                println!("Updated: {}", path.display());
            }
        }
    }

    println!("\n=== Backfill Summary ===");
    println!("  Entries needing diff stats: {total}");
    println!("  Updated:                    {updated}");
    println!("  Failed:                     {failed}");

    if failed > 0 {
        eprintln!("WARNING: {failed} entries could not be backfilled (images not on S3)");
    }
}

/// Load an image, guessing format from file content rather than extension.
fn load_image_guessing_format(path: &Path) -> Result<image::RgbaImage, Box<dyn std::error::Error>> {
    let data = std::fs::read(path)?;
    let format = image::guess_format(&data)?;
    let img = image::load_from_memory_with_format(&data, format)?;
    Ok(img.to_rgba8())
}

/// Download a petname image from S3 to the local cache, returning the cached path.
fn download_to_cache(
    base_url: &str,
    petname: &str,
    cache_dir: &Path,
) -> Option<std::path::PathBuf> {
    let filename = zensim_regress::remote::ReferenceStorage::remote_filename(petname);
    let cached = cache_dir.join(&filename);
    if cached.exists() {
        return Some(cached);
    }

    let url = format!("{base_url}/{filename}");
    let _ = std::fs::create_dir_all(cache_dir);

    // Use curl to download
    let output = std::process::Command::new("curl")
        .args(["-sf", "-o"])
        .arg(&cached)
        .arg(&url)
        .output()
        .ok()?;

    if output.status.success() && cached.exists() {
        Some(cached)
    } else {
        let _ = std::fs::remove_file(&cached);
        None
    }
}

/// Verify-only: check that all .checksums references are in uploaded.log.
///
/// Does NOT attempt uploads — just checks the log. Fast, no credentials needed.
/// Run with:
/// ```sh
/// cargo test -p imageflow_core --test integration verify_upload_log -- --ignored
/// ```
#[test]
#[ignore]
fn verify_upload_log() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let checksums_dir = manifest.join("tests/integration/visuals");

    let all_refs = upload_tracker::all_referenced_petnames(&checksums_dir);
    let uploaded = upload_tracker::read_uploaded_log();

    let missing: Vec<&String> = all_refs.difference(&uploaded).collect();

    println!(
        "Referenced: {} | Uploaded: {} | Missing: {}",
        all_refs.len(),
        uploaded.len(),
        missing.len()
    );

    if !missing.is_empty() {
        eprintln!("\nMissing from uploaded.log:");
        for name in &missing {
            eprintln!("  - {name}");
        }
        panic!(
            "{} reference image(s) not in uploaded.log. \
             Run sync_and_verify_uploads with UPLOAD_REFERENCES=1 to fix.",
            missing.len()
        );
    }

    println!("All {} references accounted for in uploaded.log.", all_refs.len());
}
