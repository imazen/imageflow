using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Imageflow.Media;

/// <summary>
/// Stable cross-language schema identifiers shared identically by Rust <c>imageflow_tool</c>,
/// the interactive gallery designer studio, and <c>Imageflow.Media</c>.
/// </summary>
public static class MediaSchemaVersions
{
    public const string Bulk = "imageflow-bulk-v1";
    public const string Manifest = "imageflow-manifest-v1";
    public const string Gallery = "imageflow-gallery-v1";
}

/// <summary>Supported still and video output formats.</summary>
[JsonConverter(typeof(OutputFormatJsonConverter))]
public enum OutputFormat
{
    Avif,
    Jxl,
    Webp,
    Jpeg,
    Png,
    Mp4Av1,
    WebmVp9,
    WebmVp8,
}

/// <summary>Semantic quality profile levels for <see cref="QualityTarget.QualityProfile"/>.</summary>
[JsonConverter(typeof(QualityProfileLevelJsonConverter))]
public enum QualityProfileLevel
{
    Low,
    Good,
    High,
    VisuallyLossless,
}

/// <summary>
/// Controls how perceptual quality targets behave on formats or workspace policies that do not
/// advertise closed-loop verification.
/// </summary>
[JsonConverter(typeof(QualityFallbackPolicyJsonConverter))]
public enum QualityFallbackPolicy
{
    /// <summary>
    /// Transparently fall back to calibrated quality (or lossless for PNG) ONLY for known
    /// unsupported format x workspace routes, recording both requested and effective coding
    /// plus the exact fallback reason in the manifest.
    /// </summary>
    Calibrated,

    /// <summary>
    /// Refuse up front if any requested lossy format cannot honor closed-loop verification
    /// under the active workspace policy.
    /// </summary>
    StrictVerify,
}

/// <summary>Resize constraint behavior for planned breakpoints.</summary>
[JsonConverter(typeof(FitModeJsonConverter))]
public enum FitMode
{
    Within,
    Fit,
    FitCrop,
    WithinCrop,
    WithinPad,
    FitPad,
}

/// <summary>Encoder speed/compression effort preset.</summary>
[JsonConverter(typeof(EncodeEffortJsonConverter))]
public enum EncodeEffort
{
    Fast,
    Balanced,
    Thorough,
}

/// <summary>Metadata retention policy for encoded outputs.</summary>
[JsonConverter(typeof(MetadataRetentionJsonConverter))]
public enum MetadataRetention
{
    Web,
    Preserve,
    ColorAndRotation,
    StripAll,
}

/// <summary>Workspace admission policy applied to native contexts.</summary>
[JsonConverter(typeof(WorkspaceAdmissionJsonConverter))]
public enum WorkspaceAdmission
{
    BestEffort,
    Strict,
}

/// <summary>Output file naming layout inside the destination directory.</summary>
[JsonConverter(typeof(OutputPathLayoutJsonConverter))]
public enum OutputPathLayout
{
    ContentAddressed,
    PreserveTree,
    Flatten,
}

/// <summary>Batch error handling policy.</summary>
[JsonConverter(typeof(FailurePolicyJsonConverter))]
public enum FailurePolicy
{
    Continue,
    FailFast,
}

/// <summary>Gallery visual layout mode.</summary>
[JsonConverter(typeof(GalleryLayoutJsonConverter))]
public enum GalleryLayout
{
    Justified,
    Masonry,
    Grid,
    Editorial,
    Filmstrip,
}

/// <summary>Gallery color theme preset.</summary>
[JsonConverter(typeof(GalleryThemeJsonConverter))]
public enum GalleryTheme
{
    GalleryDark,
    PaperLight,
    MinimalWhite,
    WarmLinen,
    MidnightSlate,
}

/// <summary>Gallery typography stack.</summary>
[JsonConverter(typeof(GalleryFontFamilyJsonConverter))]
public enum GalleryFontFamily
{
    Sans,
    Serif,
    Mono,
}

/// <summary>Gallery card caption presentation.</summary>
[JsonConverter(typeof(GalleryCaptionStyleJsonConverter))]
public enum GalleryCaptionStyle
{
    Below,
    HoverOverlay,
    LightboxOnly,
    Hidden,
}

/// <summary>Gallery ordering strategy.</summary>
[JsonConverter(typeof(GallerySortOrderJsonConverter))]
public enum GallerySortOrder
{
    FilenameAsc,
    FilenameDesc,
    AspectRatio,
    Custom,
}

/// <summary>Publication commit outcome for an individual batch item.</summary>
public enum BulkCommitState
{
    Committed,
    ReusedFromCache,
    RolledBack,
    FailedBeforeStaging,
}

/// <summary>Quality mode discriminator matching Rust <c>QualityTarget</c> tagged JSON.</summary>
public enum QualityTargetMode
{
    VerifyZensimA,
    AimForZensimA,
    CalibratedQuality,
    QualityProfile,
    Lossless,
}

/// <summary>
/// Immutable, validated perceptual or calibrated quality target matching Rust <c>QualityTarget</c>.
/// </summary>
[JsonConverter(typeof(QualityTargetJsonConverter))]
public sealed record QualityTarget
{
    public QualityTargetMode Mode { get; }
    public float? Target { get; }
    public float? Quality { get; }
    public QualityProfileLevel? ProfileLevel { get; }
    public float Dpr { get; }

    private QualityTarget(
        QualityTargetMode mode,
        float? target = null,
        float? quality = null,
        QualityProfileLevel? profileLevel = null,
        float dpr = 1.0f)
    {
        Mode = mode;
        Target = target;
        Quality = quality;
        ProfileLevel = profileLevel;
        Dpr = dpr;
        Validate();
    }

    public static QualityTarget VerifyZensimA(float target) =>
        new(QualityTargetMode.VerifyZensimA, target: target);

    public static QualityTarget AimForZensimA(float target) =>
        new(QualityTargetMode.AimForZensimA, target: target);

    public static QualityTarget CalibratedQuality(float quality) =>
        new(QualityTargetMode.CalibratedQuality, quality: quality);

    public static QualityTarget QualityProfile(QualityProfileLevel level, float dpr = 1.0f) =>
        new(QualityTargetMode.QualityProfile, profileLevel: level, dpr: dpr);

    public static QualityTarget Lossless() =>
        new(QualityTargetMode.Lossless);

    public void Validate()
    {
        switch (Mode)
        {
            case QualityTargetMode.VerifyZensimA:
            case QualityTargetMode.AimForZensimA:
                if (Target is null || !float.IsFinite(Target.Value) || Target.Value < 20.0f || Target.Value > 99.5f)
                {
                    throw new ArgumentOutOfRangeException(nameof(Target), Target, "zensim_a target must be finite and within 20.0..=99.5.");
                }
                break;
            case QualityTargetMode.CalibratedQuality:
                if (Quality is null || !float.IsFinite(Quality.Value) || Quality.Value < 1.0f || Quality.Value > 100.0f)
                {
                    throw new ArgumentOutOfRangeException(nameof(Quality), Quality, "calibrated_quality must be finite and within 1.0..=100.0.");
                }
                break;
            case QualityTargetMode.QualityProfile:
                if (ProfileLevel is null)
                {
                    throw new ArgumentException("QualityProfile requires a level.");
                }
                if (!float.IsFinite(Dpr) || Dpr < 0.5f || Dpr > 4.0f)
                {
                    throw new ArgumentOutOfRangeException(nameof(Dpr), Dpr, "quality_profile dpr must be finite and within 0.5..=4.0.");
                }
                break;
            case QualityTargetMode.Lossless:
                break;
            default:
                throw new ArgumentOutOfRangeException(nameof(Mode), Mode, "Unknown QualityTargetMode.");
        }
    }

