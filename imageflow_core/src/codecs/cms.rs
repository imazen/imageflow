#[cfg(feature = "c-codecs")]
use crate::codecs::lcms2_transform::Lcms2TransformCache;
use crate::codecs::moxcms_transform::MoxcmsTransformCache;
use crate::codecs::source_profile::SourceProfile;
use crate::graphics::bitmaps::BitmapWindowMut;
use crate::Result;

/// Selects which CMS backend to use for color profile transforms.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum CmsBackend {
    /// Pure Rust CMS (moxcms). Default. Supports CICP, ICC, gAMA+cHRM.
    #[default]
    Moxcms,
    /// C library CMS (lcms2). Supports ICC, gAMA+cHRM, CMYK.
    /// Requires the `c-codecs` feature.
    Lcms2,
    /// Run both backends, compare outputs, warn on divergence exceeding threshold.
    /// Uses moxcms result as the canonical output.
    /// Requires the `c-codecs` feature.
    Both,
}

/// Process-wide CMS backend override. Stored as u8 matching CmsBackend discriminants.
/// 0 = no override (use default), 1 = Moxcms, 2 = Lcms2, 3 = Both.
static CMS_BACKEND_OVERRIDE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// When true, CMS diagnostic messages (Both-mode divergence, warnings) print to stderr.
/// Off by default. Enable via `CmsBackend::enable_stderr_diagnostics()`.
static CMS_STDERR_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn cms_eprintln(args: std::fmt::Arguments<'_>) {
    if CMS_STDERR_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
        eprintln!("{args}");
    }
}

impl CmsBackend {
    /// Enable CMS diagnostic messages to stderr (Both-mode divergence, warnings).
    pub fn enable_stderr_diagnostics() {
        CMS_STDERR_ENABLED.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Set the process-wide CMS backend. All subsequently created Contexts will use this.
    pub fn set_process_default(backend: CmsBackend) {
        let val = match backend {
            CmsBackend::Moxcms => 1,
            CmsBackend::Lcms2 => 2,
            CmsBackend::Both => 3,
        };
        CMS_BACKEND_OVERRIDE.store(val, std::sync::atomic::Ordering::Relaxed);
    }

    /// Returns the CMS backend for new Context instances.
    pub(crate) fn default_for_context() -> CmsBackend {
        match CMS_BACKEND_OVERRIDE.load(std::sync::atomic::Ordering::Relaxed) {
            1 => CmsBackend::Moxcms,
            2 => CmsBackend::Lcms2,
            3 => CmsBackend::Both,
            _ => Default::default(),
        }
    }
}

/// Dispatch a source profile → sRGB transform to the selected backend.
///
/// If the profile is sRGB, this is a no-op regardless of backend.
pub fn transform_to_srgb(frame: &mut BitmapWindowMut<u8>, profile: &SourceProfile) -> Result<()> {
    if profile.is_srgb() {
        return Ok(());
    }

    match CmsBackend::default_for_context() {
        CmsBackend::Moxcms => MoxcmsTransformCache::transform_to_srgb(frame, profile),
        #[cfg(feature = "c-codecs")]
        CmsBackend::Lcms2 => Lcms2TransformCache::transform_to_srgb(frame, profile),
        #[cfg(not(feature = "c-codecs"))]
        CmsBackend::Lcms2 => {
            cms_eprintln(format_args!(
                "[CMS] lcms2 backend requested but c-codecs feature is disabled, using moxcms"
            ));
            MoxcmsTransformCache::transform_to_srgb(frame, profile)
        }
        #[cfg(feature = "c-codecs")]
        CmsBackend::Both => {
            // lcms2 doesn't support CICP — fall back to moxcms only
            if matches!(profile, SourceProfile::Cicp { .. }) {
                return MoxcmsTransformCache::transform_to_srgb(frame, profile);
            }

            let is_cmyk = matches!(profile, SourceProfile::CmykIcc(_));
            let threshold: u8 = if is_cmyk { 3 } else { 1 };

            // Snapshot original frame data (alloc 1 of 2)
            let row_bytes = frame.w() as usize * frame.t_per_pixel();
            let h = frame.h() as usize;
            let mut snapshot = Vec::with_capacity(row_bytes * h);
            for scanline in frame.scanlines() {
                snapshot.extend_from_slice(scanline.row());
            }

            // Run moxcms (in-place on frame)
            MoxcmsTransformCache::transform_to_srgb(frame, profile)?;

            // Capture moxcms result (alloc 2 of 2)
            let mut moxcms_result = Vec::with_capacity(row_bytes * h);
            for scanline in frame.scanlines() {
                moxcms_result.extend_from_slice(scanline.row());
            }

            // Restore original for lcms2 (then drop snapshot to reduce peak memory)
            {
                let mut src_offset = 0;
                for mut scanline in frame.scanlines() {
                    let row = scanline.row_mut();
                    row.copy_from_slice(&snapshot[src_offset..src_offset + row.len()]);
                    src_offset += row.len();
                }
            }
            drop(snapshot);

            // lcms2 may panic on unsupported pixel formats (e.g. gray ICC with BGRA frame).
            // Catch panics so a single bad profile doesn't break subsequent images.
            let lcms2_result_or_panic =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    Lcms2TransformCache::transform_to_srgb(frame, profile)
                }));

