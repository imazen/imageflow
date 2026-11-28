//! Static gallery generator and security-hardened interactive gallery designer studio.
//!
//! Static export (`gallery build`) and live designer preview (`gallery serve`) share the
//! exact same [`render::render_gallery_html`] engine so WYSIWYG previews match exported
//! static HTML pixel-for-pixel and layout-for-layout.

pub mod render;
pub mod server;

use crate::bulk::engine::{
    BulkProgressEvent, BulkRunReport, discover_media_files, publish_staged_files_atomic, run_bulk,
};
use crate::bulk::planner::render_picture_snippet;
use crate::bulk::schema::{
    BulkConfig, BulkLimits, FailurePolicy, GALLERY_SCHEMA_VERSION, GalleryConfig,
    GalleryItemOverride, GallerySortOrder, ManifestAsset, OutputPathLayout, QualityFallbackPolicy,
    ResponsivePreset,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, Serialize)]
pub struct GalleryBuildReport {
    pub index_html_path: PathBuf,
    pub gallery_json_path: PathBuf,
    pub config_toml_path: PathBuf,
    pub snippets_html_path: PathBuf,
    pub visible_items: usize,
    pub hidden_items: usize,
    pub bulk_report: BulkRunReport,
}

/// Resolved gallery item combining encoded asset metadata with curation overrides.
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedGalleryItem {
    pub id: String,
    pub source_relative_path: String,
    pub title: String,
    pub caption: String,
    pub alt_text: String,
    pub featured: bool,
    pub media_kind: String,
    pub source_format: String,
    pub original_width: u32,
    pub original_height: u32,
    pub aspect_ratio: f64,
    pub source_bytes: u64,
    pub smallest_variant_bytes: u64,
    pub savings_pct: f64,
    pub best_verified_score: Option<f64>,
    pub best_verified_metric: Option<String>,
    pub variants: Vec<crate::bulk::schema::ManifestVariant>,
    pub video_variants: Vec<crate::bulk::schema::ManifestVariant>,
    pub picture_html: String,
    pub download_href: String,
}

/// Generate a starter `gallery.toml` by scanning `input_dir` for supported media.
pub fn init_gallery_config(
    input_dir: &Path,
    title: Option<String>,
    preset_name: &str,
) -> Result<GalleryConfig, String> {
    let encoding = ResponsivePreset::from_preset_name(preset_name)?;
    let discovered = discover_media_files(&[input_dir.to_path_buf()], None, true, &[], &[], false)?;
    let inferred_title = title.unwrap_or_else(|| {
        input_dir
            .file_name()
            .and_then(|n| n.to_str())
            .filter(|s| !s.is_empty() && *s != ".")
            .map(|s| s.replace(['-', '_'], " "))
            .unwrap_or_else(|| "Photo Gallery".to_string())
    });
    let mut items = BTreeMap::new();
    for (idx, d) in discovered.iter().enumerate() {
        let stem = Path::new(&d.relative_key)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&d.relative_key)
            .replace(['-', '_'], " ");
        items.insert(
            d.relative_key.clone(),
            GalleryItemOverride {
                title: Some(stem),
                caption: None,
                alt_text: None,
                hidden: false,
                featured: idx == 0,
                order: Some(idx as u32),
            },
        );
    }
    Ok(GalleryConfig {
        schema: GALLERY_SCHEMA_VERSION.to_string(),
        title: inferred_title,
        subtitle: Some(format!("{} photos processed with Imageflow", discovered.len())),
        author: None,
        footer_text: Some("Built with Imageflow — color-managed responsive media".to_string()),
        sort_by: GallerySortOrder::FilenameAsc,
        design: Default::default(),
        encoding,
        items,
    })
}

