//! Build inventory of all corpus-builder files that fail CMS transforms,
//! noting which backend(s) fail.
//!
//! **Local development tool** — requires a local image corpus.
//! Not run on CI. Use `cargo test --release --test test_cms_inventory -- --ignored --nocapture`.
//!
//! Configure paths via env vars:
//!   IMAGEFLOW_DEV_DIR  — base directory (default: /mnt/v on Linux, V:\ on Windows)

use imageflow_core::CmsBackend;
use imageflow_core::Context;
use imageflow_types as s;
use std::io::Write;
use std::path::{Path, PathBuf};

fn dev_dir() -> PathBuf {
    std::env::var("IMAGEFLOW_DEV_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(if cfg!(windows) { "V:\\" } else { "/mnt/v" }))
}

fn collect_image_files(dir: &Path) -> Vec<PathBuf> {
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
                "jpg" | "jpeg" | "png" => out.push(path),
                _ => {}
            }
        }
    }
}

fn try_decode(path: &Path, backend: CmsBackend) -> Result<(), String> {
    std::panic::catch_unwind(|| try_decode_inner(path, backend)).unwrap_or_else(|e| {
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

fn try_decode_inner(path: &Path, backend: CmsBackend) -> Result<(), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read: {e}"))?;
    let mut ctx = Context::create().map_err(|e| format!("context: {e}"))?;
    ctx.cms_backend = backend;

    ctx.add_input_vector(0, bytes).map_err(|e| format!("input: {e}"))?;
    ctx.add_output_buffer(1).map_err(|e| format!("output: {e}"))?;

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

fn error_oneline(e: &str) -> String {
    e.lines().next().unwrap_or(e).to_string()
}

#[test]
#[ignore]
fn corpus_builder_cms_inventory() {
    let base = dev_dir().join("output").join("corpus-builder");
    if !base.exists() {
        eprintln!("Skipping: {} not found", base.display());
        return;
    }

    let files = collect_image_files(&base);
    eprintln!("Found {} JPEG/PNG files in {}", files.len(), base.display());

    let inventory_path = dev_dir().join("output/cms-errors/corpus-builder-inventory.tsv");
    let out_dir = inventory_path.parent().unwrap();
    std::fs::create_dir_all(out_dir).unwrap();

    let mut inventory = std::fs::File::create(&inventory_path).unwrap();
    writeln!(inventory, "file\tsubdir\tmoxcms\tlcms2").unwrap();

    let mut total = 0u64;
    let mut moxcms_ok = 0u64;
    let mut moxcms_fail = 0u64;
    let mut moxcms_only_fail = 0u64;
    let mut lcms2_only_fail = 0u64;
    let mut both_fail = 0u64;

    let start = std::time::Instant::now();

    for (i, path) in files.iter().enumerate() {
        total += 1;

        let rel = path.strip_prefix(&base).unwrap_or(path);
        let subdir = rel.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();

        let moxcms_result = try_decode(path, CmsBackend::Moxcms);

        if moxcms_result.is_ok() {
            moxcms_ok += 1;
            if total % 100 == 0 {
                let lcms2_result = try_decode(path, CmsBackend::Lcms2);
                if lcms2_result.is_err() {
                    let l_err = error_oneline(&lcms2_result.unwrap_err());
                    let fname = path.file_name().unwrap_or_default().to_string_lossy();
                    writeln!(inventory, "{}\t{}\tok\t{}", fname, subdir, l_err).unwrap();
                    lcms2_only_fail += 1;
                }
            }
        } else {
            moxcms_fail += 1;
            let m_err = error_oneline(&moxcms_result.unwrap_err());

            let lcms2_result = try_decode(path, CmsBackend::Lcms2);
            let l_status = match &lcms2_result {
                Ok(()) => "ok".to_string(),
                Err(e) => error_oneline(e),
            };

            let fname = path.file_name().unwrap_or_default().to_string_lossy();
            writeln!(inventory, "{}\t{}\t{}\t{}", fname, subdir, m_err, l_status).unwrap();

            match lcms2_result.is_ok() {
                true => moxcms_only_fail += 1,
                false => both_fail += 1,
            }
        }

        if (i + 1) % 1000 == 0 {
            let elapsed = start.elapsed().as_secs_f64();
            eprintln!(
                "{}/{} ({:.0}/s) moxcms_ok={} moxcms_fail={} [moxcms_only={} lcms2_only={} both={}]",
                i + 1,
                files.len(),
                (i + 1) as f64 / elapsed,
                moxcms_ok,
                moxcms_fail,
                moxcms_only_fail,
                lcms2_only_fail,
                both_fail,
            );
            inventory.flush().unwrap();
        }
    }

    inventory.flush().unwrap();

    let elapsed = start.elapsed();
    eprintln!("\n=== Corpus Builder CMS Inventory ===");
    eprintln!("Total files: {total}");
    eprintln!("moxcms ok: {moxcms_ok}");
    eprintln!("moxcms fail: {moxcms_fail}");
    eprintln!("  moxcms-only fail: {moxcms_only_fail}");
    eprintln!("  lcms2-only fail: {lcms2_only_fail}");
    eprintln!("  both fail: {both_fail}");
    eprintln!(
        "Elapsed: {:.1}s ({:.0} files/s)",
        elapsed.as_secs_f64(),
        total as f64 / elapsed.as_secs_f64()
    );
    eprintln!("Inventory written to {}", inventory_path.display());
}