            match lcms2_result_or_panic {
                Ok(Ok(())) => {
                    // Compare moxcms result against frame (which now holds lcms2 result)
                    // — no third allocation needed.
                    compare_results_against_frame(
                        &moxcms_result,
                        frame,
                        threshold,
                        is_cmyk,
                        profile,
                    );
                }
                Ok(Err(e)) => {
                    cms_eprintln(format_args!("[CMS Both] lcms2 error (moxcms result used): {e}"));
                }
                Err(_) => {
                    cms_eprintln(format_args!("[CMS Both] lcms2 panicked (moxcms result used)"));
                }
            }

            // Restore moxcms result as canonical
            {
                let mut src_offset = 0;
                for mut scanline in frame.scanlines() {
                    let row = scanline.row_mut();
                    row.copy_from_slice(&moxcms_result[src_offset..src_offset + row.len()]);
                    src_offset += row.len();
                }
            }

            Ok(())
        }
        #[cfg(not(feature = "c-codecs"))]
        CmsBackend::Both => {
            cms_eprintln(format_args!(
                "[CMS] Both-mode requested but c-codecs feature is disabled, using moxcms"
            ));
            MoxcmsTransformCache::transform_to_srgb(frame, profile)
        }
    }
}

/// Compare moxcms result buffer against the frame (which holds lcms2 result).
/// Reads lcms2 data directly from frame scanlines to avoid a third allocation.
#[cfg(feature = "c-codecs")]
fn compare_results_against_frame(
    moxcms: &[u8],
    frame: &mut BitmapWindowMut<u8>,
    max_diff: u8,
    is_cmyk: bool,
    profile: &SourceProfile,
) {
    let row_bytes = frame.w() as usize * frame.t_per_pixel();
    let h = frame.h() as usize;
    let total_bytes = row_bytes * h;

    if moxcms.len() != total_bytes {
        cms_eprintln(format_args!(
            "[CMS Both] WARNING: buffer length mismatch: moxcms={}, frame={}",
            moxcms.len(),
            total_bytes
        ));
        return;
    }

    let total_pixels = total_bytes / 4;
    let profile_type = if is_cmyk { "CMYK" } else { "RGB" };

    // Per-channel stats
    let mut max_per_channel = [0u8; 4];
    let mut sum_per_channel = [0u64; 4];
    let mut divergent_pixels = 0u64;
    let mut max_observed = 0u8;
    let mut diff_histogram = [0u64; 256];
    let mut worst_pixel: Option<(usize, [u8; 4], [u8; 4], u8)> = None;

    let mut pixel_idx = 0usize;
    let mut moxcms_offset = 0usize;
    for scanline in frame.scanlines() {
        let lcms2_row = scanline.row();
        let moxcms_row = &moxcms[moxcms_offset..moxcms_offset + lcms2_row.len()];
        moxcms_offset += lcms2_row.len();

        for (m_pixel, l_pixel) in moxcms_row.chunks_exact(4).zip(lcms2_row.chunks_exact(4)) {
            let mut pixel_max_diff: u8 = 0;
            for (ch, (&a, &b)) in m_pixel.iter().zip(l_pixel.iter()).enumerate() {
                let diff = a.abs_diff(b);
                if diff > max_per_channel[ch] {
                    max_per_channel[ch] = diff;
                }
                sum_per_channel[ch] += diff as u64;
                if diff > pixel_max_diff {
                    pixel_max_diff = diff;
                }
            }
            diff_histogram[pixel_max_diff as usize] += 1;
            if pixel_max_diff > max_diff {
                divergent_pixels += 1;
                if pixel_max_diff > max_observed {
                    max_observed = pixel_max_diff;
                    let mut m = [0u8; 4];
                    let mut l = [0u8; 4];
                    m.copy_from_slice(m_pixel);
                    l.copy_from_slice(l_pixel);
                    worst_pixel = Some((pixel_idx, m, l, pixel_max_diff));
                }
            }
            pixel_idx += 1;
        }
    }

    if divergent_pixels == 0 {
        return;
    }

    // Compute percentiles from histogram
    let p50_idx = total_pixels as u64 / 2;
    let p95_idx = total_pixels as u64 * 95 / 100;
    let p99_idx = total_pixels as u64 * 99 / 100;
    let mut cumulative = 0u64;
    let mut p50: Option<u8> = None;
    let mut p95: Option<u8> = None;
    let mut p99: Option<u8> = None;
    for (diff_val, &count) in diff_histogram.iter().enumerate() {
        cumulative += count;
        if p50.is_none() && cumulative > p50_idx {
            p50 = Some(diff_val as u8);
        }
        if p95.is_none() && cumulative > p95_idx {
            p95 = Some(diff_val as u8);
        }
        if p99.is_none() && cumulative > p99_idx {
            p99 = Some(diff_val as u8);
        }
    }
    // These are always Some when divergent_pixels > 0 because the histogram
    // loop above processes every divergent pixel, so every percentile index
    // will be reached. The unwrap_or(0) is a no-op safety net.
    let p50 = p50.unwrap_or(0);
    let p95 = p95.unwrap_or(0);
    let p99 = p99.unwrap_or(0);

    let n = total_pixels as f64;
    let mean_b = sum_per_channel[0] as f64 / n;
    let mean_g = sum_per_channel[1] as f64 / n;
    let mean_r = sum_per_channel[2] as f64 / n;

    let desc = profile.describe();
    cms_eprintln(format_args!(
        "[CMS Both] WARNING: {profile_type} {divergent_pixels}/{total_pixels} diverge >{max_diff} \
         (max={max_observed} p99={p99} p95={p95} p50={p50} mean_bgr={mean_b:.2},{mean_g:.2},{mean_r:.2} \
         ch_max=B{}/G{}/R{}/A{}) profile={desc}",
        max_per_channel[0],
        max_per_channel[1],
        max_per_channel[2],
        max_per_channel[3],
    ));
    if let Some((idx, m, l, diff)) = worst_pixel {
        cms_eprintln(format_args!(
            "[CMS Both]   worst pixel #{idx} (diff={diff}): \
             moxcms=[{},{},{},{}] lcms2=[{},{},{},{}]",
            m[0], m[1], m[2], m[3], l[0], l[1], l[2], l[3]
        ));
    }
}
