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
    Lcms2,
    /// Run both backends, compare outputs, warn on divergence exceeding threshold.
    /// Uses moxcms result as the canonical output.
    /// Thresholds: 1 per channel for RGB profiles, 17 for CMYK (different LUT grid sizes).
    /// CICP profiles fall back to moxcms-only (lcms2 doesn't support CICP).
    Both,
}

impl CmsBackend {
    /// Read from `CMS_BACKEND` env var: "moxcms", "lcms2", or "both".
    /// Falls back to the provided default if unset or unrecognized.
    pub fn from_env_or(default: CmsBackend) -> CmsBackend {
        match std::env::var("CMS_BACKEND").as_deref() {
            Ok("both") => CmsBackend::Both,
            Ok("lcms2") => CmsBackend::Lcms2,
            Ok("moxcms") => CmsBackend::Moxcms,
            _ => default,
        }
    }
}

/// Dispatch a source profile → sRGB transform to the selected backend.
///
/// If the profile is sRGB, this is a no-op regardless of backend.
pub fn transform_to_srgb(
    frame: &mut BitmapWindowMut<u8>,
    profile: &SourceProfile,
    backend: CmsBackend,
) -> Result<()> {
    if profile.is_srgb() {
        return Ok(());
    }

    match backend {
        CmsBackend::Moxcms => MoxcmsTransformCache::transform_to_srgb(frame, profile),
        CmsBackend::Lcms2 => Lcms2TransformCache::transform_to_srgb(frame, profile),
        CmsBackend::Both => {
            // lcms2 doesn't support CICP — fall back to moxcms only
            if matches!(profile, SourceProfile::Cicp { .. }) {
                return MoxcmsTransformCache::transform_to_srgb(frame, profile);
            }

            let is_cmyk = matches!(profile, SourceProfile::CmykIcc(_));
            let threshold: u8 = if is_cmyk { 17 } else { 1 };

            // Snapshot the frame data before transforms
            let row_bytes = frame.w() as usize * frame.t_per_pixel();
            let h = frame.h() as usize;
            let mut snapshot = Vec::with_capacity(row_bytes * h);
            for scanline in frame.scanlines() {
                snapshot.extend_from_slice(scanline.row());
            }

            // Run moxcms
            MoxcmsTransformCache::transform_to_srgb(frame, profile)?;

            // Capture moxcms result
            let mut moxcms_result = Vec::with_capacity(row_bytes * h);
            for scanline in frame.scanlines() {
                moxcms_result.extend_from_slice(scanline.row());
            }

            // Restore original and run lcms2
            {
                let mut src_offset = 0;
                for mut scanline in frame.scanlines() {
                    let row = scanline.row_mut();
                    row.copy_from_slice(&snapshot[src_offset..src_offset + row.len()]);
                    src_offset += row.len();
                }
            }
            // lcms2 may panic on unsupported pixel formats (e.g. gray ICC with BGRA frame).
            // Catch panics so a single bad profile doesn't break subsequent images.
            let lcms2_result_or_panic =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    Lcms2TransformCache::transform_to_srgb(frame, profile)
                }));

            match lcms2_result_or_panic {
                Ok(Ok(())) => {
                    // Capture lcms2 result and compare
                    let mut lcms2_result = Vec::with_capacity(row_bytes * h);
                    for scanline in frame.scanlines() {
                        lcms2_result.extend_from_slice(scanline.row());
                    }
                    compare_results(&moxcms_result, &lcms2_result, threshold, is_cmyk, profile);
                }
                Ok(Err(e)) => {
                    eprintln!("[CMS Both] lcms2 error (moxcms result used): {e}");
                }
                Err(_) => {
                    eprintln!("[CMS Both] lcms2 panicked (moxcms result used)");
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
    }
}

/// Compare two frame buffers and log warnings for any channel divergence exceeding the threshold.
fn compare_results(
    moxcms: &[u8],
    lcms2: &[u8],
    max_diff: u8,
    is_cmyk: bool,
    profile: &SourceProfile,
) {
    if moxcms.len() != lcms2.len() {
        eprintln!(
            "[CMS Both] WARNING: buffer length mismatch: moxcms={}, lcms2={}",
            moxcms.len(),
            lcms2.len()
        );
        return;
    }

    let total_pixels = moxcms.len() / 4;
    let profile_type = if is_cmyk { "CMYK" } else { "RGB" };

    // Per-channel stats
    let mut max_per_channel = [0u8; 4];
    let mut sum_per_channel = [0u64; 4]; // sum of diffs for mean calculation
    let mut divergent_pixels = 0u64;
    let mut max_observed = 0u8;
    // Histogram: count of pixels at each max-channel-diff level (0..=255)
    let mut diff_histogram = [0u64; 256];
    // Track worst pixel (highest single-channel diff)
    let mut worst_pixel: Option<(usize, [u8; 4], [u8; 4], u8)> = None;

    for (i, (m_pixel, l_pixel)) in moxcms.chunks_exact(4).zip(lcms2.chunks_exact(4)).enumerate() {
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
                worst_pixel = Some((i, m, l, pixel_max_diff));
            }
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
    let p50 = p50.unwrap_or(0);
    let p95 = p95.unwrap_or(0);
    let p99 = p99.unwrap_or(0);

    let n = total_pixels as f64;
    let mean_b = sum_per_channel[0] as f64 / n;
    let mean_g = sum_per_channel[1] as f64 / n;
    let mean_r = sum_per_channel[2] as f64 / n;

    let desc = profile.describe();
    eprintln!(
        "[CMS Both] WARNING: {profile_type} {divergent_pixels}/{total_pixels} diverge >{max_diff} \
         (max={max_observed} p99={p99} p95={p95} p50={p50} mean_bgr={mean_b:.2},{mean_g:.2},{mean_r:.2} \
         ch_max=B{}/G{}/R{}/A{}) profile={desc}",
        max_per_channel[0],
        max_per_channel[1],
        max_per_channel[2],
        max_per_channel[3],
    );
    if let Some((idx, m, l, diff)) = worst_pixel {
        eprintln!(
            "[CMS Both]   worst pixel #{idx} (diff={diff}): \
             moxcms=[{},{},{},{}] lcms2=[{},{},{},{}]",
            m[0], m[1], m[2], m[3], l[0], l[1], l[2], l[3]
        );
    }
}
