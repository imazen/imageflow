//! Checksum lookup via `.checksums` v1 files.

use std::path::Path;

use zensim_regress::checksums_v2::{CheckResultV2, ChecksumManagerV2};
use zensim_regress::remote::ReferenceStorage;
use zensim_regress::RegressError;

use super::ChecksumMatch;

/// Default S3 base URL for downloading reference images.
const DEFAULT_REFERENCE_URL: &str =
    "https://s3-us-west-2.amazonaws.com/imageflow-resources/visual_test_checksums";
/// Default S3 upload prefix for new reference images.
const DEFAULT_UPLOAD_PREFIX: &str =
    "s3://imageflow-resources/visual_test_checksums";

/// Adapter for `.checksums` v1 format files via `ChecksumManagerV2`.
///
/// Uses structured (module, test_name, detail_name) keys.
/// One `.checksums` file per test module.
pub struct ChecksumAdapter {
    manager: ChecksumManagerV2,
}

impl ChecksumAdapter {
    /// Create a new adapter pointing at the given checksums directory.
    ///
    /// Configures remote S3 storage with hardcoded defaults for imageflow.
    /// `REGRESS_REFERENCE_URL` env var overrides the download URL.
    /// `REGRESS_UPLOAD_PREFIX` env var overrides the upload prefix.
    /// Uploads require `UPLOAD_REFERENCES=1`.
    pub fn new(checksums_dir: &Path) -> Self {
        let cache_dir = checksums_dir.join(".remote-cache");
        let download_url = std::env::var("REGRESS_REFERENCE_URL")
            .ok()
            .and_then(|v| if v.is_empty() { None } else { Some(v) })
            .unwrap_or_else(|| DEFAULT_REFERENCE_URL.to_string());
        let upload_prefix = std::env::var("REGRESS_UPLOAD_PREFIX")
            .ok()
            .and_then(|v| if v.is_empty() { None } else { Some(v) })
            .or_else(|| Some(DEFAULT_UPLOAD_PREFIX.to_string()));
        let upload_enabled = std::env::var("UPLOAD_REFERENCES")
            .is_ok_and(|v| v == "1" || v == "true");
        let remote = ReferenceStorage::new(download_url, upload_prefix, upload_enabled, cache_dir);
        let manager = ChecksumManagerV2::new(checksums_dir)
            .with_remote_storage(remote);
        Self { manager }
    }

    /// Try to match `actual_hash` against the `.checksums` file for the module.
    ///
    /// Returns `None` if no `.checksums` file exists.
    /// Returns `Some((match_result, name))` if consulted.
    pub fn try_match(
        &self,
        module: &str,
        test_name: &str,
        detail_name: &str,
        actual_hash: &str,
        tolerance: Option<&zensim_regress::Tolerance>,
    ) -> Option<(ChecksumMatch, String)> {
        if !self.manager.has_module(module) {
            return None;
        }

        match self.manager.check_hash(module, test_name, detail_name, actual_hash, tolerance) {
            Ok(CheckResultV2::Match { entry_name }) => {
                Some((ChecksumMatch::Match, entry_name))
            }
            Ok(CheckResultV2::NoBaseline { actual_name, auto_accepted: true, .. }) => {
                Some((ChecksumMatch::NewStored, actual_name))
            }
            Ok(CheckResultV2::NoBaseline { .. }) => {
                Some((ChecksumMatch::Mismatch, String::new()))
            }
            Ok(CheckResultV2::WithinTolerance { actual_name, .. }) => {
                Some((ChecksumMatch::Match, actual_name))
            }
            Ok(CheckResultV2::Failed { authoritative_name, .. }) => {
                Some((ChecksumMatch::Mismatch, authoritative_name))
            }
            Err(e) => {
                eprintln!(
                    "Warning: ChecksumManagerV2 error for '{module}/{test_name} {detail_name}': {e}"
                );
                None
            }
        }
    }

    /// Accept a new checksum with chain-of-trust evidence.
    pub fn accept(
        &self,
        module: &str,
        test_name: &str,
        detail_name: &str,
        actual_hash: &str,
        vs_ref_hash: Option<&str>,
        tolerance_note: Option<&str>,
        diff_summary: Option<&str>,
    ) -> Result<(), RegressError> {
        self.manager.accept(
            module,
            test_name,
            detail_name,
            actual_hash,
            vs_ref_hash,
            tolerance_note,
            diff_summary,
            "auto-accepted within imageflow tolerance",
        )
    }
}
