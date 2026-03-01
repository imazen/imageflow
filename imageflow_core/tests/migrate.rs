//! Migration script: JSON checksums → per-test TOML files.
//!
//! Run with: `MIGRATE_CHECKSUMS=1 cargo test -p imageflow_core --test migrate -- --nocapture`

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use zensim_regress::checksum_file::{ChecksumEntry, TestChecksumFile, checksum_path};

#[test]
fn migrate_checksums_to_toml() {
    if std::env::var("MIGRATE_CHECKSUMS").as_deref() != Ok("1") {
        eprintln!("Skipping migration (set MIGRATE_CHECKSUMS=1 to run)");
        return;
    }

    let visuals_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("visuals");
    let checksums_path = visuals_dir.join("checksums.json");
    let alternates_path = visuals_dir.join("alternate_checksums.json");
    let output_dir = visuals_dir.join("checksums");

    // Read primary checksums: test_name → primary_hash
    let checksums_json: BTreeMap<String, String> = {
        let data = std::fs::read_to_string(&checksums_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", checksums_path.display(), e));
        serde_json::from_str(&data)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", checksums_path.display(), e))
    };

    // Read alternate checksums: primary_hash → [alternate_hashes]
    let alternates_json: BTreeMap<String, Vec<String>> = if alternates_path.exists() {
        let data = std::fs::read_to_string(&alternates_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", alternates_path.display(), e));
        serde_json::from_str(&data)
            .unwrap_or_else(|e| panic!("Failed to parse {}: {}", alternates_path.display(), e))
    } else {
        BTreeMap::new()
    };

    std::fs::create_dir_all(&output_dir).unwrap();

    let mut total_tests = 0;
    let mut total_alternates = 0;

    for (test_name, primary_hash) in &checksums_json {
        let mut file = TestChecksumFile::new(test_name.clone());

        // Primary entry
        let mut primary_entry = ChecksumEntry::new(primary_hash.clone());
        primary_entry.confidence = 10;
        primary_entry.reason = Some("migrated from checksums.json".to_string());
        file.checksum.push(primary_entry);

        // Alternate entries (keyed by primary hash in the alternates JSON)
        if let Some(alternates) = alternates_json.get(primary_hash) {
            for alt_hash in alternates {
                let mut alt_entry = ChecksumEntry::new(alt_hash.clone());
                alt_entry.confidence = 10;
                alt_entry.reason = Some("migrated alternate checksum".to_string());
                file.checksum.push(alt_entry);
                total_alternates += 1;
            }
        }

        // Store the S3 URL in meta for reference
        let s3_url = if !primary_hash.contains('.') {
            format!(
                "https://s3-us-west-2.amazonaws.com/imageflow-resources/visual_test_checksums/{}.png",
                primary_hash
            )
        } else {
            format!(
                "https://s3-us-west-2.amazonaws.com/imageflow-resources/visual_test_checksums/{}",
                primary_hash
            )
        };
        file.meta.insert(
            "s3_url".to_string(),
            zensim_regress::toml::Value::String(s3_url),
        );

        let toml_path = checksum_path(&output_dir, test_name);
        file.write_to(&toml_path)
            .unwrap_or_else(|e| panic!("Failed to write {}: {}", toml_path.display(), e));

        total_tests += 1;
    }

    eprintln!(
        "Migration complete: {} tests, {} alternate checksums",
        total_tests, total_alternates
    );
    eprintln!("Output directory: {}", output_dir.display());

    // Verify unique filenames
    let toml_files: Vec<_> = std::fs::read_dir(&output_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
        .collect();
    assert_eq!(
        toml_files.len(),
        total_tests,
        "File count mismatch — possible filename collision!"
    );

    // Verify all files parse back correctly
    let mut parse_errors = 0;
    for entry in &toml_files {
        match TestChecksumFile::read_from(&entry.path()) {
            Ok(parsed) => {
                assert!(
                    !parsed.checksum.is_empty(),
                    "Empty checksums in {}",
                    entry.path().display()
                );
            }
            Err(e) => {
                eprintln!("Parse error in {}: {}", entry.path().display(), e);
                parse_errors += 1;
            }
        }
    }
    assert_eq!(parse_errors, 0, "{} TOML files failed to parse", parse_errors);
}
