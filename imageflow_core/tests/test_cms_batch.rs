//! Batch CMS dual-backend test: decode images with CmsBackend::Both,
//! capturing moxcms vs lcms2 divergence.
//!
//! **Local development tool** — requires a local image corpus.
//! Not run on CI. Use `cargo test --test test_cms_batch -- --ignored --nocapture`.
//!
//! Configure paths via env vars:
//!   IMAGEFLOW_DEV_DIR  — base directory (default: /mnt/v on Linux, V:\ on Windows)

use imageflow_core::CmsBackend;
use imageflow_core::Context;
use imageflow_types as s;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

fn dev_dir() -> PathBuf {
    std::env::var("IMAGEFLOW_DEV_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(if cfg!(windows) { "V:\\" } else { "/mnt/v" }))
}

fn error_output_dir() -> PathBuf {
    dev_dir().join("output").join("cms-errors")
}

fn collect_image_files(dir: &Path) -> Vec<PathBuf> {
    if !dir.exists() {
        return Vec::new();
    }
    let mut files = Vec::new();
    collect_recursive(dir, &mut files);
    files.sort();
    files
}

fn collect_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_recursive(&path, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "jpg" | "jpeg" | "png" | "webp" | "gif" | "tiff" | "tif" => {
                    out.push(path);
                }
                _ => {}
            }
        }
    }
}

struct BatchResult {
    total: u64,
    ok: u64,
    errors: Vec<(PathBuf, String)>,
    elapsed: std::time::Duration,
}

fn process_file(path: &Path) -> Result<(), String> {
    std::panic::catch_unwind(|| process_file_inner(path)).unwrap_or_else(|e| {
        let msg = if let Some(s) = e.downcast_ref::<String>() {
            s.clone()
        } else if let Some(s) = e.downcast_ref::<&str>() {
            s.to_string()
        } else {
            "unknown panic".to_string()
        };
        Err(format!("PANIC: {msg}"))
    })
}

fn process_file_inner(path: &Path) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read error: {e}"))?;

    let mut ctx = Context::create().map_err(|e| format!("context: {e}"))?;
    ctx.cms_backend = CmsBackend::Both;

    ctx.add_input_vector(0, bytes).map_err(|e| format!("add input: {e}"))?;
    ctx.add_output_buffer(1).map_err(|e| format!("add output: {e}"))?;

    let execute = s::Execute001 {
        graph_recording: Some(s::Build001GraphRecording::off()),
        security: None,
        framewise: s::Framewise::Steps(vec![
            s::Node::Decode { io_id: 0, commands: None },
            s::Node::Constrain(s::Constraint {
                mode: s::ConstraintMode::Within,
                w: Some(256),
                h: Some(256),
                hints: None,
                gravity: None,
                canvas_color: None,
            }),
            s::Node::Encode {
                io_id: 1,
                preset: s::EncoderPreset::Libpng {
                    depth: Some(s::PngBitDepth::Png24),
                    matte: None,
                    zlib_compression: None,
                },
            },
        ]),
    };

    ctx.execute_1(execute).map_err(|e| format!("{e}"))?;

    Ok(())
}

