//! JPEG quality-dial → SSIMULACRA2 calibration sweep.
//!
//! Two encoders, measured on the SAME resized reference per cell:
//!   A. `libjpeg-turbo`  — stock `cjpeg` subprocess, `-quality Q -sample 2x2`
//!      (fixed 4:2:0, Annex-K tables, no trellis): the human-mindshare dial.
//!   B. `imageflow-moz`  — faithful replica of imageflow-2's `mozjpeg.rs`:
//!      mozjpeg defaults (trellis + tuned tables + optimize) + `evalchroma`
//!      content-adaptive chroma. Records evalchroma's per-cell chroma choice.
//!
//! Per (image × target-size × q × encoder): encode → decode → SSIMULACRA2,
//! emit one CSV row. Convert to Parquet + fit Q→ssim2 in the analysis step.

use anyhow::{Context as _, Result};
use clap::Parser;
use rayon::prelude::*;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(about = "libjpeg-turbo vs imageflow-2 mozjpeg+evalchroma → SSIMULACRA2 sweep")]
struct Args {
    /// Corpus directory (repeatable). content_class = the directory's file name.
    #[arg(long = "corpus", required = true)]
    corpus: Vec<PathBuf>,
    /// Output CSV path.
    #[arg(long, default_value = "sweep.csv")]
    out: PathBuf,
    /// Target long-edge sizes for downscaling (skip-upscale). The native source
    /// size is always also included as the "large" point.
    #[arg(long, value_delimiter = ',', default_value = "64,256,1024")]
    sizes: Vec<u32>,
    /// Quality grid (libjpeg/mozjpeg 0..100). Dense at low-q per sweep discipline.
    #[arg(
        long,
        value_delimiter = ',',
        default_value = "5,10,15,20,25,30,35,40,45,50,55,60,65,70,74,78,82,85,88,90,92,94,96,98,100"
    )]
    q: Vec<u8>,
    /// Limit number of images per corpus (smoke test; takes the first N).
    #[arg(long)]
    limit: Option<usize>,
    /// Stride-sample each corpus down to ~N images (representative coverage of a
    /// large set without taking the alphabetical head). Applied before --limit.
    #[arg(long)]
    max_per_corpus: Option<usize>,
    /// Cap pixels per encoded cell — the native ("large") size is downscaled to
    /// at most this. Bounds the SSIMULACRA2 precompute memory so huge tall
    /// screenshots can't OOM the box.
    #[arg(long, default_value_t = 4_000_000)]
    max_pixels: u64,
    /// Threads (default = all cores).
    #[arg(long)]
    threads: Option<usize>,
    /// Progress heartbeat file (cells done / total).
    #[arg(long, default_value = "sweep.progress")]
    progress: PathBuf,
}

#[derive(Clone)]
struct Row {
    image_id: String,
    content_class: String,
    src_w: u32,
    src_h: u32,
    tgt_w: u32,
    tgt_h: u32,
    pixels: u64,
    q: u8,
    encoder: &'static str,
    bytes: u64,
    bpp: f64,
    ssim2: f64,
    chroma: String,
    encode_ms: f64,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if let Some(t) = args.threads {
        rayon::ThreadPoolBuilder::new().num_threads(t).build_global().ok();
    }

    // Collect (path, content_class) for every image in every corpus dir.
    let mut images: Vec<(PathBuf, String)> = Vec::new();
    for dir in &args.corpus {
        let class = dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".into());
        let mut paths: Vec<PathBuf> = WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter_map(|e| {
                let p = e.path();
                let ext =
                    p.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
                matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp" | "tif" | "tiff")
                    .then(|| p.to_path_buf())
            })
            .collect();
        paths.sort();
        // Stride-sample a large corpus down to ~max_per_corpus for even coverage
        // (avoids the alphabetical head bias of a plain --limit).
        if let Some(max) = args.max_per_corpus {
            if paths.len() > max {
                let stride = paths.len().div_ceil(max);
                paths = paths.into_iter().step_by(stride).collect();
            }
        }
        if let Some(l) = args.limit {
            paths.truncate(l);
        }
        eprintln!("corpus {class}: {} images", paths.len());
        for p in paths {
            images.push((p, class.clone()));
        }
    }
    eprintln!("total source images: {}", images.len());

