//! Compare each CMS error file against moxcms and lcms2 independently.
//!
//! **Local development tool** — requires output from cms_batch_collect_errors.
//! Not run on CI. Use `cargo test --test test_cms_compare_backends -- --ignored --nocapture`.
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

fn error_output_dir() -> PathBuf {
    dev_dir().join("output").join("cms-errors")
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

fn collect_error_files() -> Vec<(PathBuf, String)> {
    let base = error_output_dir();
    let mut files = Vec::new();
    for corpus in &["jpeg", "non-srgb", "wide-gamut", "png-24-32"] {
        let tsv = base.join(corpus).join("errors.tsv");
        if !tsv.exists() {
            continue;
        }
        let contents = std::fs::read_to_string(&tsv).unwrap();
        for line in contents.lines().skip(1) {
            let parts: Vec<&str> = line.splitn(3, '\t').collect();
            if parts.len() >= 2 {
                let path = PathBuf::from(parts[0]);
                let category = parts[1].to_string();
                if path.exists() {
                    files.push((path, category));
                }
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files.dedup_by(|a, b| a.0 == b.0);
    files
}

#[test]
#[ignore]
fn compare_backends_on_error_files() {
    let files = collect_error_files();
    if files.is_empty() {
        eprintln!("No error files found — run cms_batch_collect_errors first");
        return;
    }

    let out_path = error_output_dir().join("backend-comparison.tsv");
    let mut out = std::fs::File::create(&out_path).unwrap();
    writeln!(out, "file\tcategory\tmoxcms\tlcms2").unwrap();

    let mut moxcms_only_fail = 0u32;
    let mut lcms2_only_fail = 0u32;
    let mut both_fail = 0u32;
    let mut neither_fail = 0u32;

    for (path, category) in &files {
        if category.contains("SizeLimitExceeded")
            || category.contains("IDAT")
            || category.contains("ObjectCreationError")
        {
            continue;
        }

        let moxcms_result = try_decode(path, CmsBackend::Moxcms);
        let lcms2_result = try_decode(path, CmsBackend::Lcms2);

        let m_status = match &moxcms_result {
            Ok(()) => "ok".to_string(),
            Err(e) => e.lines().next().unwrap_or("error").to_string(),
        };
        let l_status = match &lcms2_result {
            Ok(()) => "ok".to_string(),
            Err(e) => e.lines().next().unwrap_or("error").to_string(),
        };

        let fname = path.file_name().unwrap_or_default().to_string_lossy();
        writeln!(out, "{}\t{}\t{}\t{}", fname, category, m_status, l_status).unwrap();

        match (moxcms_result.is_ok(), lcms2_result.is_ok()) {
            (false, true) => moxcms_only_fail += 1,
            (true, false) => lcms2_only_fail += 1,
            (false, false) => both_fail += 1,
            (true, true) => neither_fail += 1,
        }
    }

    eprintln!("Results written to {}", out_path.display());
    eprintln!("moxcms fails, lcms2 ok: {moxcms_only_fail}");
    eprintln!("lcms2 fails, moxcms ok: {lcms2_only_fail}");
    eprintln!("both fail: {both_fail}");
    eprintln!("both ok: {neither_fail}");
}
