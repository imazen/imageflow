//! Modern v2 CLI subcommands (`bulk`, `gallery init|build|serve`, `v2/capabilities`, `v2/compare`).

use crate::bulk::engine::{BulkProgressEvent, plan_bulk_dry_run, run_bulk, write_file_atomic};
use crate::bulk::schema::{
    AspectRatio, BULK_SCHEMA_VERSION, BulkConfig, BulkLimits, EncodeEffort, FailurePolicy, FitMode,
    GalleryConfig, GalleryLayout, GalleryTheme, OutputFormat, OutputPathLayout,
    QualityFallbackPolicy, QualityTarget, ResponsivePreset, WorkspaceAdmission,
};
use crate::gallery::{
    build_gallery, init_gallery_config,
    server::{DesignerServerOptions, serve_gallery_designer},
};
use clap::{Arg, ArgAction, ArgMatches, Command, ValueHint};
use std::net::IpAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

pub fn register_modern_subcommands(app: Command) -> Command {
    app.subcommand(
        Command::new("bulk")
            .about("Bulk responsive image/video processor with single-decode multi-format fan-out and zensim_a quality dial")
            .arg(
                Arg::new("inputs")
                    .value_name("INPUT")
                    .num_args(1..)
                    .required_unless_present("config")
                    .value_hint(ValueHint::AnyPath)
                    .help("Input files or directories to process"),
            )
            .arg(
                Arg::new("out")
                    .long("out")
                    .short('o')
                    .num_args(1)
                    .value_parser(clap::value_parser!(PathBuf))
                    .value_hint(ValueHint::DirPath)
                    .required_unless_present("config")
                    .help("Output directory for responsive variants, manifest.json, and snippets.html"),
            )
            .arg(
                Arg::new("config")
                    .long("config")
                    .short('c')
                    .num_args(1)
                    .value_parser(clap::value_parser!(PathBuf))
                    .help("Declarative bulk preset config file (.toml or .json, schema imageflow-bulk-v1)"),
            )
            .arg(
                Arg::new("preset")
                    .long("preset")
                    .num_args(1)
                    .default_value("web-responsive")
                    .help("Built-in preset: web-responsive, gallery-hq, thumb-grid, compact-mobile, archival, social-cards"),
            )
            .arg(
                Arg::new("widths")
                    .long("widths")
                    .num_args(1)
                    .help("Comma-separated responsive widths in px (e.g. 400,800,1600)"),
            )
            .arg(
                Arg::new("formats")
                    .long("formats")
                    .num_args(1)
                    .help("Comma-separated still formats (e.g. avif,webp,jpeg,png,jxl)"),
            )
            .arg(
                Arg::new("zensim")
                    .long("zensim")
                    .num_args(1)
                    .value_parser(clap::value_parser!(f32))
                    .help("Target zensim_a perceptual quality (20.0..=99.5, e.g. 85)"),
            )
            .arg(
                Arg::new("quality")
                    .long("quality")
                    .num_args(1)
                    .value_parser(clap::value_parser!(f32))
                    .help("Calibrated quality (1.0..=100.0) instead of zensim_a verification"),
            )
            .arg(
                Arg::new("strict-verify")
                    .long("strict-verify")
                    .action(ArgAction::SetTrue)
                    .help("Fail upfront if any requested lossy format cannot run closed-loop zensim_a verification"),
            )
            .arg(
                Arg::new("strict-workspace")
                    .long("strict-workspace")
                    .action(ArgAction::SetTrue)
                    .help("Require strict proven codec workspace bounds"),
            )
            .arg(
                Arg::new("fit")
                    .long("fit")
                    .num_args(1)
                    .help("Resize fit mode: within, fit, fit_crop, within_crop, within_pad, fit_pad"),
            )
            .arg(
                Arg::new("aspect")
                    .long("aspect")
                    .num_args(1)
                    .help("Target aspect ratio (e.g. 16:9, 4:3, 1:1)"),
            )
            .arg(
                Arg::new("effort")
                    .long("effort")
                    .num_args(1)
                    .help("Encode effort: fast, balanced, thorough"),
            )
            .arg(
                Arg::new("include")
                    .long("include")
                    .action(ArgAction::Append)
                    .help("Include glob pattern (repeatable)"),
            )
            .arg(
                Arg::new("exclude")
                    .long("exclude")
                    .action(ArgAction::Append)
                    .help("Exclude glob pattern (repeatable)"),
            )
            .arg(
                Arg::new("no-recursive")
                    .long("no-recursive")
                    .action(ArgAction::SetTrue)
                    .help("Only scan top-level directory entries"),
            )
            .arg(
                Arg::new("jobs")
                    .long("jobs")
                    .short('j')
                    .num_args(1)
                    .value_parser(clap::value_parser!(usize))
                    .help("Maximum concurrent media jobs (1..=64)"),
            )
            .arg(
                Arg::new("force")
                    .long("force")
                    .action(ArgAction::SetTrue)
                    .help("Ignore incremental cache and re-encode all inputs"),
            )
            .arg(
                Arg::new("dry-run")
                    .long("dry-run")
                    .action(ArgAction::SetTrue)
                    .help("Discover inputs and validate routes without writing outputs"),
            )
            .arg(
                Arg::new("fail-fast")
                    .long("fail-fast")
                    .action(ArgAction::SetTrue)
                    .help("Stop on first item failure"),
            )
            .arg(
                Arg::new("snippets")
                    .long("snippets")
                    .action(ArgAction::SetTrue)
                    .help("Also emit snippets.html containing responsive <picture>/srcset markup"),
            )
            .arg(
                Arg::new("json")
                    .long("json")
                    .action(ArgAction::SetTrue)
                    .help("Print machine-readable JSON report to stdout"),
            ),
    )
    .subcommand(
        Command::new("gallery")
            .about("Design, preview, and build self-contained color-managed static photo galleries")
            .subcommand_required(true)
            .arg_required_else_help(true)
            .subcommand(
                Command::new("init")
                    .about("Create a starter gallery.toml by scanning a photo folder")
                    .arg(
                        Arg::new("input")
                            .value_name("INPUT_DIR")
                            .required(true)
                            .value_parser(clap::value_parser!(PathBuf)),
                    )
                    .arg(
                        Arg::new("out")
                            .long("out")
                            .short('o')
                            .num_args(1)
                            .value_parser(clap::value_parser!(PathBuf))
                            .help("Destination path for gallery.toml (defaults to <INPUT_DIR>/gallery.toml)"),
                    )
                    .arg(Arg::new("title").long("title").num_args(1).help("Gallery title"))
                    .arg(
                        Arg::new("preset")
                            .long("preset")
                            .num_args(1)
                            .default_value("gallery-hq")
                            .help("Encoding preset: gallery-hq, web-responsive, thumb-grid, compact-mobile, archival"),
                    ),
            )
            .subcommand(
                Command::new("build")
                    .about("Build a self-contained static HTML5 gallery + responsive media bundle")
                    .arg(
                        Arg::new("input")
                            .value_name("INPUT_DIR")
                            .required(true)
                            .value_parser(clap::value_parser!(PathBuf)),
                    )
                    .arg(
                        Arg::new("out")
                            .long("out")
                            .short('o')
                            .required(true)
                            .num_args(1)
                            .value_parser(clap::value_parser!(PathBuf))
                            .help("Output static site directory"),
                    )
                    .arg(
                        Arg::new("config")
                            .long("config")
                            .short('c')
                            .num_args(1)
                            .value_parser(clap::value_parser!(PathBuf))
                            .help("Gallery config (.toml or .json); auto-loads <INPUT_DIR>/gallery.toml if present"),
                    )
                    .arg(Arg::new("title").long("title").num_args(1).help("Override gallery title"))
                    .arg(Arg::new("subtitle").long("subtitle").num_args(1).help("Override subtitle"))
                    .arg(
                        Arg::new("layout")
                            .long("layout")
                            .num_args(1)
                            .help("Layout: justified, masonry, grid, editorial, filmstrip"),
                    )
                    .arg(
                        Arg::new("theme")
                            .long("theme")
                            .num_args(1)
                            .help("Theme: gallery_dark, paper_light, minimal_white, warm_linen, midnight_slate"),
                    )
                    .arg(
                        Arg::new("gap")
                            .long("gap")
                            .num_args(1)
                            .value_parser(clap::value_parser!(u32))
                            .help("Spacing between items in px (0..=64)"),
                    )
                    .arg(
                        Arg::new("radius")
                            .long("radius")
                            .num_args(1)
                            .value_parser(clap::value_parser!(u32))
                            .help("Card corner radius in px (0..=48)"),
                    )
                    .arg(
                        Arg::new("widths")
                            .long("widths")
                            .num_args(1)
                            .help("Override responsive widths (comma-separated px)"),
                    )
                    .arg(
                        Arg::new("formats")
                            .long("formats")
                            .num_args(1)
                            .help("Override output formats (comma-separated, e.g. avif,webp,jpeg)"),
                    )
                    .arg(
                        Arg::new("zensim")
                            .long("zensim")
                            .num_args(1)
                            .value_parser(clap::value_parser!(f32))
                            .help("Override zensim_a quality dial (20.0..=99.5)"),
                    )
                    .arg(
                        Arg::new("quality")
                            .long("quality")
                            .num_args(1)
                            .value_parser(clap::value_parser!(f32))
                            .help("Override calibrated quality (1.0..=100.0)"),
                    )
                    .arg(
                        Arg::new("effort")
                            .long("effort")
                            .num_args(1)
                            .help("Override encode effort: fast, balanced, thorough"),
                    )
                    .arg(
                        Arg::new("jobs")
                            .long("jobs")
                            .short('j')
                            .num_args(1)
                            .value_parser(clap::value_parser!(usize))
                            .default_value("4"),
                    )
                    .arg(Arg::new("force").long("force").action(ArgAction::SetTrue))
                    .arg(Arg::new("json").long("json").action(ArgAction::SetTrue)),
            )
            .subcommand(
                Command::new("serve")
                    .about("Launch the interactive live-preview gallery designer studio")
                    .arg(
                        Arg::new("input")
                            .value_name("INPUT_DIR")
                            .required(true)
                            .value_parser(clap::value_parser!(PathBuf)),
                    )
                    .arg(
                        Arg::new("export-dir")
                            .long("export-dir")
                            .short('o')
                            .num_args(1)
                            .value_parser(clap::value_parser!(PathBuf))
                            .help("Confined directory where static gallery exports are written (defaults to <INPUT_DIR>/dist-gallery)"),
                    )
                    .arg(
                        Arg::new("config")
                            .long("config")
                            .short('c')
                            .num_args(1)
                            .value_parser(clap::value_parser!(PathBuf))
                            .help("Path to gallery.toml to load and save"),
                    )
                    .arg(
                        Arg::new("bind")
                            .long("bind")
                            .num_args(1)
                            .default_value("127.0.0.1")
                            .help("Bind IP (default 127.0.0.1 loopback; non-loopback automatically enforces an access token)"),
                    )
                    .arg(
                        Arg::new("port")
                            .long("port")
                            .short('p')
                            .num_args(1)
                            .value_parser(clap::value_parser!(u16))
                            .default_value("3380")
                            .help("TCP port (3000..=3999 recommended)"),
                    )
                    .arg(
                        Arg::new("access-token")
                            .long("access-token")
                            .num_args(1)
                            .help("Explicit session token (auto-generated via OS CSPRNG if omitted when auth is required)"),
                    )
                    .arg(
                        Arg::new("public-origin")
                            .long("public-origin")
                            .num_args(1)
                            .help("Required https://<host>[:<port>] origin when binding non-loopback behind a TLS proxy"),
                    ),
            ),
    )
    .subcommand(
        Command::new("v2/capabilities").about("Print live Imageflow v2 codec, video, and IQA metric capabilities JSON"),
    )
    .subcommand(
        Command::new("v2/compare")
            .about("Compute score-only perceptual IQA metrics (zensim_a, ssimulacra2, butteraugli_max) between two images")
            .arg(Arg::new("reference").long("reference").short('r').required(true).value_parser(clap::value_parser!(PathBuf)))
            .arg(Arg::new("candidate").long("candidate").short('c').required(true).value_parser(clap::value_parser!(PathBuf)))
            .arg(
                Arg::new("metrics")
                    .long("metrics")
                    .default_value("zensim_a,ssimulacra2,butteraugli_max")
                    .help("Comma-separated metrics"),
            )
            .arg(
                Arg::new("best-effort")
                    .long("best-effort")
                    .action(ArgAction::SetTrue)
                    .help("Use BestEffort workspace policy (allows WebP/AVIF comparison)"),
            ),
    )
}