    let q_grid = args.q.clone();
    let sizes = args.sizes.clone();
    let max_pixels = args.max_pixels;
    let total_imgs = images.len();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let progress_path = args.progress.clone();

    // Stream rows to CSV incrementally (under a lock): an OOM/crash never loses
    // completed work, and memory stays bounded to one image's rows at a time.
    let mut header_w = csv::Writer::from_path(&args.out)?;
    header_w.write_record([
        "image_id",
        "content_class",
        "src_w",
        "src_h",
        "tgt_w",
        "tgt_h",
        "pixels",
        "q",
        "encoder",
        "bytes",
        "bpp",
        "ssim2",
        "chroma",
        "encode_ms",
    ])?;
    header_w.flush()?;
    let writer = std::sync::Mutex::new(header_w);

    // Parallel over source images; each emits + flushes its (size × q × encoder) rows.
    images.par_iter().for_each(|(path, class)| {
        let rows = match process_image(path, class, &sizes, &q_grid, max_pixels) {
            Ok(rows) => rows,
            Err(e) => {
                eprintln!("  SKIP {}: {e:#}", path.display());
                Vec::new()
            }
        };
        if !rows.is_empty() {
            let mut w = writer.lock().unwrap();
            for r in &rows {
                let _ = write_row(&mut w, r);
            }
            let _ = w.flush();
        }
        let d = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if d % 10 == 0 || d == total_imgs {
            let _ = std::fs::write(&progress_path, format!("{d}/{total_imgs} images\n"));
            eprintln!("  {d}/{total_imgs} images");
        }
    });

    writer.lock().unwrap().flush()?;
    eprintln!("done -> {}", args.out.display());
    Ok(())
}

fn write_row<W: std::io::Write>(w: &mut csv::Writer<W>, r: &Row) -> Result<()> {
    w.write_record([
        &r.image_id,
        &r.content_class,
        &r.src_w.to_string(),
        &r.src_h.to_string(),
        &r.tgt_w.to_string(),
        &r.tgt_h.to_string(),
        &r.pixels.to_string(),
        &r.q.to_string(),
        r.encoder,
        &r.bytes.to_string(),
        &format!("{:.5}", r.bpp),
        &format!("{:.4}", r.ssim2),
        &r.chroma,
        &format!("{:.2}", r.encode_ms),
    ])?;
    Ok(())
}

fn process_image(
    path: &std::path::Path,
    class: &str,
    sizes: &[u32],
    q_grid: &[u8],
    max_pixels: u64,
) -> Result<Vec<Row>> {
    let dyn_img = image::open(path).with_context(|| format!("open {}", path.display()))?;
    let src = flatten_to_rgb(&dyn_img);
    let (src_w, src_h) = (src.width(), src.height());
    let src_long = src_w.max(src_h);

    // Target dims: each requested long-edge < source (downscale), plus native.
    let mut targets: Vec<(u32, u32)> = Vec::new();
    for &t in sizes {
        if t < src_long {
            let f = t as f64 / src_long as f64;
            let tw = ((src_w as f64 * f).round() as u32).max(1);
            let th = ((src_h as f64 * f).round() as u32).max(1);
            targets.push((tw, th));
        }
    }
    // Native ("large"), capped to max_pixels so a huge tall screenshot doesn't
    // blow up the SSIMULACRA2 precompute buffers (× rayon threads → OOM).
    let native = if (src_w as u64) * (src_h as u64) > max_pixels {
        let scale = (max_pixels as f64 / (src_w as u64 * src_h as u64) as f64).sqrt();
        (
            ((src_w as f64 * scale).round() as u32).max(1),
            ((src_h as f64 * scale).round() as u32).max(1),
        )
    } else {
        (src_w, src_h)
    };
    targets.push(native);
    targets.sort();
    targets.dedup();

    let image_id = path.file_name().unwrap().to_string_lossy().to_string();
    let mut rows = Vec::new();

    for (tw, th) in targets {
        let reference: image::RgbImage = if (tw, th) == (src_w, src_h) {
            src.clone()
        } else {
            image::imageops::resize(&src, tw, th, image::imageops::FilterType::Lanczos3)
        };
        let pixels = tw as u64 * th as u64;
        // Precompute the reference once (~50% of SSIMULACRA2 work) — reused for
        // every (q × encoder) distorted variant of this resized reference.
        let ref_ssim = fast_ssim2::Ssimulacra2Reference::new(to_linear(&reference))
            .map_err(|e| anyhow::anyhow!("ssim2 ref precompute: {e:?}"))?;

        for &q in q_grid {
            for encoder in [Encoder::LibjpegTurbo, Encoder::ImageflowMoz] {
                let t0 = Instant::now();
                let enc = match encoder.encode(&reference, tw, th, q) {
                    Ok(e) => e,
                    Err(e) => {
                        eprintln!("  enc fail {image_id} {:?} q{q} {tw}x{th}: {e:#}", encoder);
                        continue;
                    }
                };
                let encode_ms = t0.elapsed().as_secs_f64() * 1000.0;
                let distorted = decode_jpeg(&enc.bytes)
                    .with_context(|| format!("decode {image_id} {:?} q{q}", encoder))?;
                let ssim2 = ref_ssim
                    .compare(to_linear(&distorted))
                    .map_err(|e| anyhow::anyhow!("ssim2 compare: {e:?}"))?;
                let bytes = enc.bytes.len() as u64;
                rows.push(Row {
                    image_id: image_id.clone(),
                    content_class: class.to_string(),
                    src_w,
                    src_h,
                    tgt_w: tw,
                    tgt_h: th,
                    pixels,
                    q,
                    encoder: encoder.name(),
                    bytes,
                    bpp: 8.0 * bytes as f64 / pixels as f64,
                    ssim2,
                    chroma: enc.chroma,
                    encode_ms,
                });
            }
        }
    }
    Ok(rows)
}