    public string DisplayLabel => Mode switch
    {
        QualityTargetMode.VerifyZensimA => string.Format(CultureInfo.InvariantCulture, "verify(zensim_a>={0:F1})", Target!.Value),
        QualityTargetMode.AimForZensimA => string.Format(CultureInfo.InvariantCulture, "aim_for(zensim_a~{0:F1})", Target!.Value),
        QualityTargetMode.CalibratedQuality => string.Format(CultureInfo.InvariantCulture, "calibrated_quality({0:F1})", Quality!.Value),
        QualityTargetMode.QualityProfile => string.Format(CultureInfo.InvariantCulture, "profile({0}@{1:F1}x)", MediaWireTokens.ToWire(ProfileLevel!.Value), Dpr),
        QualityTargetMode.Lossless => "lossless",
        _ => "unknown",
    };
}

/// <summary>Validated integer aspect ratio (1..=100 per component).</summary>
public sealed record AspectRatio
{
    [JsonPropertyName("width")]
    public int Width { get; init; }

    [JsonPropertyName("height")]
    public int Height { get; init; }

    [JsonConstructor]
    public AspectRatio(int width, int height)
    {
        if (width <= 0 || height <= 0 || width > 100 || height > 100)
        {
            throw new ArgumentOutOfRangeException(nameof(width), $"Aspect ratio components must be 1..=100 (got {width}:{height}).");
        }
        Width = width;
        Height = height;
    }

    [JsonIgnore]
    public string CssString => string.Create(CultureInfo.InvariantCulture, $"{Width} / {Height}");
}

/// <summary>Per-job resource limits enforced by Imageflow contexts and envelopes.</summary>
public sealed record BulkLimits
{
    [JsonPropertyName("max_memory_bytes")]
    public ulong MaxMemoryBytes { get; init; } = 1024UL * 1024UL * 1024UL;

    [JsonPropertyName("timeout_ms")]
    public ulong TimeoutMs { get; init; } = 120_000UL;

    [JsonPropertyName("max_dimension")]
    public int MaxDimension { get; init; } = 16_384;

    public static BulkLimits Default => new();
}

/// <summary>
/// Reusable, immutable responsive encoding preset shared identically across Rust CLI,
/// interactive gallery designer, and NativeAOT <c>Imageflow.Media</c>.
/// </summary>
public sealed record ResponsivePreset
{
    [JsonPropertyName("widths_px")]
    public IReadOnlyList<int> WidthsPx { get; init; } = new[] { 400, 800, 1600 };

    [JsonPropertyName("formats")]
    public IReadOnlyList<OutputFormat> Formats { get; init; } = new[] { OutputFormat.Avif, OutputFormat.Webp, OutputFormat.Jpeg };

    [JsonPropertyName("quality")]
    public QualityTarget Quality { get; init; } = QualityTarget.VerifyZensimA(85.0f);

    [JsonPropertyName("quality_fallback")]
    public QualityFallbackPolicy QualityFallback { get; init; } = QualityFallbackPolicy.Calibrated;

    [JsonPropertyName("fit_mode")]
    public FitMode FitMode { get; init; } = FitMode.Within;

    [JsonPropertyName("aspect_ratio")]
    public AspectRatio? AspectRatio { get; init; }

    [JsonPropertyName("allow_upscale")]
    public bool AllowUpscale { get; init; }

    [JsonPropertyName("effort")]
    public EncodeEffort Effort { get; init; } = EncodeEffort.Balanced;

    [JsonPropertyName("metadata")]
    public MetadataRetention Metadata { get; init; } = MetadataRetention.Web;

    [JsonPropertyName("sharpen_amount")]
    public float? SharpenAmount { get; init; }

    [JsonPropertyName("workspace_policy")]
    public WorkspaceAdmission WorkspacePolicy { get; init; } = WorkspaceAdmission.BestEffort;

    [JsonPropertyName("generate_video_posters")]
    public bool GenerateVideoPosters { get; init; } = true;

    [JsonPropertyName("video_formats")]
    public IReadOnlyList<OutputFormat> VideoFormats { get; init; } = Array.Empty<OutputFormat>();

    public static ResponsivePreset WebResponsive() => new();

    public static ResponsivePreset GalleryHighQuality() => new ResponsivePreset
    {
        WidthsPx = new[] { 480, 960, 1600, 2400 },
        Formats = new[] { OutputFormat.Avif, OutputFormat.Webp, OutputFormat.Jpeg },
        Quality = QualityTarget.VerifyZensimA(90.0f),
    };

    public static ResponsivePreset ThumbnailGrid() => new ResponsivePreset
    {
        WidthsPx = new[] { 320, 640 },
        Formats = new[] { OutputFormat.Avif, OutputFormat.Webp, OutputFormat.Jpeg },
        Quality = QualityTarget.VerifyZensimA(82.0f),
        FitMode = FitMode.FitCrop,
        AspectRatio = new AspectRatio(1, 1),
    };

    public static ResponsivePreset CompactMobile() => new ResponsivePreset
    {
        WidthsPx = new[] { 320, 640, 1024 },
        Formats = new[] { OutputFormat.Avif, OutputFormat.Webp, OutputFormat.Jpeg },
        Quality = QualityTarget.VerifyZensimA(76.0f),
    };

    public static ResponsivePreset Archival() => new ResponsivePreset
    {
        WidthsPx = new[] { 4096 },
        Formats = new[] { OutputFormat.Avif, OutputFormat.Jpeg },
        Quality = QualityTarget.VerifyZensimA(94.0f),
        Metadata = MetadataRetention.Preserve,
    };

    public static ResponsivePreset SocialCards() => new ResponsivePreset
    {
        WidthsPx = new[] { 1200 },
        Formats = new[] { OutputFormat.Jpeg, OutputFormat.Webp },
        Quality = QualityTarget.VerifyZensimA(86.0f),
        FitMode = FitMode.FitCrop,
        AspectRatio = new AspectRatio(40, 21),
    };

    public static ResponsivePreset FromPresetName(string name) => name.Trim().ToLowerInvariant() switch
    {
        "web-responsive" or "web_responsive" or "web" => WebResponsive(),
        "gallery-hq" or "gallery_hq" or "gallery" => GalleryHighQuality(),
        "thumb-grid" or "thumb_grid" or "thumbs" => ThumbnailGrid(),
        "compact-mobile" or "compact_mobile" or "compact" => CompactMobile(),
        "archival" => Archival(),
        "social-cards" or "social_cards" or "social" => SocialCards(),
        _ => throw new ArgumentException($"Unknown preset '{name}'.", nameof(name)),
    };

