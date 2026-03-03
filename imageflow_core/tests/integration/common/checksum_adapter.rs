//! Checksum lookup via `.checksums` v1 files.

use std::path::Path;

use zensim_regress::checksums_v2::{CheckResultV2, ChecksumManagerV2};
use zensim_regress::RegressError;

use super::ChecksumMatch;

/// Adapter for `.checksums` v1 format files via `ChecksumManagerV2`.
///
/// Uses structured (module, test_name, detail_name) keys.
/// One `.checksums` file per test module.
pub struct ChecksumAdapter {
    manager: ChecksumManagerV2,
}

impl ChecksumAdapter {
    /// Create a new adapter pointing at the given checksums directory.
    pub fn new(checksums_dir: &Path) -> Self {
        let manager = ChecksumManagerV2::new(checksums_dir);
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
            Ok(CheckResultV2::NoBaseline { actual_name, auto_accepted: true }) => {
                Some((ChecksumMatch::NewStored, actual_name))
            }
            Ok(CheckResultV2::NoBaseline { .. }) => {
                Some((ChecksumMatch::Mismatch, String::new()))
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
