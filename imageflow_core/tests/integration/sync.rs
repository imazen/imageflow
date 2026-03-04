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
