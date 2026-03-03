//! One-shot migration: convert legacy `checksums.json` entries to per-test TOML files.
//!
//! Run with: `cargo test -p imageflow_core --test migrate_checksums -- --nocapture`
//!
//! Safe to re-run: skips tests that already have a TOML file.

use std::collections::BTreeMap;
use std::path::Path;

use zensim_regress::checksum_file::{ChecksumEntry, TestChecksumFile};

#[test]
fn migrate_json_to_toml() {
    let visuals = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("visuals");

    let json_path = visuals.join("checksums.json");
    if !json_path.exists() {
        println!("No checksums.json found, nothing to migrate");
        return;
    }

    let json_text = std::fs::read_to_string(&json_path).unwrap();
    let entries: BTreeMap<String, String> = serde_json::from_str(&json_text).unwrap();

    let checksums_dir = visuals.join("checksums");
    std::fs::create_dir_all(&checksums_dir).unwrap();

    let mut migrated = 0;
    let mut skipped = 0;

    for (test_name, checksum_id) in &entries {
        let toml_path = zensim_regress::checksum_file::checksum_path(&checksums_dir, test_name);

        if toml_path.exists() {
            skipped += 1;
            continue;
        }

        let entry = ChecksumEntry {
            id: checksum_id.clone(),
            confidence: 100,
            commit: None,
            arch: vec![],
            reason: Some("migrated from checksums.json".to_string()),
            status: None,
            diff: None,
        };

        let file = TestChecksumFile {
            name: test_name.clone(),
            tolerance: Default::default(),
            checksum: vec![entry],
            info: None,
            meta: BTreeMap::new(),
        };

        file.write_to(&toml_path).unwrap();
        migrated += 1;
        println!("Migrated: {test_name} -> {}", toml_path.display());
    }

    println!("\n=== Migration complete ===");
    println!("Migrated: {migrated}");
    println!("Skipped (already exist): {skipped}");
    println!("Total JSON entries: {}", entries.len());
}