/// Apply curation overrides, visibility filtering, and sort order to encoded manifest assets.
pub fn resolve_gallery_items(
    config: &GalleryConfig,
    assets: &[ManifestAsset],
) -> (Vec<ResolvedGalleryItem>, usize) {
    let mut hidden_count = 0usize;
    let mut resolved: Vec<(Option<u32>, ResolvedGalleryItem)> = Vec::new();

    for asset in assets {
        let ov = config.items.get(&asset.source_relative_path);
        if ov.is_some_and(|o| o.hidden) {
            hidden_count += 1;
            continue;
        }
        let default_title = Path::new(&asset.source_relative_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&asset.source_relative_path)
            .replace(['-', '_'], " ");
        let title = ov
            .and_then(|o| o.title.clone())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(default_title);
        let caption = ov.and_then(|o| o.caption.clone()).unwrap_or_default();
        let alt_text = ov
            .and_then(|o| o.alt_text.clone())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| title.clone());
        let featured = ov.is_some_and(|o| o.featured);
        let order = ov.and_then(|o| o.order);

        let smallest_variant_bytes = asset
            .variants
            .iter()
            .filter(|v| v.width == asset.variants.iter().map(|x| x.width).max().unwrap_or(0))
            .map(|v| v.bytes)
            .min()
            .unwrap_or(0);
        let savings_pct = if asset.source_bytes > 0 && smallest_variant_bytes > 0 {
            ((asset.source_bytes as f64 - smallest_variant_bytes as f64)
                / asset.source_bytes as f64)
                * 100.0
        } else {
            0.0
        };
        let best_verified = asset
            .variants
            .iter()
            .filter_map(|v| Some((v.verified_metric.clone()?, v.verified_score?)))
            .max_by(|a, b| a.1.total_cmp(&b.1));
        let download_href = asset
            .variants
            .iter()
            .max_by_key(|v| (v.width, usize::from(v.format == crate::bulk::OutputFormat::Jpeg)))
            .map(|v| v.relative_path.clone())
            .unwrap_or_default();
        let aspect_ratio =
            f64::from(asset.original_width.max(1)) / f64::from(asset.original_height.max(1));
        let picture_html = render_picture_snippet(
            asset,
            "",
            "(max-width: 640px) 100vw, (max-width: 1100px) 50vw, 33vw",
            &alt_text,
            featured,
        );

        resolved.push((
            order,
            ResolvedGalleryItem {
                id: asset.id.clone(),
                source_relative_path: asset.source_relative_path.clone(),
                title,
                caption,
                alt_text,
                featured,
                media_kind: asset.media_kind.clone(),
                source_format: asset.source_format.clone(),
                original_width: asset.original_width,
                original_height: asset.original_height,
                aspect_ratio,
                source_bytes: asset.source_bytes,
                smallest_variant_bytes,
                savings_pct,
                best_verified_metric: best_verified.as_ref().map(|(m, _)| m.clone()),
                best_verified_score: best_verified.as_ref().map(|(_, s)| *s),
                variants: asset.variants.clone(),
                video_variants: asset.video_variants.clone(),
                picture_html,
                download_href,
            },
        ));
    }

    match config.sort_by {
        GallerySortOrder::FilenameAsc => {
            resolved.sort_by(|a, b| a.1.source_relative_path.cmp(&b.1.source_relative_path));
        }
        GallerySortOrder::FilenameDesc => {
            resolved.sort_by(|a, b| b.1.source_relative_path.cmp(&a.1.source_relative_path));
        }
        GallerySortOrder::AspectRatio => {
            resolved.sort_by(|a, b| {
                b.1.aspect_ratio
                    .total_cmp(&a.1.aspect_ratio)
                    .then_with(|| a.1.source_relative_path.cmp(&b.1.source_relative_path))
            });
        }
        GallerySortOrder::Custom => {
            resolved.sort_by(|a, b| {
                a.0.unwrap_or(u32::MAX)
                    .cmp(&b.0.unwrap_or(u32::MAX))
                    .then_with(|| a.1.source_relative_path.cmp(&b.1.source_relative_path))
            });
        }
    }

    (resolved.into_iter().map(|(_, item)| item).collect(), hidden_count)
}

/// Build a complete, self-contained static gallery (`assets/`, `gallery.json`, `snippets.html`,
/// `gallery.toml`, and `index.html`) published atomically.
#[allow(clippy::too_many_arguments)]
pub fn build_gallery(
    input_dir: &Path,
    output_dir: &Path,
    config: &GalleryConfig,
    concurrency: usize,
    force_rebuild: bool,
    limits: BulkLimits,
    cancel: Option<Arc<AtomicBool>>,
    on_progress: Option<Arc<dyn Fn(BulkProgressEvent) + Send + Sync>>,
) -> Result<GalleryBuildReport, String> {
    config.validate()?;
    let on_error = if config.encoding.quality_fallback == QualityFallbackPolicy::StrictVerify {
        FailurePolicy::FailFast
    } else {
        FailurePolicy::Continue
    };
    let bulk_cfg = BulkConfig {
        schema: crate::bulk::BULK_SCHEMA_VERSION.to_string(),
        inputs: vec![input_dir.to_path_buf()],
        output_dir: output_dir.to_path_buf(),
        recursive: true,
        include_globs: Vec::new(),
        exclude_globs: Vec::new(),
        follow_symlinks: false,
        output_layout: OutputPathLayout::ContentAddressed,
        concurrency: concurrency.clamp(1, 64),
        incremental_cache: true,
        force_rebuild,
        on_error,
        write_manifest: false,
        write_snippets: true,
        preset: config.encoding.clone(),
        limits,
    };

    let bulk_report = run_bulk(&bulk_cfg, cancel, on_progress)?;
    let (visible_items, hidden_items) = resolve_gallery_items(config, &bulk_report.manifest.assets);
    if bulk_report.failed > 0 && visible_items.is_empty() {
        return Err(format!(
            "gallery build produced zero valid items ({} failed): {}",
            bulk_report.failed,
            bulk_report.errors.join("; ")
        ));
    }

    let gallery_payload = serde_json::json!({
        "schema": GALLERY_SCHEMA_VERSION,
        "generated_at_utc": bulk_report.manifest.generated_at_utc,
        "title": config.title,
        "subtitle": config.subtitle,
        "author": config.author,
        "footer_text": config.footer_text,
        "design": config.design,
        "encoding": config.encoding,
        "total_input_bytes": bulk_report.total_input_bytes,
        "total_output_bytes": bulk_report.total_output_bytes,
        "items": visible_items,
    });

    // Stage config, gallery.json, and index.html in one coordinated atomic batch (index.html published LAST!).
    let config_toml_path = output_dir.join("gallery.toml");
    let config_toml_bytes = config.to_toml_string()?.into_bytes();

    let gallery_json_path = output_dir.join("gallery.json");
    let gallery_json_bytes = serde_json::to_vec_pretty(&gallery_payload)
        .map_err(|e| format!("failed serializing gallery.json: {e}"))?;

    let index_html_path = output_dir.join("index.html");
    let index_html_bytes = render::render_gallery_html(config, &visible_items, false).into_bytes();

    publish_staged_files_atomic(
        output_dir,
        &[
            (config_toml_path.clone(), config_toml_bytes),
            (gallery_json_path.clone(), gallery_json_bytes),
            (index_html_path.clone(), index_html_bytes),
        ],
    )?;

    let snippets_html_path = bulk_report
        .snippets_path
        .clone()
        .unwrap_or_else(|| output_dir.join("snippets.html"));

    Ok(GalleryBuildReport {
        index_html_path,
        gallery_json_path,
        config_toml_path,
        snippets_html_path,
        visible_items: visible_items.len(),
        hidden_items,
        bulk_report,
    })
}
