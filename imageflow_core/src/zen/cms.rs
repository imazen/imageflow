//! Color management: thin wrapper around zencodecs::cms.
//!
//! Delegates ICC profile detection, sRGB matching, and profile synthesis
//! to zencodecs::cms. This module only adds the zenpipe Source wrapping.

use super::execute::ZenError;

// ─── Transform application ───

/// Apply ICC→sRGB transform if the source image has a non-sRGB ICC profile.
pub(super) fn apply_icc_transform(
    source: Box<dyn zenpipe::Source>,
    info: &zencodecs::ImageInfo,
    cms_mode: imageflow_types::CmsMode,
) -> Result<Box<dyn zenpipe::Source>, ZenError> {
    let zen_mode = match cms_mode {
        imageflow_types::CmsMode::Imageflow2Compat => zencodecs::CmsMode::Compat,
        imageflow_types::CmsMode::SceneReferred => zencodecs::CmsMode::SceneReferred,
    };

    let transform_icc = zencodecs::cms::srgb_transform_icc(&info.source_color, None, zen_mode);

    let Some((src_icc, dst_icc)) = transform_icc else {
        return Ok(source); // Already sRGB
    };

    apply_icc_to_source(source, &src_icc, &dst_icc)
}

/// Parse gAMA/cHRM/cICP from raw PNG bytes, synthesize ICC, and apply transform.
pub(super) fn apply_png_gamma_transform(
    source: Box<dyn zenpipe::Source>,
    png_data: &[u8],
    honor_gama_only: bool,
) -> Result<Box<dyn zenpipe::Source>, ZenError> {
    let transform_icc = zencodecs::cms::png_srgb_transform_icc_ex(png_data, honor_gama_only);

    let Some((src_icc, dst_icc)) = transform_icc else {
        return Ok(source);
    };

    apply_icc_to_source(source, &src_icc, &dst_icc)
}

/// Wrap a source with a format conversion to RGBA8 sRGB if needed.
pub(super) fn ensure_srgb_rgba8(
    source: Box<dyn zenpipe::Source>,
) -> Result<Box<dyn zenpipe::Source>, ZenError> {
    let src_format = source.format();
    let target = zenpipe::format::RGBA8_SRGB;

    if src_format == target {
        return Ok(source);
    }
    if let Some(converter) = zenpipe::ops::RowConverterOp::new(src_format, target) {
        let transform =
            zenpipe::sources::TransformSource::new(source).push_boxed(Box::new(converter));
        Ok(Box::new(transform))
    } else {
        Ok(source)
    }
}

// ─── Internal ───

fn apply_icc_to_source(
    source: Box<dyn zenpipe::Source>,
    src_icc: &[u8],
    dst_icc: &[u8],
) -> Result<Box<dyn zenpipe::Source>, ZenError> {
    let src_format = source.format();
    let pixel_format = src_format.pixel_format();

    use zenpipe::ColorManagement as _;
    let transform =
        zenpipe::MoxCms.build_transform_for_format(src_icc, dst_icc, pixel_format, pixel_format);

    match transform {
        Ok(row_transform) => {
            let dst_arc: std::sync::Arc<[u8]> = std::sync::Arc::from(dst_icc.to_vec());
            let transformed = zenpipe::sources::IccTransformSource::from_transform(
                source,
                row_transform,
                dst_arc,
            );
            Ok(Box::new(transformed))
        }
        Err(_) => Ok(source), // Transform not possible — pass through
    }
}
