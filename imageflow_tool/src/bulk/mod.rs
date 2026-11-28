//! Bulk responsive media processing engine, shared JSON/TOML schemas, single-decode
//! multi-output `zenpipe` DAG planner, incremental content-hash cache, and `<picture>`/`srcset`
//! HTML snippet generator.

pub mod engine;
pub mod planner;
pub mod schema;

pub use engine::{
    BulkProgressEvent, BulkRunReport, DiscoveredInput, discover_media_files, plan_bulk_dry_run,
    run_bulk, write_file_atomic,
};
pub use planner::{
    PlannedBreakpoint, PlannedOutputSlot, ResolvedCoding, build_still_fanout_envelope,
    build_video_poster_envelope, build_video_transcode_envelope, plan_breakpoints,
    render_picture_snippet, resolve_variant_coding,
};
pub use schema::{
    AspectRatio, BULK_SCHEMA_VERSION, BulkConfig, BulkLimits, BulkManifest, EncodeEffort,
    FailurePolicy, FitMode, GALLERY_SCHEMA_VERSION, GalleryCaptionStyle, GalleryConfig,
    GalleryDesign, GalleryFontFamily, GalleryItemOverride, GalleryLayout, GallerySortOrder,
    GalleryTheme, MANIFEST_SCHEMA_VERSION, ManifestAsset, ManifestVariant, MetadataRetention,
    OutputFormat, OutputPathLayout, QualityFallbackPolicy, QualityProfileLevel, QualityTarget,
    ResponsivePreset, WorkspaceAdmission,
};