/// Composite any alpha over white (imageflow's default matte), giving opaque RGB8.
fn flatten_to_rgb(img: &image::DynamicImage) -> image::RgbImage {
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let mut out = image::RgbImage::new(w, h);
    for (x, y, px) in rgba.enumerate_pixels() {
        let a = px[3] as u32;
        let blend = |c: u8| -> u8 { ((c as u32 * a + 255 * (255 - a)) / 255) as u8 };
        out.put_pixel(x, y, image::Rgb([blend(px[0]), blend(px[1]), blend(px[2])]));
    }
    out
}

#[derive(Clone, Copy, Debug)]
enum Encoder {
    LibjpegTurbo,
    ImageflowMoz,
}

struct Encoded {
    bytes: Vec<u8>,
    chroma: String,
}

impl Encoder {
    fn name(self) -> &'static str {
        match self {
            Encoder::LibjpegTurbo => "libjpeg-turbo",
            Encoder::ImageflowMoz => "imageflow-moz",
        }
    }
    fn encode(self, img: &image::RgbImage, w: u32, h: u32, q: u8) -> Result<Encoded> {
        match self {
            Encoder::LibjpegTurbo => encode_cjpeg(img, w, h, q),
            Encoder::ImageflowMoz => encode_imageflow_moz(img, w, h, q),
        }
    }
}

/// Stock libjpeg-turbo via the `cjpeg` CLI: `-quality Q -sample 2x2` (4:2:0,
/// Annex-K, baseline). PPM in on stdin, JPEG out on stdout.
fn encode_cjpeg(img: &image::RgbImage, w: u32, h: u32, q: u8) -> Result<Encoded> {
    let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
    ppm.extend_from_slice(img.as_raw());
    let mut child = Command::new("cjpeg")
        .args(["-quality", &q.to_string(), "-sample", "2x2"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn cjpeg (install libjpeg-turbo-progs)")?;
    // Write stdin on a separate thread while wait_with_output() drains stdout
    // concurrently — otherwise a large JPEG fills the stdout pipe, cjpeg blocks
    // writing it, stops reading stdin, and our write_all deadlocks.
    let mut stdin = child.stdin.take().unwrap();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&ppm);
        // `stdin` drops here → EOF for cjpeg.
    });
    let out = child.wait_with_output()?;
    let _ = writer.join();
    anyhow::ensure!(out.status.success() && !out.stdout.is_empty(), "cjpeg failed");
    Ok(Encoded { bytes: out.stdout, chroma: "4:2:0".into() })
}

