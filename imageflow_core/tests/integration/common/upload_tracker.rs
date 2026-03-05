//! Upload tracking and sync verification for visual test reference images.
//!
//! Maintains `uploaded.log` — a sorted list of petnames that have been
//! successfully uploaded to S3. Provides sync and verification functions
//! for CI pipelines.
//!
//! # CI workflow
//!
//! 1. Run tests (may auto-accept new arch variants → .checksums modified)
//! 2. Run `sync_and_verify()` with `UPLOAD_REFERENCES=1`
//!    - Uploads missing images to S3
//!    - Uploads .checksums files to S3
//!    - Updates uploaded.log
//!    - **Fails** if any referenced image can't be uploaded
//! 3. Commit uploaded.log + .checksums changes
//! 4. Open PR if .checksums changed

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use zensim_regress::checksums::ChecksumsFile;
use zensim_regress::remote::ReferenceStorage;
use zensim_regress::upload::ResourceUploader;

/// Path to the uploaded.log file (committed to git).
fn uploaded_log_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/integration/visuals/uploaded.log")
}

/// Read the set of already-uploaded petnames from uploaded.log.
pub fn read_uploaded_log() -> BTreeSet<String> {
    let path = uploaded_log_path();
    if !path.exists() {
        return BTreeSet::new();
    }
    std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|s| s.trim().to_string())
        .collect()
}

/// Write the uploaded log (sorted, one petname per line).
pub fn write_uploaded_log(uploaded: &BTreeSet<String>) {
    let path = uploaded_log_path();
    let mut content = String::new();
    for name in uploaded {
        content.push_str(name);
        content.push('\n');
    }
    std::fs::write(&path, content).unwrap();
}

/// Extract all active (non-retired) petnames from all .checksums files.
pub fn all_referenced_petnames(checksums_dir: &Path) -> BTreeSet<String> {
    petname_to_local_paths(checksums_dir).into_keys().collect()
}

/// Build a mapping from petname → local reference image path.
///
/// Scans all `.checksums` files, extracts active entries, and resolves
/// each petname to its reference image at `{checksums_dir}/images/{module}/{test}_{detail}.png`.
/// Only includes entries whose reference image exists locally.
pub fn petname_to_local_paths(checksums_dir: &Path) -> BTreeMap<String, PathBuf> {
    let mut map = BTreeMap::new();
    let images_dir = checksums_dir.join("images");
    for entry in std::fs::read_dir(checksums_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "checksums") {
            let module = path.file_stem().unwrap().to_str().unwrap();
            match ChecksumsFile::read_from(&path) {
                Ok(file) => {
                    for section in &file.sections {
                        let flat_name = if section.detail_name.is_empty() {
                            section.test_name.clone()
                        } else {
                            format!("{}_{}", section.test_name, section.detail_name)
                        };
                        let ref_path = images_dir.join(module).join(format!("{flat_name}.png"));
                        for entry in section.active_entries() {
                            map.insert(entry.name_hash.clone(), ref_path.clone());
                        }
                    }
                }
                Err(e) => eprintln!("Warning: failed to read {}: {e}", path.display()),
            }
        }
    }
    map
}

/// Sync result from a sync_and_verify run.
pub struct SyncResult {
    /// Total petnames referenced in .checksums files.
    pub total_referenced: usize,
    /// Petnames already in uploaded.log before sync.
    pub already_uploaded: usize,
    /// Petnames newly uploaded during this sync.
    pub newly_uploaded: usize,
    /// Petnames that failed to upload (missing locally or upload error).
    pub failed: Vec<String>,
}

/// Sync all reference images to S3 and verify completeness.
///
/// 1. Scans all .checksums files for active petnames
/// 2. Resolves each petname to its reference image via the checksums mapping
/// 3. For each petname not in uploaded.log:
///    a. Checks if the reference image exists locally
///    b. Uploads to S3 via the provided storage
///    c. Records success in uploaded.log
/// 4. Uploads .checksums files to S3 under `checksums/` prefix
/// 5. Updates uploaded.log on disk
/// 6. Returns result with any failures
///
/// **Does not fail tests** — caller decides whether to panic on failures.
pub fn sync_and_verify(checksums_dir: &Path, storage: &ReferenceStorage) -> SyncResult {
    let petname_paths = petname_to_local_paths(checksums_dir);
    let mut uploaded = read_uploaded_log();
    let already_uploaded = uploaded.len();
    let mut newly_uploaded = 0;
    let mut failed = Vec::new();

    let can_upload = storage.uploads_configured();

    for (petname, local_path) in &petname_paths {
        if uploaded.contains(petname) {
            continue;
        }

        if !local_path.exists() {
            eprintln!("MISSING locally: {petname} (expected at {})", local_path.display());
            failed.push(petname.clone());
            continue;
        }

        if !can_upload {
            failed.push(petname.clone());
            continue;
        }

        match storage.upload_reference(local_path, petname) {
            Ok(()) => {
                println!("Uploaded: {petname}");
                uploaded.insert(petname.clone());
                newly_uploaded += 1;
            }
            Err(e) => {
                eprintln!("UPLOAD FAILED: {petname}: {e}");
                failed.push(petname.clone());
            }
        }
    }

    // Upload .checksums files to S3
    sync_checksums_files(checksums_dir, storage);

    // Persist the updated log
    write_uploaded_log(&uploaded);

    SyncResult { total_referenced: petname_paths.len(), already_uploaded, newly_uploaded, failed }
}

/// Upload .checksums files to S3 under a `checksums/` path.
///
/// Uses the uploader from the provided storage directly (not via
/// `remote_filename`, which appends `.png`).
fn sync_checksums_files(checksums_dir: &Path, storage: &ReferenceStorage) {
    let Some(prefix) = storage.upload_prefix() else {
        return;
    };
    if !storage.uploads_configured() {
        return;
    }

    let uploader = zensim_regress::upload::ShellUploader::new();
    for entry in std::fs::read_dir(checksums_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "checksums") {
            let filename = path.file_name().unwrap().to_str().unwrap();
            let remote_url = format!("{}/checksums/{}", prefix.trim_end_matches('/'), filename);
            match uploader.upload(&path, &remote_url) {
                Ok(()) => println!("Uploaded checksums: {filename}"),
                Err(e) => eprintln!("Warning: failed to upload {filename}: {e}"),
            }
        }
    }
}