    public ResponsivePreset WithWidths(params int[] widthsPx) => this with { WidthsPx = widthsPx.ToArray() };
    public ResponsivePreset WithFormats(params OutputFormat[] formats) => this with { Formats = formats.ToArray() };
    public ResponsivePreset WithQuality(QualityTarget quality) => this with { Quality = quality ?? throw new ArgumentNullException(nameof(quality)) };
    public ResponsivePreset WithQualityFallback(QualityFallbackPolicy fallback) => this with { QualityFallback = fallback };
    public ResponsivePreset WithFitMode(FitMode fitMode) => this with { FitMode = fitMode };
    public ResponsivePreset WithAspectRatio(AspectRatio? aspectRatio) => this with { AspectRatio = aspectRatio };
    public ResponsivePreset WithAllowUpscale(bool allowUpscale) => this with { AllowUpscale = allowUpscale };
    public ResponsivePreset WithEffort(EncodeEffort effort) => this with { Effort = effort };
    public ResponsivePreset WithMetadata(MetadataRetention metadata) => this with { Metadata = metadata };
    public ResponsivePreset WithSharpen(float? amount) => this with { SharpenAmount = amount };
    public ResponsivePreset WithWorkspacePolicy(WorkspaceAdmission policy) => this with { WorkspacePolicy = policy };
    public ResponsivePreset WithVideoFormats(params OutputFormat[] videoFormats) => this with { VideoFormats = videoFormats.ToArray() };

    public void Validate()
    {
        if (WidthsPx is null || WidthsPx.Count == 0)
        {
            throw new ArgumentException("WidthsPx must contain at least one positive width.");
        }
        foreach (int w in WidthsPx)
        {
            if (w <= 0 || w > 16_384)
            {
                throw new ArgumentOutOfRangeException(nameof(WidthsPx), w, $"Width {w} is out of range 1..=16384.");
            }
        }
        if (Formats is null || Formats.Count == 0)
        {
            throw new ArgumentException("Formats must contain at least one still output format.");
        }
        foreach (OutputFormat fmt in Formats)
        {
            if (MediaWireTokens.IsVideo(fmt))
            {
                throw new ArgumentException($"Still formats list must not contain video format '{fmt}'.");
            }
        }
        if (VideoFormats is not null)
        {
            foreach (OutputFormat vf in VideoFormats)
            {
                if (!MediaWireTokens.IsVideo(vf))
                {
                    throw new ArgumentException($"VideoFormats must only contain video formats (got '{vf}').");
                }
            }
        }
        if (SharpenAmount is float s && (!float.IsFinite(s) || s < 0.0f || s > 10.0f))
        {
            throw new ArgumentOutOfRangeException(nameof(SharpenAmount), s, "SharpenAmount must be finite within 0.0..=10.0.");
        }
        Quality.Validate();
    }

    public string ToJson() => JsonSerializer.Serialize(this, MediaBulkJsonContext.Default.ResponsivePreset);

    public static ResponsivePreset FromJson(string json)
    {
        ResponsivePreset? preset = JsonSerializer.Deserialize(json, MediaBulkJsonContext.Default.ResponsivePreset);
        if (preset is null)
        {
            throw new JsonException("ResponsivePreset JSON deserialized to null.");
        }
        preset.Validate();
        return preset;
    }
}

/// <summary>One emitted variant file recorded in a responsive asset manifest.</summary>
public sealed record ManifestVariant
{
    [JsonPropertyName("relative_path")]
    public required string RelativePath { get; init; }

    [JsonPropertyName("format")]
    public required OutputFormat Format { get; init; }

    [JsonPropertyName("mime_type")]
    public required string MimeType { get; init; }

    [JsonPropertyName("width")]
    public required int Width { get; init; }

    [JsonPropertyName("height")]
    public required int Height { get; init; }

    [JsonPropertyName("bytes")]
    public required long Bytes { get; init; }

    [JsonPropertyName("requested_coding")]
    public required string RequestedCoding { get; init; }

    [JsonPropertyName("effective_coding")]
    public required string EffectiveCoding { get; init; }

