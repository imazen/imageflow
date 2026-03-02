//! TOML-first checksum lookup via `zensim-regress::ChecksumManager`.
//!
//! Delegates to [`ChecksumManager::check_hash()`] for per-test `.toml` files,
//! falling through to the legacy JSON system when no TOML file exists.
//! Supports auto-accept of new checksums within imageflow's pixel tolerance.

use std::path::Path;

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
