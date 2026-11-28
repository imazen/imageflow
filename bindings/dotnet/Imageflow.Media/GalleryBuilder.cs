using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;

namespace Imageflow.Media;

/// <summary>
/// Fluent, NativeAOT-safe static gallery builder sharing the exact <c>imageflow-gallery-v1</c>
/// schema, XSS-hardened HTML/JSON escaping, accessible lightbox, and incremental variant cache
/// with Rust <c>imageflow_tool gallery</c>.
/// </summary>
public sealed class GalleryBuilder
{
    private readonly string _inputDirectory;
    private GalleryConfig _config;
    private int _concurrency = Math.Clamp(Environment.ProcessorCount, 1, 4);
    private bool _forceRebuild;
    private BulkLimits _limits = BulkLimits.Default;
    private Func<CancellationToken, ValueTask>? _beforePublishAsync;

    private GalleryBuilder(string inputDirectory, GalleryConfig config)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(inputDirectory);
        _inputDirectory = Path.GetFullPath(inputDirectory);
        _config = config ?? throw new ArgumentNullException(nameof(config));
    }

    /// <summary>Create a new gallery builder targeting <paramref name="inputDirectory"/>.</summary>
    public static GalleryBuilder FromDirectory(string inputDirectory, string? title = null)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(inputDirectory);
        string full = Path.GetFullPath(inputDirectory);
        string dirName = Path.GetFileName(full.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar));
        string inferred = string.IsNullOrWhiteSpace(title)
            ? (string.IsNullOrWhiteSpace(dirName) ? "Photo Gallery" : dirName.Replace('-', ' ').Replace('_', ' '))
            : title;
        var cfg = new GalleryConfig
        {
            Title = inferred,
            FooterText = "Built with Imageflow — color-managed responsive media",
        };
        return new GalleryBuilder(full, cfg);
    }

    /// <summary>Create a gallery builder initialized from an existing <see cref="GalleryConfig"/>.</summary>
    public static GalleryBuilder FromConfig(string inputDirectory, GalleryConfig config)
    {
        ArgumentNullException.ThrowIfNull(config);
        config.Validate();
        return new GalleryBuilder(inputDirectory, config);
    }

    /// <summary>Load an <c>imageflow-gallery-v1</c> JSON config file produced by C# or Rust CLI.</summary>
    public static GalleryBuilder FromConfigJsonFile(string inputDirectory, string jsonPath)
    {
        string raw = File.ReadAllText(jsonPath, Encoding.UTF8);
        return FromConfig(inputDirectory, GalleryConfig.FromJson(raw));
    }

    public GalleryBuilder WithTitle(string title) { _config = _config with { Title = title }; return this; }
    public GalleryBuilder WithSubtitle(string? subtitle) { _config = _config with { Subtitle = subtitle }; return this; }
    public GalleryBuilder WithAuthor(string? author) { _config = _config with { Author = author }; return this; }
    public GalleryBuilder WithFooter(string? footerText) { _config = _config with { FooterText = footerText }; return this; }
    public GalleryBuilder WithSortBy(GallerySortOrder sortBy) { _config = _config with { SortBy = sortBy }; return this; }
    public GalleryBuilder WithLayout(GalleryLayout layout) { _config = _config with { Design = _config.Design with { Layout = layout } }; return this; }
    public GalleryBuilder WithTheme(GalleryTheme theme) { _config = _config with { Design = _config.Design with { Theme = theme } }; return this; }
    public GalleryBuilder WithFont(GalleryFontFamily font) { _config = _config with { Design = _config.Design with { FontFamily = font } }; return this; }

    /// <summary>Update one or more spacing properties without clobbering unspecified values.</summary>
    public GalleryBuilder WithSpacing(int? gapPx = null, int? cornerRadiusPx = null, int? paddingPx = null)
    {
        _config = _config with
        {
            Design = _config.Design with
            {
                GapPx = gapPx ?? _config.Design.GapPx,
                CornerRadiusPx = cornerRadiusPx ?? _config.Design.CornerRadiusPx,
                PaddingPx = paddingPx ?? _config.Design.PaddingPx,
            },
        };
        return this;
    }

    public GalleryBuilder WithTargetRowHeight(int targetRowHeightPx)
    {
        _config = _config with { Design = _config.Design with { TargetRowHeightPx = targetRowHeightPx } };
        return this;
    }

    /// <summary>Update one or more responsive column counts without clobbering unspecified breakpoints.</summary>
    public GalleryBuilder WithColumns(int? desktop = null, int? tablet = null, int? mobile = null)
    {
        _config = _config with
        {
            Design = _config.Design with
            {
                ColumnsDesktop = desktop ?? _config.Design.ColumnsDesktop,
                ColumnsTablet = tablet ?? _config.Design.ColumnsTablet,
                ColumnsMobile = mobile ?? _config.Design.ColumnsMobile,
            },
        };
        return this;
    }
    public GalleryBuilder WithGridAspectRatio(AspectRatio? aspectRatio)
    {
        _config = _config with { Design = _config.Design with { GridAspectRatio = aspectRatio } };
        return this;
    }
    public GalleryBuilder WithMaxWidth(int maxWidthPx)
    {
        _config = _config with { Design = _config.Design with { MaxWidthPx = maxWidthPx } };
        return this;
    }
    public GalleryBuilder WithCaptionStyle(GalleryCaptionStyle style)
    {
        _config = _config with { Design = _config.Design with { CaptionStyle = style } };
        return this;
    }
    public GalleryBuilder WithExifBadges(bool show)
    {
        _config = _config with { Design = _config.Design with { ShowExifBadges = show } };
        return this;
    }
    public GalleryBuilder WithDownloadButton(bool show)
    {
        _config = _config with { Design = _config.Design with { ShowDownloadButton = show } };
        return this;
    }
    public GalleryBuilder WithLightbox(bool enabled)
    {
        _config = _config with { Design = _config.Design with { EnableLightbox = enabled } };
        return this;
    }
    public GalleryBuilder WithAccentColor(string? hexColor)
    {
        _config = _config with { Design = _config.Design with { AccentColorHex = hexColor } };
        return this;
    }
    public GalleryBuilder WithEncoding(ResponsivePreset preset)
    {
        ArgumentNullException.ThrowIfNull(preset);
        _config = _config with { Encoding = preset };
        return this;
    }
    public GalleryBuilder ConfigureItem(string sourceRelativePath, Func<GalleryItemOverride, GalleryItemOverride> configure)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(sourceRelativePath);
        ArgumentNullException.ThrowIfNull(configure);
        var items = new Dictionary<string, GalleryItemOverride>(_config.Items, StringComparer.Ordinal);
        GalleryItemOverride existing = items.TryGetValue(sourceRelativePath, out GalleryItemOverride? prev) ? prev : new GalleryItemOverride();
        items[sourceRelativePath] = configure(existing);
        _config = _config with { Items = items };
        return this;
    }
    public GalleryBuilder WithConcurrency(int concurrency) { _concurrency = Math.Clamp(concurrency, 1, 64); return this; }
    public GalleryBuilder WithForceRebuild(bool forceRebuild = true) { _forceRebuild = forceRebuild; return this; }
    public GalleryBuilder WithLimits(BulkLimits limits) { _limits = limits ?? throw new ArgumentNullException(nameof(limits)); return this; }

    /// <summary>
    /// Optional hook invoked immediately before atomic gallery entry-point publication
    /// (<c>index.html</c>, <c>gallery.json</c>, <c>gallery.config.json</c>).
    /// </summary>
    public GalleryBuilder WithBeforePublishAsync(Func<CancellationToken, ValueTask>? beforePublishAsync)
    {
        _beforePublishAsync = beforePublishAsync;
        return this;
    }

    public GalleryConfig ToConfig()
    {
        _config.Validate();
        return _config;
    }

    /// <summary>Apply curation overrides, visibility filters, and sort order to encoded manifest assets.</summary>
    public static (IReadOnlyList<ResolvedGalleryItem> VisibleItems, int HiddenCount) ResolveItems(
        GalleryConfig config,
        IReadOnlyList<ManifestAsset> assets)
    {
        ArgumentNullException.ThrowIfNull(config);
        ArgumentNullException.ThrowIfNull(assets);

        int hidden = 0;
        var resolved = new List<(uint? Order, ResolvedGalleryItem Item)>(assets.Count);

        foreach (ManifestAsset asset in assets)
        {
            config.Items.TryGetValue(asset.SourceRelativePath, out GalleryItemOverride? ov);
            if (ov is { Hidden: true })
            {
                hidden++;
                continue;
            }

            string defaultTitle = Path.GetFileNameWithoutExtension(asset.SourceRelativePath)
                .Replace('-', ' ')
                .Replace('_', ' ');
            if (string.IsNullOrWhiteSpace(defaultTitle))
            {
                defaultTitle = asset.SourceRelativePath;
            }
            string title = !string.IsNullOrWhiteSpace(ov?.Title) ? ov.Title! : defaultTitle;
            string caption = ov?.Caption ?? string.Empty;
            string altText = !string.IsNullOrWhiteSpace(ov?.AltText) ? ov.AltText! : title;
            bool featured = ov?.Featured ?? false;

            int maxWidth = asset.Variants.Count > 0 ? asset.Variants.Max(v => v.Width) : 0;
            long smallestAtMaxW = asset.Variants
                .Where(v => v.Width == maxWidth)
                .Select(v => v.Bytes)
                .DefaultIfEmpty(0L)
                .Min();
            double savingsPct = asset.SourceBytes > 0 && smallestAtMaxW > 0
                ? ((asset.SourceBytes - smallestAtMaxW) / (double)asset.SourceBytes) * 100.0
                : 0.0;

            ManifestVariant? bestVerified = asset.Variants
                .Where(v => v.VerifiedScore.HasValue && !string.IsNullOrEmpty(v.VerifiedMetric))
                .OrderByDescending(v => v.VerifiedScore!.Value)
                .FirstOrDefault();

            string downloadHref = asset.Variants
                .OrderByDescending(v => v.Width)
                .ThenByDescending(v => v.Format == OutputFormat.Jpeg ? 1 : 0)
                .Select(v => v.RelativePath)
                .FirstOrDefault() ?? string.Empty;

            double aspectRatio = Math.Max(1, asset.OriginalWidth) / (double)Math.Max(1, asset.OriginalHeight);
            string pictureHtml = BulkMediaProcessor.RenderPictureSnippet(
                asset,
                urlPrefix: "",
                sizesAttr: "(max-width: 640px) 100vw, (max-width: 1100px) 50vw, 33vw",
                altText: altText,
                eager: featured);

            resolved.Add((ov?.Order, new ResolvedGalleryItem
            {
                Id = asset.Id,
                SourceRelativePath = asset.SourceRelativePath,
                Title = title,
                Caption = caption,
                AltText = altText,
                Featured = featured,
                MediaKind = asset.MediaKind,
                SourceFormat = asset.SourceFormat,
                OriginalWidth = asset.OriginalWidth,
                OriginalHeight = asset.OriginalHeight,
                AspectRatio = aspectRatio,
                SourceBytes = asset.SourceBytes,
                SmallestVariantBytes = smallestAtMaxW,
                SavingsPct = savingsPct,
                BestVerifiedMetric = bestVerified?.VerifiedMetric,
                BestVerifiedScore = bestVerified?.VerifiedScore,
                Variants = asset.Variants,
                VideoVariants = asset.VideoVariants,
                PictureHtml = pictureHtml,
                DownloadHref = downloadHref,
            }));
        }

        switch (config.SortBy)
        {
            case GallerySortOrder.FilenameAsc:
                resolved.Sort((a, b) => StringComparer.Ordinal.Compare(a.Item.SourceRelativePath, b.Item.SourceRelativePath));
                break;
            case GallerySortOrder.FilenameDesc:
                resolved.Sort((a, b) => StringComparer.Ordinal.Compare(b.Item.SourceRelativePath, a.Item.SourceRelativePath));
                break;
            case GallerySortOrder.AspectRatio:
                resolved.Sort((a, b) =>
                {
                    int c = b.Item.AspectRatio.CompareTo(a.Item.AspectRatio);
                    return c != 0 ? c : StringComparer.Ordinal.Compare(a.Item.SourceRelativePath, b.Item.SourceRelativePath);
                });
                break;
            case GallerySortOrder.Custom:
                resolved.Sort((a, b) =>
                {
                    uint ao = a.Order ?? uint.MaxValue;
                    uint bo = b.Order ?? uint.MaxValue;
                    int c = ao.CompareTo(bo);
                    return c != 0 ? c : StringComparer.Ordinal.Compare(a.Item.SourceRelativePath, b.Item.SourceRelativePath);
                });
                break;
        }

        return (resolved.Select(r => r.Item).ToList(), hidden);
    }

    /// <summary>Escape JSON safely for embedding inside <c>&lt;script type="application/json"&gt;</c>.</summary>
    public static string EscapeJsonForScript(string rawJson)
    {
        var sb = new StringBuilder(rawJson.Length + 16);
        foreach (char ch in rawJson)
        {
            switch (ch)
            {
                case '<': sb.Append("\\u003c"); break;
                case '>': sb.Append("\\u003e"); break;
                case '&': sb.Append("\\u0026"); break;
                case '\u2028': sb.Append("\\u2028"); break;
                case '\u2029': sb.Append("\\u2029"); break;
                default: sb.Append(ch); break;
            }
        }
        return sb.ToString();
    }

    /// <summary>
    /// Render a complete self-contained HTML5 gallery document matching Rust
    /// <c>imageflow_tool_lib::gallery::render::render_gallery_html</c>.
    /// </summary>
    public static string RenderGalleryHtml(GalleryConfig config, IReadOnlyList<ResolvedGalleryItem> items)
    {
        ArgumentNullException.ThrowIfNull(config);
        ArgumentNullException.ThrowIfNull(items);
        config.Validate();

        GalleryDesign d = config.Design;
        (string bg, string surface, string surface2, string border, string text, string muted, string themeAccent, string shadow) = d.Theme switch
        {
            GalleryTheme.GalleryDark => ("#0b0c10", "#14161d", "#1c202a", "rgba(255,255,255,0.09)", "#f3f4f6", "#9ca3af", "#60a5fa", "0 10px 30px rgba(0,0,0,0.45)"),
            GalleryTheme.PaperLight => ("#f7f5f0", "#ffffff", "#efece4", "rgba(24,24,27,0.10)", "#18181b", "#52525b", "#2563eb", "0 8px 24px rgba(15,23,42,0.08)"),
            GalleryTheme.MinimalWhite => ("#ffffff", "#fafafa", "#f4f4f5", "rgba(0,0,0,0.08)", "#111827", "#6b7280", "#111827", "0 4px 16px rgba(0,0,0,0.06)"),
            GalleryTheme.WarmLinen => ("#f4efe6", "#fbf8f2", "#eae1d2", "rgba(68,51,34,0.12)", "#2c221a", "#6d5b4b", "#b45309", "0 8px 24px rgba(44,34,26,0.09)"),
            GalleryTheme.MidnightSlate => ("#0f172a", "#1e293b", "#283548", "rgba(148,163,184,0.16)", "#f8fafc", "#94a3b8", "#38bdf8", "0 12px 32px rgba(2,6,23,0.5)"),
            _ => ("#0b0c10", "#14161d", "#1c202a", "rgba(255,255,255,0.09)", "#f3f4f6", "#9ca3af", "#60a5fa", "0 10px 30px rgba(0,0,0,0.45)"),
        };

        string accent = GalleryDesign.IsValidHexColor(d.AccentColorHex) ? d.AccentColorHex! : themeAccent;
        string fonts = d.FontFamily switch
        {
            GalleryFontFamily.Sans => "Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
            GalleryFontFamily.Serif => "'Iowan Old Style', 'Palatino Linotype', Palatino, Georgia, 'Times New Roman', serif",
            GalleryFontFamily.Mono => "'JetBrains Mono', ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
            _ => "system-ui, sans-serif",
        };
        string gridAr = d.GridAspectRatio?.CssString ?? "4 / 3";
        string layoutClass = d.Layout switch
        {
            GalleryLayout.Justified => "layout-justified",
            GalleryLayout.Masonry => "layout-masonry",
            GalleryLayout.Grid => "layout-grid",
            GalleryLayout.Editorial => "layout-editorial",
            GalleryLayout.Filmstrip => "layout-filmstrip",
            _ => "layout-justified",
        };
        string captionClass = d.CaptionStyle switch
        {
            GalleryCaptionStyle.Below => "cap-below",
            GalleryCaptionStyle.HoverOverlay => "cap-overlay",
            GalleryCaptionStyle.LightboxOnly => "cap-lightbox",
            GalleryCaptionStyle.Hidden => "cap-hidden",
            _ => "cap-below",
        };

        var cards = new StringBuilder(items.Count * 640);
        for (int idx = 0; idx < items.Count; idx++)
        {
            ResolvedGalleryItem item = items[idx];
            string featuredCls = item.Featured ? " is-featured" : "";
            double flexBasis = Math.Max(120.0, Math.Round(d.TargetRowHeightPx * item.AspectRatio));
            double flexGrow = Math.Max(30.0, Math.Round(item.AspectRatio * 100.0));

            string badgeHtml = string.Empty;
            if (d.ShowExifBadges)
            {
                var parts = new List<string> { string.Create(CultureInfo.InvariantCulture, $"{item.OriginalWidth}×{item.OriginalHeight}") };
                if (item.SmallestVariantBytes > 0)
                {
                    parts.Add(FormatBytes(item.SmallestVariantBytes));
                }
                if (item.BestVerifiedScore is double score)
                {
                    parts.Add(string.Format(CultureInfo.InvariantCulture, "zensim {0:F1}", score));
                }
                if (item.MediaKind is "video" or "animation")
                {
                    parts.Add(item.MediaKind.ToUpperInvariant());
                }
                badgeHtml = $"<span class=\"meta-badge\">{BulkMediaProcessor.EscapeHtml(string.Join(" · ", parts))}</span>";
            }

            string captionInner = !string.IsNullOrEmpty(item.Caption)
                ? $"<div class=\"cap-title\">{BulkMediaProcessor.EscapeHtml(item.Title)}</div><div class=\"cap-desc\">{BulkMediaProcessor.EscapeHtml(item.Caption)}</div>{badgeHtml}"
                : $"<div class=\"cap-title\">{BulkMediaProcessor.EscapeHtml(item.Title)}</div>{badgeHtml}";

            string triggerAttrs = d.EnableLightbox
                ? string.Create(CultureInfo.InvariantCulture, $"tabindex=\"0\" role=\"button\" aria-label=\"Open {BulkMediaProcessor.EscapeHtml(item.Title)} in lightbox\" data-index=\"{idx}\"")
                : string.Create(CultureInfo.InvariantCulture, $"data-index=\"{idx}\"");

            cards.Append(string.Create(
                CultureInfo.InvariantCulture,
                $"<figure class=\"g-card{featuredCls}\" data-id=\"{BulkMediaProcessor.EscapeHtml(item.Id)}\" style=\"--ar:{item.AspectRatio:F4};--jf-basis:{flexBasis:F0}px;--jf-grow:{flexGrow:F0};\">\n  <div class=\"g-frame\" {triggerAttrs}>\n    {item.PictureHtml}\n  </div>\n  <figcaption class=\"g-cap\">{captionInner}</figcaption>\n</figure>\n"));
        }

        string itemsJsonRaw = JsonSerializer.Serialize(items, MediaBulkJsonContext.Default.IReadOnlyListResolvedGalleryItem);
        string itemsJson = EscapeJsonForScript(itemsJsonRaw);

        string titleEsc = BulkMediaProcessor.EscapeHtml(config.Title);
        string subtitleHtml = !string.IsNullOrWhiteSpace(config.Subtitle) ? $"<p class=\"g-sub\">{BulkMediaProcessor.EscapeHtml(config.Subtitle)}</p>" : "";
        string authorHtml = !string.IsNullOrWhiteSpace(config.Author) ? $"<div class=\"g-author\">By {BulkMediaProcessor.EscapeHtml(config.Author)}</div>" : "";
        string footerHtml = !string.IsNullOrWhiteSpace(config.FooterText) ? $"<footer class=\"g-footer\">{BulkMediaProcessor.EscapeHtml(config.FooterText)}</footer>" : "";
        string dlBtn = d.ShowDownloadButton ? "<a id=\"lb-dl\" class=\"lb-btn\" download>Download</a>" : "";
        string cursor = d.EnableLightbox ? "pointer" : "default";
        string enableLb = d.EnableLightbox ? "true" : "false";

        return string.Create(CultureInfo.InvariantCulture, $$"""
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{{titleEsc}}</title>
<style>
:root {
  --bg: {{bg}}; --surface: {{surface}}; --surface-2: {{surface2}}; --border: {{border}};
  --text: {{text}}; --muted: {{muted}}; --accent: {{accent}}; --shadow: {{shadow}};
  --font: {{fonts}}; --gap: {{d.GapPx}}px; --radius: {{d.CornerRadiusPx}}px;
  --max-w: {{d.MaxWidthPx}}px; --pad: {{d.PaddingPx}}px; --row-h: {{d.TargetRowHeightPx}}px;
  --cols-d: {{d.ColumnsDesktop}}; --cols-t: {{d.ColumnsTablet}}; --cols-m: {{d.ColumnsMobile}};
  --grid-ar: {{gridAr}};
}
* { box-sizing: border-box; }
html, body { margin: 0; padding: 0; background: var(--bg); color: var(--text); font-family: var(--font); -webkit-font-smoothing: antialiased; }
.g-shell { max-width: var(--max-w); margin: 0 auto; padding: var(--pad); }
.g-header { margin-bottom: calc(var(--gap) * 1.6); border-bottom: 1px solid var(--border); padding-bottom: 18px; }
.g-header h1 { margin: 0 0 6px; font-size: clamp(1.6rem, 3vw, 2.5rem); letter-spacing: -0.02em; }
.g-sub { margin: 0; color: var(--muted); font-size: 1rem; line-height: 1.5; }
.g-author { margin-top: 8px; font-size: 0.85rem; color: var(--accent); font-weight: 600; }
.g-card { margin: 0; background: var(--surface); border: 1px solid var(--border); border-radius: var(--radius); overflow: hidden; box-shadow: var(--shadow); position: relative; transition: transform 180ms ease, border-color 180ms ease; }
.g-card:hover { border-color: var(--accent); transform: translateY(-2px); }
.g-frame { cursor: {{cursor}}; overflow: hidden; background: var(--surface-2); position: relative; display: block; }
.g-frame:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
.g-frame picture, .g-frame img { display: block; width: 100%; height: 100%; object-fit: cover; }
.g-cap { padding: 10px 12px; display: flex; flex-direction: column; gap: 3px; }
.cap-title { font-weight: 600; font-size: 0.92rem; line-height: 1.3; }
.cap-desc { font-size: 0.82rem; color: var(--muted); line-height: 1.4; }
.meta-badge { margin-top: 4px; font-size: 0.72rem; color: var(--muted); font-variant-numeric: tabular-nums; }
.cap-hidden .g-cap, .cap-lightbox .g-cap { display: none; }
.cap-overlay .g-cap { position: absolute; left: 0; right: 0; bottom: 0; background: linear-gradient(transparent, rgba(0,0,0,0.82)); color: #fff; opacity: 0; transition: opacity 180ms ease; pointer-events: none; }
.cap-overlay .g-card:hover .g-cap, .cap-overlay .g-card:focus-within .g-cap { opacity: 1; }
.layout-justified { display: flex; flex-wrap: wrap; gap: var(--gap); }
.layout-justified .g-card { flex: var(--jf-grow, 100) 1 var(--jf-basis, 260px); display: flex; flex-direction: column; }
.layout-justified .g-frame { height: var(--row-h); }
.layout-justified::after { content: ""; flex: 9999 1 0; }
.layout-masonry { column-count: var(--cols-d); column-gap: var(--gap); }
.layout-masonry .g-card { break-inside: avoid; margin-bottom: var(--gap); }
.layout-masonry .g-frame img { height: auto; }
.layout-grid { display: grid; grid-template-columns: repeat(var(--cols-d), minmax(0, 1fr)); gap: var(--gap); }
.layout-grid .g-frame { aspect-ratio: var(--grid-ar); }
.layout-editorial { display: grid; grid-template-columns: repeat(6, minmax(0, 1fr)); gap: var(--gap); }
.layout-editorial .g-card { grid-column: span 3; }
.layout-editorial .g-card:first-child, .layout-editorial .g-card.is-featured { grid-column: 1 / -1; }
.layout-editorial .g-card:first-child .g-frame, .layout-editorial .g-card.is-featured .g-frame { max-height: 70vh; aspect-ratio: 16 / 9; }
.layout-editorial .g-card:nth-child(5n+2), .layout-editorial .g-card:nth-child(5n+3), .layout-editorial .g-card:nth-child(5n+4) { grid-column: span 2; }
.layout-editorial .g-frame { aspect-ratio: 4 / 3; }
.layout-filmstrip { display: flex; gap: var(--gap); overflow-x: auto; scroll-snap-type: x mandatory; padding-bottom: 14px; }
.layout-filmstrip .g-card { flex: 0 0 min(78vw, 680px); scroll-snap-align: center; }
.layout-filmstrip .g-frame { aspect-ratio: 3 / 2; }
@media (max-width: 1024px) { .layout-masonry { column-count: var(--cols-t); } .layout-grid { grid-template-columns: repeat(var(--cols-t), minmax(0, 1fr)); } }
@media (max-width: 640px) { .layout-masonry { column-count: var(--cols-m); } .layout-grid { grid-template-columns: repeat(var(--cols-m), minmax(0, 1fr)); } .layout-editorial .g-card { grid-column: 1 / -1 !important; } }
.lb-backdrop { position: fixed; inset: 0; background: rgba(4,6,12,0.94); z-index: 9999; display: none; flex-direction: column; padding: 18px; color: #f3f4f6; }
.lb-backdrop.open { display: flex; }
.lb-bar { display: flex; justify-content: space-between; align-items: center; gap: 12px; margin-bottom: 12px; }
.lb-counter { font-size: 0.88rem; color: #9ca3af; font-variant-numeric: tabular-nums; }
.lb-actions { display: flex; gap: 8px; }
.lb-btn { background: rgba(255,255,255,0.10); color: #fff; border: 1px solid rgba(255,255,255,0.18); border-radius: 8px; padding: 7px 12px; cursor: pointer; font-size: 0.86rem; text-decoration: none; }
.lb-btn:hover, .lb-btn:focus-visible { background: var(--accent); border-color: var(--accent); outline: none; }
.lb-stage { flex: 1; min-height: 0; display: flex; align-items: center; justify-content: center; position: relative; overflow: auto; }
.lb-stage img { max-width: 100%; max-height: 100%; object-fit: contain; border-radius: 6px; }
.lb-stage.zoomed img { max-width: none; max-height: none; cursor: zoom-out; }
.lb-nav { position: absolute; top: 50%; transform: translateY(-50%); background: rgba(0,0,0,0.55); border: 1px solid rgba(255,255,255,0.2); color: #fff; width: 44px; height: 44px; border-radius: 999px; font-size: 1.3rem; cursor: pointer; }
.lb-prev { left: 10px; } .lb-next { right: 10px; }
.lb-foot { margin-top: 12px; text-align: center; max-width: 820px; margin-inline: auto; }
.lb-title { font-weight: 600; font-size: 1.02rem; }
.lb-caption { color: #cbd5e1; font-size: 0.9rem; margin-top: 4px; }
.lb-meta { color: #94a3b8; font-size: 0.78rem; margin-top: 6px; font-variant-numeric: tabular-nums; }
.g-footer { margin-top: 40px; padding-top: 16px; border-top: 1px solid var(--border); color: var(--muted); font-size: 0.82rem; text-align: center; }
</style>
</head>
<body>
<main class="g-shell">
  <header class="g-header">
    <h1 id="g-title">{{titleEsc}}</h1>
    {{subtitleHtml}}
    {{authorHtml}}
  </header>
  <section id="g-root" class="g-grid {{layoutClass}} {{captionClass}}" aria-label="{{titleEsc}}">
    {{cards}}
  </section>
  {{footerHtml}}
</main>
<div id="lb" class="lb-backdrop" role="dialog" aria-modal="true" aria-label="Gallery viewer">
  <div class="lb-bar">
    <span id="lb-counter" class="lb-counter"></span>
    <div class="lb-actions">
      <button id="lb-zoom" class="lb-btn" type="button">Zoom</button>
      {{dlBtn}}
      <button id="lb-close" class="lb-btn" type="button">Close (Esc)</button>
    </div>
  </div>
  <div id="lb-stage" class="lb-stage">
    <button id="lb-prev" class="lb-nav lb-prev" type="button" aria-label="Previous photo">&#8249;</button>
    <div id="lb-media"></div>
    <button id="lb-next" class="lb-nav lb-next" type="button" aria-label="Next photo">&#8250;</button>
  </div>
  <div class="lb-foot">
    <div id="lb-title" class="lb-title"></div>
    <div id="lb-caption" class="lb-caption"></div>
    <div id="lb-meta" class="lb-meta"></div>
  </div>
</div>
<script id="g-data" type="application/json">{{itemsJson}}</script>
<script>
(() => {
  const items = JSON.parse(document.getElementById('g-data').textContent || '[]');
  const lb = document.getElementById('lb'), lbMedia = document.getElementById('lb-media'), lbStage = document.getElementById('lb-stage');
  const lbTitle = document.getElementById('lb-title'), lbCaption = document.getElementById('lb-caption'), lbMeta = document.getElementById('lb-meta');
  const lbCounter = document.getElementById('lb-counter'), lbClose = document.getElementById('lb-close'), lbPrev = document.getElementById('lb-prev'), lbNext = document.getElementById('lb-next'), lbZoom = document.getElementById('lb-zoom'), lbDl = document.getElementById('lb-dl');
  const enableLightbox = {{enableLb}};
  let cur = -1, lastFocus = null;
  function renderSlide(idx) {
    if (!items.length) return;
    cur = ((idx % items.length) + items.length) % items.length;
    const it = items[cur];
    lbStage.classList.remove('zoomed');
    lbMedia.replaceChildren();
    const pic = document.createElement('picture');
    const byFmt = new Map();
    for (const v of (it.variants || [])) { if (!byFmt.has(v.mime_type)) byFmt.set(v.mime_type, []); byFmt.get(v.mime_type).push(v); }
    let fallback = null;
    for (const [mime, list] of byFmt.entries()) {
      list.sort((a, b) => a.width - b.width);
      if (mime === 'image/jpeg' || mime === 'image/png') { fallback = list[list.length - 1]; }
      else { const s = document.createElement('source'); s.type = mime; s.srcset = list.map(v => v.relative_path + ' ' + v.width + 'w').join(', '); s.sizes = '100vw'; pic.appendChild(s); }
    }
    const best = fallback || (it.variants && it.variants[it.variants.length - 1]);
    if (best) { const img = document.createElement('img'); img.src = best.relative_path; img.alt = it.alt_text || it.title || ''; img.decoding = 'async'; pic.appendChild(img); }
    lbMedia.appendChild(pic);
    lbCounter.textContent = (cur + 1) + ' / ' + items.length;
    lbTitle.textContent = it.title || '';
    lbCaption.textContent = it.caption || '';
    const bits = [it.original_width + '×' + it.original_height];
    if (it.best_verified_score != null) bits.push('zensim_a ' + it.best_verified_score.toFixed(1));
    if (it.savings_pct > 0) bits.push(it.savings_pct.toFixed(0) + '% smaller');
    lbMeta.textContent = bits.join(' · ');
    if (lbDl && it.download_href) lbDl.href = it.download_href;
  }
  function openLb(idx) { if (!enableLightbox || !items.length) return; lastFocus = document.activeElement; renderSlide(idx); lb.classList.add('open'); lbClose.focus(); }
  function closeLb() { lb.classList.remove('open'); lbMedia.replaceChildren(); if (lastFocus && typeof lastFocus.focus === 'function') lastFocus.focus(); }
  document.querySelectorAll('.g-frame[data-index]').forEach((el) => {
    el.addEventListener('click', () => openLb(Number(el.dataset.index)));
    el.addEventListener('keydown', (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); openLb(Number(el.dataset.index)); } });
  });
  lbClose.addEventListener('click', closeLb); lbPrev.addEventListener('click', () => renderSlide(cur - 1)); lbNext.addEventListener('click', () => renderSlide(cur + 1));
  lbZoom.addEventListener('click', () => lbStage.classList.toggle('zoomed'));
  lb.addEventListener('click', (e) => { if (e.target === lb || e.target === lbStage) closeLb(); });
  document.addEventListener('keydown', (e) => {
    if (!lb.classList.contains('open')) return;
    if (e.key === 'Escape') closeLb();
    else if (e.key === 'ArrowLeft') renderSlide(cur - 1);
    else if (e.key === 'ArrowRight') renderSlide(cur + 1);
    else if (e.key === 'Tab') {
      const f = lb.querySelectorAll('button, a[href]');
      if (!f.length) return;
      if (e.shiftKey && document.activeElement === f[0]) { e.preventDefault(); f[f.length - 1].focus(); }
      else if (!e.shiftKey && document.activeElement === f[f.length - 1]) { e.preventDefault(); f[0].focus(); }
    }
  });
})();
</script>
</body>
</html>
""");
    }

    /// <summary>
    /// Encode/reuse all responsive gallery variants and atomically publish <c>gallery.config.json</c>,
    /// <c>gallery.json</c>, and <c>index.html</c> in one coordinated transaction.
    /// </summary>
    public async Task<GalleryBuildReport> BuildAsync(
        Func<SafeHandle> contextFactory,
        string outputDirectory,
        IProgress<BulkItemOutcome>? progress = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(contextFactory);
        ArgumentException.ThrowIfNullOrWhiteSpace(outputDirectory);
        _config.Validate();

        string outRoot = Path.GetFullPath(outputDirectory);
        FileStagedOutputAdapter.CreateConfinedDirectory(outRoot, outRoot);

        FailurePolicy onError = _config.Encoding.QualityFallback == QualityFallbackPolicy.StrictVerify
            ? FailurePolicy.FailFast
            : FailurePolicy.Continue;

        BulkProcessingReport bulkReport = await BulkMediaProcessor.ProcessAsync(
            contextFactory,
            new BulkProcessOptions
            {
                Inputs = new[] { _inputDirectory },
                OutputDirectory = outRoot,
                Preset = _config.Encoding,
                Recursive = true,
                FollowSymlinks = false,
                OutputLayout = OutputPathLayout.ContentAddressed,
                Concurrency = _concurrency,
                IncrementalCache = true,
                ForceRebuild = _forceRebuild,
                OnError = onError,
                WriteManifest = false,
                WriteSnippets = true,
                Limits = _limits,
            },
            progress,
            cancellationToken).ConfigureAwait(false);

        (IReadOnlyList<ResolvedGalleryItem> visibleItems, int hiddenCount) = ResolveItems(_config, bulkReport.Manifest.Assets);
        if (bulkReport.Failed > 0 && visibleItems.Count == 0)
        {
            throw new InvalidOperationException(
                $"Gallery build produced zero valid items ({bulkReport.Failed} failed): {string.Join("; ", bulkReport.Errors)}");
        }

        var galleryDoc = new GalleryOutputDocument
        {
            Schema = MediaSchemaVersions.Gallery,
            GeneratedAtUtc = bulkReport.Manifest.GeneratedAtUtc,
            Title = _config.Title,
            Subtitle = _config.Subtitle,
            Author = _config.Author,
            FooterText = _config.FooterText,
            Design = _config.Design,
            Encoding = _config.Encoding,
            TotalInputBytes = bulkReport.TotalInputBytes,
            TotalOutputBytes = bulkReport.TotalOutputBytes,
            Items = visibleItems,
        };

        string configJsonPath = Path.Combine(outRoot, "gallery.config.json");
        string galleryJsonPath = Path.Combine(outRoot, "gallery.json");
        string indexHtmlPath = Path.Combine(outRoot, "index.html");
        string snippetsHtmlPath = bulkReport.SnippetsPath ?? Path.Combine(outRoot, "snippets.html");

        byte[] configBytes = JsonSerializer.SerializeToUtf8Bytes(_config, MediaBulkJsonContext.Default.GalleryConfig);
        byte[] galleryJsonBytes = JsonSerializer.SerializeToUtf8Bytes(galleryDoc, MediaBulkJsonContext.Default.GalleryOutputDocument);
        byte[] indexHtmlBytes = Encoding.UTF8.GetBytes(RenderGalleryHtml(_config, visibleItems));

        await using var cfgSink = FileStagedOutputAdapter.ForFixedPath(configJsonPath, outRoot);
        await using var jsonSink = FileStagedOutputAdapter.ForFixedPath(galleryJsonPath, outRoot);
        await using var htmlSink = FileStagedOutputAdapter.ForFixedPath(indexHtmlPath, outRoot);

        await cfgSink.WriteAppendAsync(configBytes, cancellationToken).ConfigureAwait(false);
        await jsonSink.WriteAppendAsync(galleryJsonBytes, cancellationToken).ConfigureAwait(false);
        await htmlSink.WriteAppendAsync(indexHtmlBytes, cancellationToken).ConfigureAwait(false);

        var tx = new AtomicFileCommitTransaction(new[] { cfgSink, jsonSink, htmlSink }, _beforePublishAsync);
        await tx.CommitAllAsync().ConfigureAwait(false);

        return new GalleryBuildReport(
            indexHtmlPath,
            galleryJsonPath,
            configJsonPath,
            snippetsHtmlPath,
            visibleItems.Count,
            hiddenCount,
            bulkReport);
    }

    private static string FormatBytes(long bytes)
    {
        if (bytes >= 1024L * 1024L)
        {
            return string.Format(CultureInfo.InvariantCulture, "{0:F2} MB", bytes / (1024.0 * 1024.0));
        }
        if (bytes >= 1024L)
        {
            return string.Format(CultureInfo.InvariantCulture, "{0:F1} KB", bytes / 1024.0);
        }
        return string.Create(CultureInfo.InvariantCulture, $"{bytes} B");
    }
}