pub fn handle_modern_subcommand(matches: &ArgMatches) -> Option<i32> {
    if let Some(m) = matches.subcommand_matches("bulk") {
        return Some(run_bulk_cli(m));
    }
    if let Some(m) = matches.subcommand_matches("gallery") {
        return Some(run_gallery_cli(m));
    }
    if matches.subcommand_matches("v2/capabilities").is_some() {
        return Some(run_capabilities_cli());
    }
    if let Some(m) = matches.subcommand_matches("v2/compare") {
        return Some(run_compare_cli(m));
    }
    None
}

fn parse_widths_csv(s: &str) -> Result<Vec<u32>, String> {
    s.split(',')
        .map(|part| {
            part.trim()
                .parse::<u32>()
                .map_err(|_| format!("invalid width '{part}'"))
        })
        .collect()
}

fn parse_formats_csv(s: &str) -> Result<Vec<OutputFormat>, String> {
    s.split(',').map(OutputFormat::from_str).collect()
}

fn run_bulk_cli(m: &ArgMatches) -> i32 {
    let mut cfg = if let Some(path) = m.get_one::<PathBuf>("config") {
        match BulkConfig::load_file(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error: {e}");
                return 2;
            }
        }
    } else {
        let preset_name = m.get_one::<String>("preset").map(String::as_str).unwrap_or("web-responsive");
        let preset = match ResponsivePreset::from_preset_name(preset_name) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("error: {e}");
                return 2;
            }
        };
        BulkConfig {
            schema: BULK_SCHEMA_VERSION.to_string(),
            inputs: Vec::new(),
            output_dir: PathBuf::new(),
            recursive: true,
            include_globs: Vec::new(),
            exclude_globs: Vec::new(),
            follow_symlinks: false,
            output_layout: OutputPathLayout::ContentAddressed,
            concurrency: std::thread::available_parallelism().map_or(2, |n| n.get().clamp(1, 4)),
            incremental_cache: true,
            force_rebuild: false,
            on_error: FailurePolicy::Continue,
            write_manifest: true,
            write_snippets: false,
            preset,
            limits: BulkLimits::default(),
        }
    };

    if let Some(ins) = m.get_many::<String>("inputs") {
        cfg.inputs = ins.map(PathBuf::from).collect();
    }
    if let Some(out) = m.get_one::<PathBuf>("out") {
        cfg.output_dir = out.clone();
    }
    if let Some(w_str) = m.get_one::<String>("widths") {
        match parse_widths_csv(w_str) {
            Ok(ws) => cfg.preset.widths_px = ws,
            Err(e) => {
                eprintln!("error: {e}");
                return 2;
            }
        }
    }
    if let Some(f_str) = m.get_one::<String>("formats") {
        match parse_formats_csv(f_str) {
            Ok(fs) => cfg.preset.formats = fs,
            Err(e) => {
                eprintln!("error: {e}");
                return 2;
            }
        }
    }
    if let Some(&z) = m.get_one::<f32>("zensim") {
        cfg.preset.quality = QualityTarget::VerifyZensimA { target: z };
    } else if let Some(&q) = m.get_one::<f32>("quality") {
        cfg.preset.quality = QualityTarget::CalibratedQuality { quality: q };
    }
    if m.get_flag("strict-verify") {
        cfg.preset.quality_fallback = QualityFallbackPolicy::StrictVerify;
    }
    if m.get_flag("strict-workspace") {
        cfg.preset.workspace_policy = WorkspaceAdmission::Strict;
    }
    if let Some(fit_s) = m.get_one::<String>("fit") {
        match FitMode::from_str(fit_s) {
            Ok(f) => cfg.preset.fit_mode = f,
            Err(e) => {
                eprintln!("error: {e}");
                return 2;
            }
        }
    }
    if let Some(ar_s) = m.get_one::<String>("aspect") {
        match AspectRatio::from_str(ar_s) {
            Ok(ar) => cfg.preset.aspect_ratio = Some(ar),
            Err(e) => {
                eprintln!("error: {e}");
                return 2;
            }
        }
    }
    if let Some(eff_s) = m.get_one::<String>("effort") {
        match EncodeEffort::from_str(eff_s) {
            Ok(e) => cfg.preset.effort = e,
            Err(e) => {
                eprintln!("error: {e}");
                return 2;
            }
        }
    }
    if let Some(incs) = m.get_many::<String>("include") {
        cfg.include_globs = incs.cloned().collect();
    }
    if let Some(excs) = m.get_many::<String>("exclude") {
        cfg.exclude_globs = excs.cloned().collect();
    }
    if m.get_flag("no-recursive") {
        cfg.recursive = false;
    }
    if let Some(&j) = m.get_one::<usize>("jobs") {
        cfg.concurrency = j;
    }
    if m.get_flag("force") {
        cfg.force_rebuild = true;
    }
    if m.get_flag("fail-fast") {
        cfg.on_error = FailurePolicy::FailFast;
    }
    if m.get_flag("snippets") {
        cfg.write_snippets = true;
    }

    let json_mode = m.get_flag("json");
    let report = if m.get_flag("dry-run") {
        plan_bulk_dry_run(&cfg)
    } else {
        let cb: Option<Arc<dyn Fn(BulkProgressEvent) + Send + Sync>> = if json_mode {
            None
        } else {
            Some(Arc::new(|ev: BulkProgressEvent| {
                eprintln!(
                    "[{}/{}] {:<7} {} ({} variants, {:.1} KB -> {:.1} KB){}",
                    ev.index,
                    ev.total,
                    ev.status,
                    ev.relative_key,
                    ev.variants_written,
                    ev.input_bytes as f64 / 1024.0,
                    ev.output_bytes as f64 / 1024.0,
                    ev.error.as_deref().map_or(String::new(), |e| format!(" ERROR: {e}")),
                );
            }))
        };
        run_bulk(&cfg, None, cb)
    };

    match report {
        Ok(rep) => {
            if json_mode {
                println!("{}", serde_json::to_string_pretty(&rep).unwrap_or_default());
            } else {
                println!(
                    "Bulk complete: {} discovered, {} encoded, {} cached, {} failed ({:.1} KB in -> {:.1} KB out, {} ms)",
                    rep.discovered,
                    rep.processed,
                    rep.skipped_cached,
                    rep.failed,
                    rep.total_input_bytes as f64 / 1024.0,
                    rep.total_output_bytes as f64 / 1024.0,
                    rep.elapsed_ms
                );
                if let Some(mp) = &rep.manifest_path {
                    println!("Manifest: {}", mp.display());
                }
            }
            if rep.failed > 0 { 1 } else { 0 }
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

fn run_gallery_cli(m: &ArgMatches) -> i32 {
    if let Some(im) = m.subcommand_matches("init") {
        let input = im.get_one::<PathBuf>("input").unwrap();
        let out = im
            .get_one::<PathBuf>("out")
            .cloned()
            .unwrap_or_else(|| input.join("gallery.toml"));
        let title = im.get_one::<String>("title").cloned();
        let preset = im.get_one::<String>("preset").map(String::as_str).unwrap_or("gallery-hq");
        match init_gallery_config(input, title, preset)
            .and_then(|c| c.to_toml_string().map(|s| (c, s)))
            .and_then(|(c, toml_s)| write_file_atomic(&out, toml_s.as_bytes()).map(|()| c))
        {
            Ok(c) => {
                println!("Initialized {} ({} items discovered)", out.display(), c.items.len());
                return 0;
            }
            Err(e) => {
                eprintln!("error: {e}");
                return 1;
            }
        }
    }

    if let Some(bm) = m.subcommand_matches("build") {
        let input = bm.get_one::<PathBuf>("input").unwrap();
        let out = bm.get_one::<PathBuf>("out").unwrap();
        let mut cfg = if let Some(cp) = bm.get_one::<PathBuf>("config") {
            match GalleryConfig::load_file(cp) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            }
        } else {
            let auto = input.join("gallery.toml");
            if auto.is_file() {
                match GalleryConfig::load_file(&auto) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("error: {e}");
                        return 2;
                    }
                }
            } else {
                match init_gallery_config(input, None, "gallery-hq") {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("error: {e}");
                        return 2;
                    }
                }
            }
        };

        if let Some(t) = bm.get_one::<String>("title") {
            cfg.title = t.clone();
        }
        if let Some(s) = bm.get_one::<String>("subtitle") {
            cfg.subtitle = Some(s.clone());
        }
        if let Some(l) = bm.get_one::<String>("layout") {
            match GalleryLayout::from_str(l) {
                Ok(v) => cfg.design.layout = v,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            }
        }
        if let Some(t) = bm.get_one::<String>("theme") {
            match GalleryTheme::from_str(t) {
                Ok(v) => cfg.design.theme = v,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            }
        }
        if let Some(&g) = bm.get_one::<u32>("gap") {
            cfg.design.gap_px = g;
        }
        if let Some(&r) = bm.get_one::<u32>("radius") {
            cfg.design.corner_radius_px = r;
        }
        if let Some(ws) = bm.get_one::<String>("widths") {
            match parse_widths_csv(ws) {
                Ok(v) => cfg.encoding.widths_px = v,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            }
        }
        if let Some(fs) = bm.get_one::<String>("formats") {
            match parse_formats_csv(fs) {
                Ok(v) => cfg.encoding.formats = v,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            }
        }
        if let Some(&q) = bm.get_one::<f32>("quality") {
            cfg.encoding.quality = QualityTarget::CalibratedQuality { quality: q };
        }
        if let Some(&z) = bm.get_one::<f32>("zensim") {
            cfg.encoding.quality = QualityTarget::VerifyZensimA { target: z };
        }
        if let Some(eff) = bm.get_one::<String>("effort") {
            match EncodeEffort::from_str(eff) {
                Ok(v) => cfg.encoding.effort = v,
                Err(e) => {
                    eprintln!("error: {e}");
                    return 2;
                }
            }
        }
        let jobs = *bm.get_one::<usize>("jobs").unwrap_or(&4);
        let force = bm.get_flag("force");
        let json_mode = bm.get_flag("json");
        let cb: Option<Arc<dyn Fn(BulkProgressEvent) + Send + Sync>> = if json_mode {
            None
        } else {
            Some(Arc::new(|ev: BulkProgressEvent| {
                eprintln!("[{}/{}] {:<7} {}", ev.index, ev.total, ev.status, ev.relative_key);
            }))
        };
        return match build_gallery(input, out, &cfg, jobs, force, BulkLimits::default(), None, cb) {
            Ok(rep) => {
                if json_mode {
                    println!("{}", serde_json::to_string_pretty(&rep).unwrap_or_default());
                } else {
                    println!(
                        "Gallery built: {} ({} visible, {} hidden, {} encoded, {} cached, {} ms)",
                        rep.index_html_path.display(),
                        rep.visible_items,
                        rep.hidden_items,
                        rep.bulk_report.processed,
                        rep.bulk_report.skipped_cached,
                        rep.bulk_report.elapsed_ms
                    );
                }
                if rep.bulk_report.failed > 0 { 1 } else { 0 }
            }
            Err(e) => {
                eprintln!("error: {e}");
                1
            }
        };
    }

    if let Some(sm) = m.subcommand_matches("serve") {
        let input_dir = sm.get_one::<PathBuf>("input").unwrap().clone();
        let export_dir = sm
            .get_one::<PathBuf>("export-dir")
            .cloned()
            .unwrap_or_else(|| input_dir.join("dist-gallery"));
        let config_path = sm.get_one::<PathBuf>("config").cloned();
        let bind_str = sm.get_one::<String>("bind").map(String::as_str).unwrap_or("127.0.0.1");
        let bind_ip: IpAddr = match bind_str.parse() {
            Ok(ip) => ip,
            Err(e) => {
                eprintln!("error: invalid bind IP '{bind_str}': {e}");
                return 2;
            }
        };
        let port = *sm.get_one::<u16>("port").unwrap_or(&3380);
        let access_token = sm.get_one::<String>("access-token").cloned();
        let public_origin = sm.get_one::<String>("public-origin").cloned();
        let opts = DesignerServerOptions {
            input_dir,
            export_dir,
            config_path,
            bind_ip,
            port,
            access_token,
            public_origin,
        };
        return match serve_gallery_designer(opts, None) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("error: {e}");
                1
            }
        };
    }

    64
}

