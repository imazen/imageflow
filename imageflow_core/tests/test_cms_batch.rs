//! Batch CMS dual-backend test: decode images with CmsBackend::Both,
//! capturing moxcms vs lcms2 divergence warnings.
//!
//! Run: cargo test --test test_cms_batch -- --nocapture
//!
//! Directories are hardcoded to V:\ datasets. Skipped if not present.

use imageflow_core::CmsBackend;
use imageflow_core::Context;
use imageflow_types as s;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

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
    _ok: u64,
    errors: Vec<(PathBuf, String)>,
    _elapsed: std::time::Duration,
}

fn process_file(path: &Path) -> Result<Vec<String>, String> {
    // Catch panics (e.g. lcms2 assertion failures on gray ICC profiles)
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

fn process_file_inner(path: &Path) -> Result<Vec<String>, String> {
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
            // Small constrain to keep memory reasonable
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

    Ok(Vec::new())
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
            Ok(_) => {
                ok_count.fetch_add(1, Ordering::Relaxed);
            }
            Err(e) => {
                // Categorize error by first line
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

    // Print error summary by category
    for (cat, count) in &error_counts {
        eprintln!("[{label}]   {count}x {cat}");
    }

    eprintln!(
        "[{label}] Done: {ok}/{} ok, {} errors in {:.1}s",
        files.len(),
        errors.len(),
        elapsed.as_secs_f64()
    );

    Some(BatchResult { total: files.len() as u64, _ok: ok, errors, _elapsed: elapsed })
}

#[test]
fn cms_batch_jpeg_scraping() {
    let dir = Path::new("/mnt/v/datasets/scraping/jpeg");
    if !dir.exists() {
        eprintln!("Skipping: {} not found", dir.display());
        return;
    }
    let result = run_batch("jpeg", dir).unwrap();
    assert!(
        result.errors.len() < result.total as usize / 10,
        "Too many errors: {}/{}",
        result.errors.len(),
        result.total
    );
}

#[test]
fn cms_batch_non_srgb() {
    let dir = Path::new("/mnt/v/datasets/non-srgb-by-profile");
    if !dir.exists() {
        eprintln!("Skipping: {} not found", dir.display());
        return;
    }
    let result = run_batch("non-srgb", dir).unwrap();
    assert!(
        result.errors.len() < result.total as usize / 10,
        "Too many errors: {}/{}",
        result.errors.len(),
        result.total
    );
}

#[test]
fn cms_batch_wide_gamut() {
    let dir = Path::new("/mnt/v/output/corpus-builder/wide-gamut");
    if !dir.exists() {
        eprintln!("Skipping: {} not found", dir.display());
        return;
    }
    let result = run_batch("wide-gamut", dir).unwrap();
    assert!(
        result.errors.len() < result.total as usize / 10,
        "Too many errors: {}/{}",
        result.errors.len(),
        result.total
    );
}

#[test]
fn cms_batch_png_24_32() {
    let dir = Path::new("/mnt/v/output/corpus-builder/png-24-32");
    if !dir.exists() {
        eprintln!("Skipping: {} not found", dir.display());
        return;
    }
    let result = run_batch("png-24-32", dir).unwrap();
    assert!(
        result.errors.len() < result.total as usize / 10,
        "Too many errors: {}/{}",
        result.errors.len(),
        result.total
    );
}