/// Faithful replica of imageflow-2 `mozjpeg.rs::write_frame` (Defaults::MozJPEG):
/// mozjpeg defaults (trellis + tuned tables) + optimize + progressive, with
/// `evalchroma::adjust_sampling` choosing chroma subsampling content-adaptively.
fn encode_imageflow_moz(img: &image::RgbImage, w: u32, h: u32, q: u8) -> Result<Encoded> {
    use rgb::FromSlice;
    let rgb_pixels: &[rgb::RGB8] = img.as_raw().as_rgb();
    let imgref = imgref::ImgRef::new(rgb_pixels, w as usize, h as usize);

    // evalchroma: worst-allowed 2x2 (4:2:0), quality == q (imageflow's chroma_quality).
    let max_sampling = evalchroma::PixelSize { cb: (2, 2), cr: (2, 2) };
    let res = evalchroma::adjust_sampling(imgref, max_sampling, q as f32);
    let sub = res.subsampling;
    let max_h = sub.cb.0.max(sub.cr.0);
    let max_v = sub.cb.1.max(sub.cr.1);
    let chroma = chroma_label(max_h, max_v, sub.cb, sub.cr);

    let mut comp = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
    comp.set_size(w as usize, h as usize);
    comp.set_quality(q as f32);
    comp.set_optimize_coding(true);
    comp.set_progressive_mode();

    // Translate evalchroma's chroma pixel sizes into JPEG channel sampling factors
    // (exactly mozjpeg.rs: luma is (1,1) reference, chroma scaled by max/this).
    let px_sizes: [(u8, u8); 3] = [(1, 1), sub.cb, sub.cr];
    for (c, &(hh, vv)) in comp.components_mut().iter_mut().zip(px_sizes.iter()) {
        c.h_samp_factor = (max_h / hh) as i32;
        c.v_samp_factor = (max_v / vv) as i32;
    }

    let mut started = comp.start_compress(Vec::new())?;
    started.write_scanlines(img.as_raw())?;
    let writer = started.finish()?;
    Ok(Encoded { bytes: writer, chroma })
}

fn chroma_label(max_h: u8, max_v: u8, cb: (u8, u8), cr: (u8, u8)) -> String {
    // 4:4:4 = no subsampling (1,1); 4:2:0 = (2,2); 4:2:2 = (2,1); else explicit.
    if max_h == 1 && max_v == 1 {
        "4:4:4".into()
    } else if max_h == 2 && max_v == 2 {
        "4:2:0".into()
    } else if max_h == 2 && max_v == 1 {
        "4:2:2".into()
    } else {
        format!("cb{}x{}_cr{}x{}", cb.0, cb.1, cr.0, cr.1)
    }
}

/// Decode any JPEG (cjpeg or mozjpeg output) to RGB8 via mozjpeg's decoder.
fn decode_jpeg(bytes: &[u8]) -> Result<image::RgbImage> {
    let dec = mozjpeg::Decompress::new_mem(bytes)?;
    let mut dec = dec.rgb()?;
    let w = dec.width() as u32;
    let h = dec.height() as u32;
    let pixels: Vec<rgb::RGB8> = dec.read_scanlines()?;
    dec.finish()?;
    let mut raw = Vec::with_capacity(pixels.len() * 3);
    for p in pixels {
        raw.extend_from_slice(&[p.r, p.g, p.b]);
    }
    image::RgbImage::from_raw(w, h, raw).context("rgb image from decoded scanlines")
}

/// sRGB RGB8 → fast-ssim2 linear-RGB input (SSIMULACRA2 works in linear/XYB).
fn to_linear(img: &image::RgbImage) -> fast_ssim2::LinearRgbImage {
    let (w, h) = img.dimensions();
    let data: Vec<[f32; 3]> = img
        .pixels()
        .map(|p| {
            [
                fast_ssim2::srgb_u8_to_linear(p[0]),
                fast_ssim2::srgb_u8_to_linear(p[1]),
                fast_ssim2::srgb_u8_to_linear(p[2]),
            ]
        })
        .collect();
    fast_ssim2::LinearRgbImage::new(data, w as usize, h as usize)
}