fn run_capabilities_cli() -> i32 {
    let mut ctx = match imageflow_core::Context::create() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: context create failed: {e}");
            return 1;
        }
    };
    let (resp, res) = ctx.message("v2/capabilities", b"{}");
    match res {
        Ok(()) => {
            println!("{}", String::from_utf8_lossy(&resp.response_json));
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

fn run_compare_cli(m: &ArgMatches) -> i32 {
    let ref_path = m.get_one::<PathBuf>("reference").unwrap();
    let cand_path = m.get_one::<PathBuf>("candidate").unwrap();
    let metrics: Vec<String> = m
        .get_one::<String>("metrics")
        .map(String::as_str)
        .unwrap_or("zensim_a")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let ref_bytes = match std::fs::read(ref_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error reading reference '{}': {e}", ref_path.display());
            return 2;
        }
    };
    let cand_bytes = match std::fs::read(cand_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error reading candidate '{}': {e}", cand_path.display());
            return 2;
        }
    };
    let mut ctx = match imageflow_core::Context::create() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error creating context: {e}");
            return 1;
        }
    };
    if m.get_flag("best-effort") {
        let _ = ctx.message("v2/configure_context", br#"{"workspace_policy":"best_effort"}"#);
    }
    if let Err(e) = ctx.add_copied_input_buffer(0, &ref_bytes) {
        eprintln!("error: {e}");
        return 1;
    }
    if let Err(e) = ctx.add_copied_input_buffer(1, &cand_bytes) {
        eprintln!("error: {e}");
        return 1;
    }
    let req = serde_json::json!({
        "reference_io_id": 0,
        "candidate_io_id": 1,
        "metrics": metrics
    });
    let (resp, res) = ctx.message("v2/compare", req.to_string().as_bytes());
    match res {
        Ok(()) => {
            println!("{}", String::from_utf8_lossy(&resp.response_json));
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}
