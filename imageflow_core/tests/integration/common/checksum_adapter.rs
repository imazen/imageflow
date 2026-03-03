//! Checksum lookup with v2 (.checksums) + TOML + JSON fallback chain.
//!
//! Three layers, tried in order:
//! 1. `.checksums` v1 files via [`ChecksumManagerV2`] (one file per module)
//! 2. Per-test `.toml` files via [`ChecksumManager`] (legacy)
//! 3. Global `checksums.json` (oldest legacy, handled by caller)

use std::path::Path;

use zensim_regress::checksums_v2::{CheckResultV2, ChecksumManagerV2};
use zensim_regress::manager::{CheckResult, ChecksumManager};
use zensim_regress::RegressError;

use super::ChecksumMatch;

/// Adapter that checks TOML checksum files via `ChecksumManager`.
///
/// The manager reads `UPDATE_CHECKSUMS` and `REPLACE_CHECKSUMS` env vars,
/// so `check_hash()` will auto-write TOML for new baselines in those modes.
pub struct TomlChecksumAdapter {
    manager: ChecksumManager,
}

impl TomlChecksumAdapter {
    /// Create a new adapter pointing at the `checksums/` subdirectory
    /// within the given visuals directory.
    pub fn new(visuals_dir: &Path) -> Self {
        let checksum_dir = visuals_dir.join("checksums");
        let manager = ChecksumManager::new(&checksum_dir);
        Self { manager }
    }

    /// Try to match `actual_hash` against the TOML file for `test_name`.
    ///
    /// Returns `None` if no TOML file exists (caller should fall through
    /// to JSON). Returns `Some((match_result, authoritative_id))` if a
    /// TOML file was found and consulted.
    pub fn try_match(
        &self,
        test_name: &str,
        actual_hash: &str,
    ) -> Option<(ChecksumMatch, String)> {
        if !self.manager.test_path(test_name).exists() {
            return None;
        }

        match self.manager.check_hash(test_name, actual_hash) {
            Ok(CheckResult::Match { entry_id, .. }) => {
                Some((ChecksumMatch::Match, entry_id))
            }
            Ok(CheckResult::NoBaseline { actual_hash, auto_accepted: true }) => {
                Some((ChecksumMatch::NewStored, actual_hash))
            }
            Ok(CheckResult::NoBaseline { .. }) => {
                Some((ChecksumMatch::Mismatch, String::new()))
            }
            Ok(CheckResult::WithinTolerance { authoritative_id, .. }) => {
                // check_hash() can't produce WithinTolerance (no pixels),
                // but handle it for completeness
                Some((ChecksumMatch::Match, authoritative_id))
            }
            Ok(CheckResult::Failed { authoritative_id, .. }) => {
                Some((ChecksumMatch::Mismatch, authoritative_id.unwrap_or_default()))
            }
            Err(e) => {
                eprintln!("Warning: ChecksumManager error for '{test_name}': {e}");
                None
            }
        }
    }

    /// Accept a new checksum for a test, recording it in the TOML file
    /// with chain-of-trust evidence.
    ///
    /// This is unconditional — the caller is responsible for gating on
    /// `UPDATE_CHECKSUMS=1` or other policy.
    pub fn accept(
        &self,
        test_name: &str,
        actual_hash: &str,
    ) -> Result<(), RegressError> {
        self.manager.accept(
            test_name,
            actual_hash,
            None,
            "auto-accepted within imageflow tolerance",
        )
    }

    /// Whether a TOML checksum file exists for the given test.
    pub fn has_toml(&self, test_name: &str) -> bool {
        self.manager.test_path(test_name).exists()
    }
}

// ─── V2 adapter (.checksums format) ─────────────────────────────────────

/// Adapter for `.checksums` v1 format files via `ChecksumManagerV2`.
///
/// Uses structured (module, test_name, detail_name) keys instead of flat
/// strings. One `.checksums` file per test module.
pub struct V2ChecksumAdapter {
    manager: ChecksumManagerV2,
}

impl V2ChecksumAdapter {
    /// Create a new adapter pointing at the `checksums/` subdirectory
    /// within the given visuals directory.
    pub fn new(visuals_dir: &Path) -> Self {
        let checksum_dir = visuals_dir.join("checksums");
        let manager = ChecksumManagerV2::new(&checksum_dir);
        Self { manager }
    }

    /// Try to match `actual_hash` against the `.checksums` file for the module.
    ///
    /// Returns `None` if no `.checksums` file exists (caller should fall through
    /// to TOML/JSON). Returns `Some((match_result, name))` if consulted.
    pub fn try_match(
        &self,
        module: &str,
        test_name: &str,
        detail_name: &str,
        actual_hash: &str,
    ) -> Option<(ChecksumMatch, String)> {
        if !self.manager.has_module(module) {
            return None;
        }

        match self.manager.check_hash(module, test_name, detail_name, actual_hash) {
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

    /// Whether a `.checksums` file exists for the given module.
    pub fn has_module(&self, module: &str) -> bool {
        self.manager.has_module(module)
    }
}
