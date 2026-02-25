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
    Both { max_diff_per_channel: u8 },
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
        CmsBackend::Both { max_diff_per_channel } => {
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
            Lcms2TransformCache::transform_to_srgb(frame, profile)?;

            // Capture lcms2 result
            let mut lcms2_result = Vec::with_capacity(row_bytes * h);
            for scanline in frame.scanlines() {
                lcms2_result.extend_from_slice(scanline.row());
            }

            // Compare
            compare_results(&moxcms_result, &lcms2_result, max_diff_per_channel);

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
fn compare_results(moxcms: &[u8], lcms2: &[u8], max_diff: u8) {
    if moxcms.len() != lcms2.len() {
        eprintln!(
            "[CMS Both] WARNING: buffer length mismatch: moxcms={}, lcms2={}",
            moxcms.len(),
            lcms2.len()
        );
        return;
    }

    let mut max_observed = 0u8;
    let mut divergent_pixels = 0u64;
    let total_pixels = moxcms.len() / 4;

    for (i, (a, b)) in moxcms.iter().zip(lcms2.iter()).enumerate() {
        let diff = a.abs_diff(*b);
        if diff > max_diff {
            divergent_pixels += 1;
            if diff > max_observed {
                max_observed = diff;
            }
        }
    }

    if divergent_pixels > 0 {
        let divergent_pixel_count = divergent_pixels / 4; // 4 channels per pixel
        eprintln!(
            "[CMS Both] WARNING: {}/{} pixels diverge by more than {} (max observed: {})",
            divergent_pixel_count, total_pixels, max_diff, max_observed
        );
    }
}