fn run_batch(label: &str, dir: &Path) -> Option<BatchResult> {
    let files = collect_image_files(dir);
    if files.is_empty() {
        eprintln!("[{label}] Directory not found or empty: {}", dir.display());
        return None;
    }

    eprintln!("[{label}] Processing {} files from {}", files.len(), dir.display());
    let start = Instant::now();

    let ok_count = AtomicU64::new(0);
    let errors: Mutex<Vec<(PathBuf, String)>> = Mutex::new(Vec::new());

    // Process sequentially — Context isn't Send
    let mut error_counts: std::collections::BTreeMap<String, u64> =
        std::collections::BTreeMap::new();
    for (i, path) in files.iter().enumerate() {
        match process_file(path) {
            Ok(()) => {
                ok_count.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => {
                let category = e.lines().next().unwrap_or(&e).to_string();
                *error_counts.entry(category).or_default() += 1;
                errors.lock().unwrap().push((path.clone(), e));
            }
        }
        if (i + 1) % 500 == 0 {
            eprintln!(
                "[{label}] {}/{} done ({:.0}/s)",
                i + 1,
                files.len(),
                (i + 1) as f64 / start.elapsed().as_secs_f64()
            );
        }
    }

    let elapsed = start.elapsed();
    let errors = errors.into_inner().unwrap();
    let ok = ok_count.load(Ordering::Relaxed);

    for (cat, count) in &error_counts {
        eprintln!("[{label}]   {count}x {cat}");
    }

    eprintln!(
        "[{label}] Done: {ok}/{} ok, {} errors in {:.1}s",
        files.len(),
        errors.len(),
        elapsed.as_secs_f64()
    );

    Some(BatchResult { total: files.len() as u64, ok, errors, elapsed })
}

fn collect_error_files(label: &str, result: &BatchResult, error_dir: &Path) {
    if result.errors.is_empty() {
        return;
    }

    let out_dir = error_dir.join(label);
    std::fs::create_dir_all(&out_dir).unwrap();

    let manifest_path = out_dir.join("errors.tsv");
    let mut manifest = std::fs::File::create(&manifest_path).unwrap();
    writeln!(manifest, "file\terror_category\tfull_error").unwrap();

    for (path, error) in &result.errors {
        let category = error.lines().next().unwrap_or(error);
        let cat_dir_name = category
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .take(60)
            .collect::<String>();

        let cat_dir = out_dir.join(&cat_dir_name);
        std::fs::create_dir_all(&cat_dir).unwrap();

        let file_name = path.file_name().unwrap_or_default();
        let dest = cat_dir.join(file_name);
        if let Err(e) = std::fs::copy(path, &dest) {
            eprintln!("[{label}] Failed to copy {}: {e}", path.display());
        }

        let error_oneline = error.replace('\n', " | ").replace('\t', " ");
        writeln!(manifest, "{}\t{}\t{}", path.display(), cat_dir_name, error_oneline).unwrap();
    }

    eprintln!(
        "[{label}] Wrote {} error files to {} and manifest to {}",
        result.errors.len(),
        out_dir.display(),
        manifest_path.display()
    );
}

#[test]
#[ignore]
fn cms_batch_collect_errors() {
    let base = dev_dir();
    let corpora: Vec<(&str, PathBuf)> = vec![
        ("jpeg", base.join("datasets/scraping/jpeg")),
        ("non-srgb", base.join("datasets/non-srgb-by-profile")),
        ("wide-gamut", base.join("output/corpus-builder/wide-gamut")),
        ("png-24-32", base.join("output/corpus-builder/png-24-32")),
    ];

    let mut results: Vec<(&str, BatchResult)> = Vec::new();
    for (label, dir) in &corpora {
        if !dir.exists() {
            eprintln!("Skipping: {} not found", dir.display());
            continue;
        }
        if let Some(result) = run_batch(label, dir) {
            results.push((label, result));
        }
    }

    let total_files: u64 = results.iter().map(|(_, r)| r.total).sum();
    let total_ok: u64 = results.iter().map(|(_, r)| r.ok).sum();
    let total_errors: usize = results.iter().map(|(_, r)| r.errors.len()).sum();

    if total_files == 0 {
        eprintln!("No corpus directories found or all empty, skipping batch test");
        return;
    }

    let error_dir = error_output_dir();
    if error_dir.exists() {
        std::fs::remove_dir_all(&error_dir).unwrap();
    }
    std::fs::create_dir_all(&error_dir).unwrap();

    for (label, result) in &results {
        collect_error_files(label, result, &error_dir);
    }

    // Write summary
    let summary_path = error_dir.join("summary.txt");
    let mut summary = std::fs::File::create(&summary_path).unwrap();
    writeln!(summary, "CMS Batch Dual-Backend Test Summary").unwrap();
    writeln!(summary, "Total files: {total_files}").unwrap();
    writeln!(summary, "OK: {total_ok}").unwrap();
    writeln!(summary, "Errors: {total_errors}").unwrap();
    writeln!(summary, "Pass rate: {:.1}%", total_ok as f64 / total_files as f64 * 100.0).unwrap();

    eprintln!("Overall: {total_ok}/{total_files} ok, {total_errors} errors");
    eprintln!("Error files and manifests in {}", error_dir.display());

    assert!(
        total_errors < total_files as usize / 10,
        "Too many errors: {total_errors}/{total_files}"
    );
}
