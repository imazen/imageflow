//! TOML-first checksum lookup via `zensim-regress::TestChecksumFile`.
//!
//! Checks for a per-test `.toml` file before falling through to the
//! legacy JSON system. This enables incremental migration: migrated
//! tests use TOML, unmigrated tests fall through to JSON unchanged.

use std::path::{Path, PathBuf};

use zensim_regress::checksum_file::{checksum_path, TestChecksumFile};

use super::ChecksumMatch;

/// Adapter that checks TOML checksum files before falling through to JSON.
pub struct TomlChecksumAdapter {
    checksum_dir: PathBuf,
}

impl TomlChecksumAdapter {
    /// Create a new adapter pointing at the `checksums/` subdirectory
    /// within the given visuals directory.
    pub fn new(visuals_dir: &Path) -> Self {
        Self {
            checksum_dir: visuals_dir.join("checksums"),
        }
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
        let path = checksum_path(&self.checksum_dir, test_name);
        if !path.exists() {
            return None;
        }

        let file = match TestChecksumFile::read_from(&path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!(
                    "Warning: failed to parse TOML checksum file {}: {}",
                    path.display(),
                    e
                );
                return None;
            }
        };

        // Check if actual_hash matches any active entry
        if let Some(entry) = file.find_by_id(actual_hash) {
            if entry.is_active() {
                return Some((ChecksumMatch::Match, actual_hash.to_string()));
            }
        }

        // Hash not matched — report mismatch with authoritative reference
        let auth_id = file
            .authoritative()
            .map(|e| e.id.clone())
            .unwrap_or_default();
        Some((ChecksumMatch::Mismatch, auth_id))
    }
}