    [JsonPropertyName("fallback_reason")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? FallbackReason { get; init; }

    [JsonPropertyName("verified_metric")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? VerifiedMetric { get; init; }

    [JsonPropertyName("verified_score")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public double? VerifiedScore { get; init; }
}

/// <summary>One processed source media asset and all its emitted responsive variants.</summary>
public sealed record ManifestAsset
{
    [JsonPropertyName("id")]
    public required string Id { get; init; }

    [JsonPropertyName("source_relative_path")]
    public required string SourceRelativePath { get; init; }

    [JsonPropertyName("source_sha256")]
    public required string SourceSha256 { get; init; }

    [JsonPropertyName("source_bytes")]
    public required long SourceBytes { get; init; }

    [JsonPropertyName("media_kind")]
    public required string MediaKind { get; init; }

    [JsonPropertyName("source_format")]
    public required string SourceFormat { get; init; }

    [JsonPropertyName("original_width")]
    public required int OriginalWidth { get; init; }

    [JsonPropertyName("original_height")]
    public required int OriginalHeight { get; init; }

    [JsonPropertyName("variants")]
    public required IReadOnlyList<ManifestVariant> Variants { get; init; }

    [JsonPropertyName("video_variants")]
    public IReadOnlyList<ManifestVariant> VideoVariants { get; init; } = Array.Empty<ManifestVariant>();

    [JsonPropertyName("picture_html")]
    public required string PictureHtml { get; init; }
}

/// <summary>Complete machine-readable responsive asset manifest (<c>imageflow-manifest-v1</c>).</summary>
public sealed record BulkManifest
{
    [JsonPropertyName("schema")]
    public string Schema { get; init; } = MediaSchemaVersions.Manifest;

    [JsonPropertyName("generated_at_utc")]
    public required string GeneratedAtUtc { get; init; }

    [JsonPropertyName("generator")]
    public required string Generator { get; init; }

    [JsonPropertyName("preset")]
    public required ResponsivePreset Preset { get; init; }

    [JsonPropertyName("total_input_bytes")]
    public long TotalInputBytes { get; init; }

    [JsonPropertyName("total_output_bytes")]
    public long TotalOutputBytes { get; init; }

    [JsonPropertyName("assets")]
    public required IReadOnlyList<ManifestAsset> Assets { get; init; }

    public string ToJson() => JsonSerializer.Serialize(this, MediaBulkJsonContext.Default.BulkManifest);

    public static BulkManifest FromJson(string json) =>
        JsonSerializer.Deserialize(json, MediaBulkJsonContext.Default.BulkManifest)
        ?? throw new JsonException("BulkManifest JSON deserialized to null.");
}

/// <summary>Gallery visual design settings matching Rust <c>GalleryDesign</c>.</summary>
public sealed record GalleryDesign
{
    [JsonPropertyName("layout")]
    public GalleryLayout Layout { get; init; } = GalleryLayout.Justified;

    [JsonPropertyName("theme")]
    public GalleryTheme Theme { get; init; } = GalleryTheme.GalleryDark;

    [JsonPropertyName("font_family")]
    public GalleryFontFamily FontFamily { get; init; } = GalleryFontFamily.Sans;

    [JsonPropertyName("target_row_height_px")]
    public int TargetRowHeightPx { get; init; } = 280;

    [JsonPropertyName("columns_desktop")]
    public int ColumnsDesktop { get; init; } = 3;

    [JsonPropertyName("columns_tablet")]
    public int ColumnsTablet { get; init; } = 2;

    [JsonPropertyName("columns_mobile")]
    public int ColumnsMobile { get; init; } = 1;

    [JsonPropertyName("grid_aspect_ratio")]
    public AspectRatio? GridAspectRatio { get; init; } = new(4, 3);

    [JsonPropertyName("gap_px")]
    public int GapPx { get; init; } = 14;

    [JsonPropertyName("corner_radius_px")]
    public int CornerRadiusPx { get; init; } = 8;

    [JsonPropertyName("max_width_px")]
    public int MaxWidthPx { get; init; } = 1440;

    [JsonPropertyName("padding_px")]
    public int PaddingPx { get; init; } = 32;

    [JsonPropertyName("caption_style")]
    public GalleryCaptionStyle CaptionStyle { get; init; } = GalleryCaptionStyle.Below;

    [JsonPropertyName("show_exif_badges")]
    public bool ShowExifBadges { get; init; } = true;

    [JsonPropertyName("show_download_button")]
    public bool ShowDownloadButton { get; init; } = true;

    [JsonPropertyName("enable_lightbox")]
    public bool EnableLightbox { get; init; } = true;

    [JsonPropertyName("accent_color_hex")]
    public string? AccentColorHex { get; init; }

    public void Validate()
    {
        if (TargetRowHeightPx < 80 || TargetRowHeightPx > 1200)
        {
            throw new ArgumentOutOfRangeException(nameof(TargetRowHeightPx), "TargetRowHeightPx must be 80..=1200.");
        }
        if (ColumnsDesktop < 1 || ColumnsDesktop > 8 || ColumnsTablet < 1 || ColumnsTablet > 6 || ColumnsMobile < 1 || ColumnsMobile > 4)
        {
            throw new ArgumentOutOfRangeException(nameof(ColumnsDesktop), "Column counts out of range.");
        }
        if (GapPx < 0 || GapPx > 64)
        {
            throw new ArgumentOutOfRangeException(nameof(GapPx), "GapPx must be 0..=64.");
        }
        if (CornerRadiusPx < 0 || CornerRadiusPx > 48)
        {
            throw new ArgumentOutOfRangeException(nameof(CornerRadiusPx), "CornerRadiusPx must be 0..=48.");
        }
        if (MaxWidthPx < 320 || MaxWidthPx > 3840)
        {
            throw new ArgumentOutOfRangeException(nameof(MaxWidthPx), "MaxWidthPx must be 320..=3840.");
        }
        if (PaddingPx < 0 || PaddingPx > 128)
        {
            throw new ArgumentOutOfRangeException(nameof(PaddingPx), "PaddingPx must be 0..=128.");
        }
        if (AccentColorHex is not null && !IsValidHexColor(AccentColorHex))
        {
            throw new ArgumentException($"Invalid hex color '{AccentColorHex}' (expected #RGB or #RRGGBB).", nameof(AccentColorHex));
        }
    }

    public static bool IsValidHexColor(string? value)
    {
        if (string.IsNullOrEmpty(value) || (value.Length != 4 && value.Length != 7) || value[0] != '#')
        {
            return false;
        }
        for (int i = 1; i < value.Length; i++)
        {
            char c = value[i];
            bool hex = (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F');
            if (!hex)
            {
                return false;
            }
        }
        return true;
    }
}

/// <summary>Per-photo curation override matching Rust <c>GalleryItemOverride</c>.</summary>
public sealed record GalleryItemOverride
{
    [JsonPropertyName("title")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Title { get; init; }

    [JsonPropertyName("caption")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Caption { get; init; }

    [JsonPropertyName("alt_text")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? AltText { get; init; }

    [JsonPropertyName("hidden")]
    public bool Hidden { get; init; }

    [JsonPropertyName("featured")]
    public bool Featured { get; init; }

    [JsonPropertyName("order")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public uint? Order { get; init; }
}

/// <summary>Complete declarative gallery configuration (<c>imageflow-gallery-v1</c>).</summary>
public sealed record GalleryConfig
{
    [JsonPropertyName("schema")]
    public string Schema { get; init; } = MediaSchemaVersions.Gallery;

    [JsonPropertyName("title")]
    public string Title { get; init; } = "Photo Gallery";

    [JsonPropertyName("subtitle")]
    public string? Subtitle { get; init; }

    [JsonPropertyName("author")]
    public string? Author { get; init; }

    [JsonPropertyName("footer_text")]
    public string? FooterText { get; init; }

    [JsonPropertyName("sort_by")]
    public GallerySortOrder SortBy { get; init; } = GallerySortOrder.FilenameAsc;

    [JsonPropertyName("design")]
    public GalleryDesign Design { get; init; } = new();

    [JsonPropertyName("encoding")]
    public ResponsivePreset Encoding { get; init; } = ResponsivePreset.GalleryHighQuality();

    [JsonPropertyName("items")]
    public Dictionary<string, GalleryItemOverride> Items { get; init; } = new(StringComparer.Ordinal);

    public void Validate()
    {
        if (!string.Equals(Schema, MediaSchemaVersions.Gallery, StringComparison.Ordinal))
        {
            throw new ArgumentException($"Unsupported gallery schema '{Schema}' (expected '{MediaSchemaVersions.Gallery}').");
        }
        if (Title is null || System.Text.Encoding.UTF8.GetByteCount(Title) > 256)
        {
            throw new ArgumentException("Gallery title must be non-null and <= 256 UTF-8 bytes.");
        }
        Design.Validate();
        Encoding.Validate();
    }

    public string ToJson() => JsonSerializer.Serialize(this, MediaBulkJsonContext.Default.GalleryConfig);

    public static GalleryConfig FromJson(string json)
    {
        GalleryConfig? cfg = JsonSerializer.Deserialize(json, MediaBulkJsonContext.Default.GalleryConfig);
        if (cfg is null)
        {
            throw new JsonException("GalleryConfig JSON deserialized to null.");
        }
        cfg.Validate();
        return cfg;
    }
}

/// <summary>Resolved gallery item emitted into <c>gallery.json</c> and rendered HTML.</summary>
public sealed record ResolvedGalleryItem
{
    [JsonPropertyName("id")]
    public required string Id { get; init; }

    [JsonPropertyName("source_relative_path")]
    public required string SourceRelativePath { get; init; }

    [JsonPropertyName("title")]
    public required string Title { get; init; }

    [JsonPropertyName("caption")]
    public required string Caption { get; init; }

    [JsonPropertyName("alt_text")]
    public required string AltText { get; init; }

    [JsonPropertyName("featured")]
    public bool Featured { get; init; }

    [JsonPropertyName("media_kind")]
    public required string MediaKind { get; init; }

    [JsonPropertyName("source_format")]
    public required string SourceFormat { get; init; }

    [JsonPropertyName("original_width")]
    public int OriginalWidth { get; init; }

    [JsonPropertyName("original_height")]
    public int OriginalHeight { get; init; }

    [JsonPropertyName("aspect_ratio")]
    public double AspectRatio { get; init; }

    [JsonPropertyName("source_bytes")]
    public long SourceBytes { get; init; }

    [JsonPropertyName("smallest_variant_bytes")]
    public long SmallestVariantBytes { get; init; }

    [JsonPropertyName("savings_pct")]
    public double SavingsPct { get; init; }

    [JsonPropertyName("best_verified_score")]
    public double? BestVerifiedScore { get; init; }

    [JsonPropertyName("best_verified_metric")]
    public string? BestVerifiedMetric { get; init; }

    [JsonPropertyName("variants")]
    public required IReadOnlyList<ManifestVariant> Variants { get; init; }

    [JsonPropertyName("video_variants")]
    public IReadOnlyList<ManifestVariant> VideoVariants { get; init; } = Array.Empty<ManifestVariant>();

    [JsonPropertyName("picture_html")]
    public required string PictureHtml { get; init; }

    [JsonPropertyName("download_href")]
    public required string DownloadHref { get; init; }
}

/// <summary>Serialized <c>gallery.json</c> payload matching Rust <c>build_gallery</c>.</summary>
public sealed record GalleryOutputDocument
{
    [JsonPropertyName("schema")]
    public string Schema { get; init; } = MediaSchemaVersions.Gallery;

    [JsonPropertyName("generated_at_utc")]
    public required string GeneratedAtUtc { get; init; }

    [JsonPropertyName("title")]
    public required string Title { get; init; }

    [JsonPropertyName("subtitle")]
    public string? Subtitle { get; init; }

    [JsonPropertyName("author")]
    public string? Author { get; init; }

    [JsonPropertyName("footer_text")]
    public string? FooterText { get; init; }

    [JsonPropertyName("design")]
    public required GalleryDesign Design { get; init; }

    [JsonPropertyName("encoding")]
    public required ResponsivePreset Encoding { get; init; }

    [JsonPropertyName("total_input_bytes")]
    public long TotalInputBytes { get; init; }

    [JsonPropertyName("total_output_bytes")]
    public long TotalOutputBytes { get; init; }

    [JsonPropertyName("items")]
    public required IReadOnlyList<ResolvedGalleryItem> Items { get; init; }
}

/// <summary>Per-item progress and outcome record emitted by <see cref="BulkMediaProcessor"/>.</summary>
public sealed record BulkItemOutcome(
    int Index,
    int Total,
    string SourceRelativePath,
    BulkCommitState CommitState,
    int VariantsWritten,
    long InputBytes,
    long OutputBytes,
    string? ErrorMessage,
    ManifestAsset? Asset);

/// <summary>Aggregate summary returned by <see cref="BulkMediaProcessor.ProcessAsync"/>.</summary>
public sealed record BulkProcessingReport(
    int Discovered,
    int Processed,
    int SkippedCached,
    int Failed,
    long TotalInputBytes,
    long TotalOutputBytes,
    TimeSpan Elapsed,
    string? ManifestPath,
    string? SnippetsPath,
    IReadOnlyList<string> Errors,
    BulkManifest Manifest);

/// <summary>Aggregate summary returned by <see cref="GalleryBuilder.BuildAsync"/>.</summary>
public sealed record GalleryBuildReport(
    string IndexHtmlPath,
    string GalleryJsonPath,
    string ConfigJsonPath,
    string SnippetsHtmlPath,
    int VisibleItems,
    int HiddenItems,
    BulkProcessingReport BulkReport);

/// <summary>Internal incremental cache file format matching Rust <c>.imageflow-cache.json</c>.</summary>
internal sealed class BulkCacheDocument
{
    [JsonPropertyName("schema")]
    public string Schema { get; set; } = "imageflow-bulk-cache-v1";

    [JsonPropertyName("engine_fingerprint")]
    public string EngineFingerprint { get; set; } = string.Empty;

    [JsonPropertyName("entries")]
    public Dictionary<string, CachedAssetEntry> Entries { get; set; } = new(StringComparer.Ordinal);
}

internal sealed class CachedAssetEntry
{
    [JsonPropertyName("source_sha256")]
    public string SourceSha256 { get; set; } = string.Empty;

    [JsonPropertyName("recipe_sha256")]
    public string RecipeSha256 { get; set; } = string.Empty;

    [JsonPropertyName("asset")]
    public ManifestAsset? Asset { get; set; }
}

internal static class MediaWireTokens
{
    internal static string Extension(OutputFormat fmt) => fmt switch
    {
        OutputFormat.Avif => "avif",
        OutputFormat.Jxl => "jxl",
        OutputFormat.Webp => "webp",
        OutputFormat.Jpeg => "jpg",
        OutputFormat.Png => "png",
        OutputFormat.Mp4Av1 => "mp4",
        OutputFormat.WebmVp9 or OutputFormat.WebmVp8 => "webm",
        _ => throw new ArgumentOutOfRangeException(nameof(fmt)),
    };

    internal static string WireFormat(OutputFormat fmt) => fmt switch
    {
        OutputFormat.Avif => "avif",
        OutputFormat.Jxl => "jxl",
        OutputFormat.Webp => "webp",
        OutputFormat.Jpeg => "jpeg",
        OutputFormat.Png => "png",
        OutputFormat.Mp4Av1 => "mp4",
        OutputFormat.WebmVp9 or OutputFormat.WebmVp8 => "webm",
        _ => throw new ArgumentOutOfRangeException(nameof(fmt)),
    };

    internal static string? VideoCodec(OutputFormat fmt) => fmt switch
    {
        OutputFormat.Mp4Av1 => "av1",
        OutputFormat.WebmVp9 => "vp9",
        OutputFormat.WebmVp8 => "vp8",
        _ => null,
    };

    internal static string MimeType(OutputFormat fmt) => fmt switch
    {
        OutputFormat.Avif => "image/avif",
        OutputFormat.Jxl => "image/jxl",
        OutputFormat.Webp => "image/webp",
        OutputFormat.Jpeg => "image/jpeg",
        OutputFormat.Png => "image/png",
        OutputFormat.Mp4Av1 => "video/mp4",
        OutputFormat.WebmVp9 or OutputFormat.WebmVp8 => "video/webm",
        _ => throw new ArgumentOutOfRangeException(nameof(fmt)),
    };

    internal static bool IsLosslessOnly(OutputFormat fmt) => fmt == OutputFormat.Png;
    internal static bool IsVideo(OutputFormat fmt) => VideoCodec(fmt) is not null;

    internal static int PictureSourcePriority(OutputFormat fmt) => fmt switch
    {
        OutputFormat.Avif => 0,
        OutputFormat.Jxl => 1,
        OutputFormat.Webp => 2,
        OutputFormat.Jpeg => 3,
        OutputFormat.Png => 4,
        OutputFormat.Mp4Av1 => 5,
        OutputFormat.WebmVp9 => 6,
        OutputFormat.WebmVp8 => 7,
        _ => 99,
    };

    internal static string ToWire(OutputFormat fmt) => fmt switch
    {
        OutputFormat.Avif => "avif",
        OutputFormat.Jxl => "jxl",
        OutputFormat.Webp => "webp",
        OutputFormat.Jpeg => "jpeg",
        OutputFormat.Png => "png",
        OutputFormat.Mp4Av1 => "mp4_av1",
        OutputFormat.WebmVp9 => "webm_vp9",
        OutputFormat.WebmVp8 => "webm_vp8",
        _ => throw new ArgumentOutOfRangeException(nameof(fmt)),
    };

    internal static OutputFormat ParseOutputFormat(string s) => s.Trim().ToLowerInvariant() switch
    {
        "avif" => OutputFormat.Avif,
        "jxl" => OutputFormat.Jxl,
        "webp" => OutputFormat.Webp,
        "jpeg" or "jpg" => OutputFormat.Jpeg,
        "png" => OutputFormat.Png,
        "mp4_av1" or "mp4" or "av1" => OutputFormat.Mp4Av1,
        "webm_vp9" or "webm" or "vp9" => OutputFormat.WebmVp9,
        "webm_vp8" or "vp8" => OutputFormat.WebmVp8,
        _ => throw new JsonException($"Unknown OutputFormat '{s}'."),
    };

    internal static string ToWire(QualityProfileLevel v) => v switch
    {
        QualityProfileLevel.Low => "low",
        QualityProfileLevel.Good => "good",
        QualityProfileLevel.High => "high",
        QualityProfileLevel.VisuallyLossless => "visually_lossless",
        _ => throw new ArgumentOutOfRangeException(nameof(v)),
    };

    internal static QualityProfileLevel ParseQualityProfileLevel(string s) => s switch
    {
        "low" => QualityProfileLevel.Low,
        "good" => QualityProfileLevel.Good,
        "high" => QualityProfileLevel.High,
        "visually_lossless" => QualityProfileLevel.VisuallyLossless,
        _ => throw new JsonException($"Unknown QualityProfileLevel '{s}'."),
    };

    internal static string ToWire(QualityFallbackPolicy v) => v switch
    {
        QualityFallbackPolicy.Calibrated => "calibrated",
        QualityFallbackPolicy.StrictVerify => "strict_verify",
        _ => throw new ArgumentOutOfRangeException(nameof(v)),
    };

    internal static QualityFallbackPolicy ParseQualityFallbackPolicy(string s) => s switch
    {
        "calibrated" => QualityFallbackPolicy.Calibrated,
        "strict_verify" => QualityFallbackPolicy.StrictVerify,
        _ => throw new JsonException($"Unknown QualityFallbackPolicy '{s}'."),
    };

    internal static string ToWire(FitMode v) => v switch
    {
        FitMode.Within => "within",
        FitMode.Fit => "fit",
        FitMode.FitCrop => "fit_crop",
        FitMode.WithinCrop => "within_crop",
        FitMode.WithinPad => "within_pad",
        FitMode.FitPad => "fit_pad",
        _ => throw new ArgumentOutOfRangeException(nameof(v)),
    };

    internal static FitMode ParseFitMode(string s) => s switch
    {
        "within" => FitMode.Within,
        "fit" => FitMode.Fit,
        "fit_crop" => FitMode.FitCrop,
        "within_crop" => FitMode.WithinCrop,
        "within_pad" => FitMode.WithinPad,
        "fit_pad" => FitMode.FitPad,
        _ => throw new JsonException($"Unknown FitMode '{s}'."),
    };

    internal static string ToWire(EncodeEffort v) => v switch
    {
        EncodeEffort.Fast => "fast",
        EncodeEffort.Balanced => "balanced",
        EncodeEffort.Thorough => "thorough",
        _ => throw new ArgumentOutOfRangeException(nameof(v)),
    };

    internal static EncodeEffort ParseEncodeEffort(string s) => s switch
    {
        "fast" => EncodeEffort.Fast,
        "balanced" => EncodeEffort.Balanced,
        "thorough" => EncodeEffort.Thorough,
        _ => throw new JsonException($"Unknown EncodeEffort '{s}'."),
    };

    internal static string ToWire(MetadataRetention v) => v switch
    {
        MetadataRetention.Web => "web",
        MetadataRetention.Preserve => "preserve",
        MetadataRetention.ColorAndRotation => "color_and_rotation",
        MetadataRetention.StripAll => "strip_all",
        _ => throw new ArgumentOutOfRangeException(nameof(v)),
    };

    internal static MetadataRetention ParseMetadataRetention(string s) => s switch
    {
        "web" => MetadataRetention.Web,
        "preserve" => MetadataRetention.Preserve,
        "color_and_rotation" => MetadataRetention.ColorAndRotation,
        "strip_all" => MetadataRetention.StripAll,
        _ => throw new JsonException($"Unknown MetadataRetention '{s}'."),
    };

    internal static string ToWire(WorkspaceAdmission v) => v switch
    {
        WorkspaceAdmission.BestEffort => "best_effort",
        WorkspaceAdmission.Strict => "strict",
        _ => throw new ArgumentOutOfRangeException(nameof(v)),
    };

    internal static WorkspaceAdmission ParseWorkspaceAdmission(string s) => s switch
    {
        "best_effort" => WorkspaceAdmission.BestEffort,
        "strict" => WorkspaceAdmission.Strict,
        _ => throw new JsonException($"Unknown WorkspaceAdmission '{s}'."),
    };

    internal static string ToWire(OutputPathLayout v) => v switch
    {
        OutputPathLayout.ContentAddressed => "content_addressed",
        OutputPathLayout.PreserveTree => "preserve_tree",
        OutputPathLayout.Flatten => "flatten",
        _ => throw new ArgumentOutOfRangeException(nameof(v)),
    };

    internal static OutputPathLayout ParseOutputPathLayout(string s) => s switch
    {
        "content_addressed" => OutputPathLayout.ContentAddressed,
        "preserve_tree" => OutputPathLayout.PreserveTree,
        "flatten" => OutputPathLayout.Flatten,
        _ => throw new JsonException($"Unknown OutputPathLayout '{s}'."),
    };

    internal static string ToWire(FailurePolicy v) => v switch
    {
        FailurePolicy.Continue => "continue",
        FailurePolicy.FailFast => "fail_fast",
        _ => throw new ArgumentOutOfRangeException(nameof(v)),
    };

    internal static FailurePolicy ParseFailurePolicy(string s) => s switch
    {
        "continue" => FailurePolicy.Continue,
        "fail_fast" => FailurePolicy.FailFast,
        _ => throw new JsonException($"Unknown FailurePolicy '{s}'."),
    };

    internal static string ToWire(GalleryLayout v) => v switch
    {
        GalleryLayout.Justified => "justified",
        GalleryLayout.Masonry => "masonry",
        GalleryLayout.Grid => "grid",
        GalleryLayout.Editorial => "editorial",
        GalleryLayout.Filmstrip => "filmstrip",
        _ => throw new ArgumentOutOfRangeException(nameof(v)),
    };

    internal static GalleryLayout ParseGalleryLayout(string s) => s switch
    {
        "justified" => GalleryLayout.Justified,
        "masonry" => GalleryLayout.Masonry,
        "grid" => GalleryLayout.Grid,
        "editorial" => GalleryLayout.Editorial,
        "filmstrip" => GalleryLayout.Filmstrip,
        _ => throw new JsonException($"Unknown GalleryLayout '{s}'."),
    };

    internal static string ToWire(GalleryTheme v) => v switch
    {
        GalleryTheme.GalleryDark => "gallery_dark",
        GalleryTheme.PaperLight => "paper_light",
        GalleryTheme.MinimalWhite => "minimal_white",
        GalleryTheme.WarmLinen => "warm_linen",
        GalleryTheme.MidnightSlate => "midnight_slate",
        _ => throw new ArgumentOutOfRangeException(nameof(v)),
    };

    internal static GalleryTheme ParseGalleryTheme(string s) => s switch
    {
        "gallery_dark" => GalleryTheme.GalleryDark,
        "paper_light" => GalleryTheme.PaperLight,
        "minimal_white" => GalleryTheme.MinimalWhite,
        "warm_linen" => GalleryTheme.WarmLinen,
        "midnight_slate" => GalleryTheme.MidnightSlate,
        _ => throw new JsonException($"Unknown GalleryTheme '{s}'."),
    };

    internal static string ToWire(GalleryFontFamily v) => v switch
    {
        GalleryFontFamily.Sans => "sans",
        GalleryFontFamily.Serif => "serif",
        GalleryFontFamily.Mono => "mono",
        _ => throw new ArgumentOutOfRangeException(nameof(v)),
    };

    internal static GalleryFontFamily ParseGalleryFontFamily(string s) => s switch
    {
        "sans" => GalleryFontFamily.Sans,
        "serif" => GalleryFontFamily.Serif,
        "mono" => GalleryFontFamily.Mono,
        _ => throw new JsonException($"Unknown GalleryFontFamily '{s}'."),
    };

    internal static string ToWire(GalleryCaptionStyle v) => v switch
    {
        GalleryCaptionStyle.Below => "below",
        GalleryCaptionStyle.HoverOverlay => "hover_overlay",
        GalleryCaptionStyle.LightboxOnly => "lightbox_only",
        GalleryCaptionStyle.Hidden => "hidden",
        _ => throw new ArgumentOutOfRangeException(nameof(v)),
    };

    internal static GalleryCaptionStyle ParseGalleryCaptionStyle(string s) => s switch
    {
        "below" => GalleryCaptionStyle.Below,
        "hover_overlay" => GalleryCaptionStyle.HoverOverlay,
        "lightbox_only" => GalleryCaptionStyle.LightboxOnly,
        "hidden" => GalleryCaptionStyle.Hidden,
        _ => throw new JsonException($"Unknown GalleryCaptionStyle '{s}'."),
    };

    internal static string ToWire(GallerySortOrder v) => v switch
    {
        GallerySortOrder.FilenameAsc => "filename_asc",
        GallerySortOrder.FilenameDesc => "filename_desc",
        GallerySortOrder.AspectRatio => "aspect_ratio",
        GallerySortOrder.Custom => "custom",
        _ => throw new ArgumentOutOfRangeException(nameof(v)),
    };

    internal static GallerySortOrder ParseGallerySortOrder(string s) => s switch
    {
        "filename_asc" => GallerySortOrder.FilenameAsc,
        "filename_desc" => GallerySortOrder.FilenameDesc,
        "aspect_ratio" => GallerySortOrder.AspectRatio,
        "custom" => GallerySortOrder.Custom,
        _ => throw new JsonException($"Unknown GallerySortOrder '{s}'."),
    };
}

// =============================================================================
// NativeAOT-safe reflection-free JSON converters matching Rust serde snake_case
// =============================================================================

internal sealed class OutputFormatJsonConverter : JsonConverter<OutputFormat>
{
    public override OutputFormat Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        MediaWireTokens.ParseOutputFormat(reader.GetString() ?? throw new JsonException("Expected string for OutputFormat."));
    public override void Write(Utf8JsonWriter writer, OutputFormat value, JsonSerializerOptions options) =>
        writer.WriteStringValue(MediaWireTokens.ToWire(value));
}

internal sealed class QualityProfileLevelJsonConverter : JsonConverter<QualityProfileLevel>
{
    public override QualityProfileLevel Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        MediaWireTokens.ParseQualityProfileLevel(reader.GetString() ?? throw new JsonException("Expected string for QualityProfileLevel."));
    public override void Write(Utf8JsonWriter writer, QualityProfileLevel value, JsonSerializerOptions options) =>
        writer.WriteStringValue(MediaWireTokens.ToWire(value));
}

internal sealed class QualityFallbackPolicyJsonConverter : JsonConverter<QualityFallbackPolicy>
{
    public override QualityFallbackPolicy Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        MediaWireTokens.ParseQualityFallbackPolicy(reader.GetString() ?? throw new JsonException("Expected string for QualityFallbackPolicy."));
    public override void Write(Utf8JsonWriter writer, QualityFallbackPolicy value, JsonSerializerOptions options) =>
        writer.WriteStringValue(MediaWireTokens.ToWire(value));
}

internal sealed class FitModeJsonConverter : JsonConverter<FitMode>
{
    public override FitMode Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        MediaWireTokens.ParseFitMode(reader.GetString() ?? throw new JsonException("Expected string for FitMode."));
    public override void Write(Utf8JsonWriter writer, FitMode value, JsonSerializerOptions options) =>
        writer.WriteStringValue(MediaWireTokens.ToWire(value));
}

internal sealed class EncodeEffortJsonConverter : JsonConverter<EncodeEffort>
{
    public override EncodeEffort Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        MediaWireTokens.ParseEncodeEffort(reader.GetString() ?? throw new JsonException("Expected string for EncodeEffort."));
    public override void Write(Utf8JsonWriter writer, EncodeEffort value, JsonSerializerOptions options) =>
        writer.WriteStringValue(MediaWireTokens.ToWire(value));
}

internal sealed class MetadataRetentionJsonConverter : JsonConverter<MetadataRetention>
{
    public override MetadataRetention Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        MediaWireTokens.ParseMetadataRetention(reader.GetString() ?? throw new JsonException("Expected string for MetadataRetention."));
    public override void Write(Utf8JsonWriter writer, MetadataRetention value, JsonSerializerOptions options) =>
        writer.WriteStringValue(MediaWireTokens.ToWire(value));
}

internal sealed class WorkspaceAdmissionJsonConverter : JsonConverter<WorkspaceAdmission>
{
    public override WorkspaceAdmission Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        MediaWireTokens.ParseWorkspaceAdmission(reader.GetString() ?? throw new JsonException("Expected string for WorkspaceAdmission."));
    public override void Write(Utf8JsonWriter writer, WorkspaceAdmission value, JsonSerializerOptions options) =>
        writer.WriteStringValue(MediaWireTokens.ToWire(value));
}

internal sealed class OutputPathLayoutJsonConverter : JsonConverter<OutputPathLayout>
{
    public override OutputPathLayout Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        MediaWireTokens.ParseOutputPathLayout(reader.GetString() ?? throw new JsonException("Expected string for OutputPathLayout."));
    public override void Write(Utf8JsonWriter writer, OutputPathLayout value, JsonSerializerOptions options) =>
        writer.WriteStringValue(MediaWireTokens.ToWire(value));
}

internal sealed class FailurePolicyJsonConverter : JsonConverter<FailurePolicy>
{
    public override FailurePolicy Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        MediaWireTokens.ParseFailurePolicy(reader.GetString() ?? throw new JsonException("Expected string for FailurePolicy."));
    public override void Write(Utf8JsonWriter writer, FailurePolicy value, JsonSerializerOptions options) =>
        writer.WriteStringValue(MediaWireTokens.ToWire(value));
}

internal sealed class GalleryLayoutJsonConverter : JsonConverter<GalleryLayout>
{
    public override GalleryLayout Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        MediaWireTokens.ParseGalleryLayout(reader.GetString() ?? throw new JsonException("Expected string for GalleryLayout."));
    public override void Write(Utf8JsonWriter writer, GalleryLayout value, JsonSerializerOptions options) =>
        writer.WriteStringValue(MediaWireTokens.ToWire(value));
}

internal sealed class GalleryThemeJsonConverter : JsonConverter<GalleryTheme>
{
    public override GalleryTheme Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        MediaWireTokens.ParseGalleryTheme(reader.GetString() ?? throw new JsonException("Expected string for GalleryTheme."));
    public override void Write(Utf8JsonWriter writer, GalleryTheme value, JsonSerializerOptions options) =>
        writer.WriteStringValue(MediaWireTokens.ToWire(value));
}

internal sealed class GalleryFontFamilyJsonConverter : JsonConverter<GalleryFontFamily>
{
    public override GalleryFontFamily Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        MediaWireTokens.ParseGalleryFontFamily(reader.GetString() ?? throw new JsonException("Expected string for GalleryFontFamily."));
    public override void Write(Utf8JsonWriter writer, GalleryFontFamily value, JsonSerializerOptions options) =>
        writer.WriteStringValue(MediaWireTokens.ToWire(value));
}

internal sealed class GalleryCaptionStyleJsonConverter : JsonConverter<GalleryCaptionStyle>
{
    public override GalleryCaptionStyle Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        MediaWireTokens.ParseGalleryCaptionStyle(reader.GetString() ?? throw new JsonException("Expected string for GalleryCaptionStyle."));
    public override void Write(Utf8JsonWriter writer, GalleryCaptionStyle value, JsonSerializerOptions options) =>
        writer.WriteStringValue(MediaWireTokens.ToWire(value));
}

internal sealed class GallerySortOrderJsonConverter : JsonConverter<GallerySortOrder>
{
    public override GallerySortOrder Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        MediaWireTokens.ParseGallerySortOrder(reader.GetString() ?? throw new JsonException("Expected string for GallerySortOrder."));
    public override void Write(Utf8JsonWriter writer, GallerySortOrder value, JsonSerializerOptions options) =>
        writer.WriteStringValue(MediaWireTokens.ToWire(value));
}

internal sealed class QualityTargetJsonConverter : JsonConverter<QualityTarget>
{
    public override QualityTarget Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        using JsonDocument doc = JsonDocument.ParseValue(ref reader);
        JsonElement root = doc.RootElement;
        if (root.ValueKind != JsonValueKind.Object || !root.TryGetProperty("mode", out JsonElement modeEl))
        {
            throw new JsonException("QualityTarget JSON must be an object with a 'mode' property.");
        }
        string mode = modeEl.GetString() ?? throw new JsonException("QualityTarget 'mode' must be a string.");
        return mode switch
        {
            "verify_zensim_a" => QualityTarget.VerifyZensimA(root.GetProperty("target").GetSingle()),
            "aim_for_zensim_a" => QualityTarget.AimForZensimA(root.GetProperty("target").GetSingle()),
            "calibrated_quality" => QualityTarget.CalibratedQuality(root.GetProperty("quality").GetSingle()),
            "quality_profile" => QualityTarget.QualityProfile(
                MediaWireTokens.ParseQualityProfileLevel(root.GetProperty("level").GetString() ?? string.Empty),
                root.TryGetProperty("dpr", out JsonElement dprEl) ? dprEl.GetSingle() : 1.0f),
            "lossless" => QualityTarget.Lossless(),
            _ => throw new JsonException($"Unknown QualityTarget mode '{mode}'."),
        };
    }

    public override void Write(Utf8JsonWriter writer, QualityTarget value, JsonSerializerOptions options)
    {
        writer.WriteStartObject();
        switch (value.Mode)
        {
            case QualityTargetMode.VerifyZensimA:
                writer.WriteString("mode", "verify_zensim_a");
                writer.WriteNumber("target", value.Target!.Value);
                break;
            case QualityTargetMode.AimForZensimA:
                writer.WriteString("mode", "aim_for_zensim_a");
                writer.WriteNumber("target", value.Target!.Value);
                break;
            case QualityTargetMode.CalibratedQuality:
                writer.WriteString("mode", "calibrated_quality");
                writer.WriteNumber("quality", value.Quality!.Value);
                break;
            case QualityTargetMode.QualityProfile:
                writer.WriteString("mode", "quality_profile");
                writer.WriteString("level", MediaWireTokens.ToWire(value.ProfileLevel!.Value));
                writer.WriteNumber("dpr", value.Dpr);
                break;
            case QualityTargetMode.Lossless:
                writer.WriteString("mode", "lossless");
                break;
            default:
                throw new JsonException($"Unknown QualityTarget mode '{value.Mode}'.");
        }
        writer.WriteEndObject();
    }
}

[JsonSourceGenerationOptions(WriteIndented = true)]
[JsonSerializable(typeof(ResponsivePreset))]
[JsonSerializable(typeof(BulkManifest))]
[JsonSerializable(typeof(ManifestAsset))]
[JsonSerializable(typeof(ManifestVariant))]
[JsonSerializable(typeof(GalleryConfig))]
[JsonSerializable(typeof(GalleryDesign))]
[JsonSerializable(typeof(GalleryItemOverride))]
[JsonSerializable(typeof(ResolvedGalleryItem))]
[JsonSerializable(typeof(IReadOnlyList<ResolvedGalleryItem>))]
[JsonSerializable(typeof(GalleryOutputDocument))]
[JsonSerializable(typeof(BulkCacheDocument))]
[JsonSerializable(typeof(CachedAssetEntry))]
internal sealed partial class MediaBulkJsonContext : JsonSerializerContext
{
}
