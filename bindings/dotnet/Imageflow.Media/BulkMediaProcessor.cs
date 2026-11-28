using System;
using System.Buffers;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;

namespace Imageflow.Media;

/// <summary>
/// Resolved per-format coding policy computed UP FRONT from the provider capability truth table so
/// runtime encode failures (<c>TargetUnmet</c>, <c>PolicyRefusal</c>) are never masked.
/// </summary>
public sealed record ResolvedVariantCoding(
    string CodingJson,
    string RequestedLabel,
    string EffectiveLabel,
    string? FallbackReason);

/// <summary>One deduplicated, no-upscale target breakpoint for a source image.</summary>
public sealed record PlannedBreakpoint(
    int Width,
    int Height,
    FitMode FitMode,
    bool BypassResize);

/// <summary>One output slot inside a single-decode multi-output <c>v2/execute</c> DAG.</summary>
public sealed record PlannedOutputSlot(
    string SlotName,
    int IoId,
    PlannedBreakpoint Breakpoint,
    OutputFormat Format,
    ResolvedVariantCoding ResolvedCoding);

/// <summary>Configuration for <see cref="BulkMediaProcessor.ProcessAsync"/>.</summary>
public sealed record BulkProcessOptions
{
    public required IReadOnlyList<string> Inputs { get; init; }
    public required string OutputDirectory { get; init; }
    public ResponsivePreset Preset { get; init; } = ResponsivePreset.WebResponsive();
    public bool Recursive { get; init; } = true;
    public bool FollowSymlinks { get; init; }
    public OutputPathLayout OutputLayout { get; init; } = OutputPathLayout.ContentAddressed;
    public int Concurrency { get; init; } = Math.Clamp(Environment.ProcessorCount, 1, 4);
    public bool IncrementalCache { get; init; } = true;
    public bool ForceRebuild { get; init; }
    public FailurePolicy OnError { get; init; } = FailurePolicy.Continue;
    public bool WriteManifest { get; init; } = true;
    public bool WriteSnippets { get; init; } = true;
    public BulkLimits Limits { get; init; } = BulkLimits.Default;

    /// <summary>
    /// Optional lifecycle hook invoked immediately before atomic file publication inside
    /// <see cref="AtomicFileCommitTransaction"/> (useful for custom verification or failure-injection tests).
    /// </summary>
    public Func<CancellationToken, ValueTask>? BeforeCommitPublishAsync { get; init; }
}

/// <summary>
/// High-level, NativeAOT-safe bulk media processor built over <see cref="MediaJob"/> single-decode
/// multi-output streaming DAGs, atomic staged file publication, and incremental content-hash caching.
/// </summary>
public static class BulkMediaProcessor
{
    private const string CacheFileName = ".imageflow-cache.json";
    private const string CacheSchema = "imageflow-bulk-cache-v1";

    private static readonly HashSet<string> SupportedImageExts = new(StringComparer.OrdinalIgnoreCase)
    {
        ".jpg", ".jpeg", ".png", ".webp", ".avif", ".gif", ".jxl", ".bmp",
    };

    private static readonly HashSet<string> SupportedVideoExts = new(StringComparer.OrdinalIgnoreCase)
    {
        ".mp4", ".webm", ".ivf",
    };

    /// <summary>
    /// Resolve the exact <c>zenpipe</c> <c>encode.coding</c> JSON object up front for
    /// <paramref name="format"/> under <paramref name="workspacePolicy"/>, matching Rust
    /// <c>imageflow_tool_lib::bulk::resolve_variant_coding_with_caps</c>.
    /// </summary>
    public static ResolvedVariantCoding ResolveVariantCoding(
        OutputFormat format,
        QualityTarget quality,
        QualityFallbackPolicy fallbackPolicy,
        WorkspaceAdmission workspacePolicy,
        bool hasZensimAIqa = true)
    {
        ArgumentNullException.ThrowIfNull(quality);
        quality.Validate();
        string requested = quality.DisplayLabel;

        if (MediaWireTokens.IsLosslessOnly(format))
        {
            string? reason = quality.Mode == QualityTargetMode.Lossless ? null : "lossless_format_uses_lossless";
            return new ResolvedVariantCoding("\"lossless\"", requested, "lossless", reason);
        }

        switch (quality.Mode)
        {
            case QualityTargetMode.Lossless:
                return new ResolvedVariantCoding("\"lossless\"", requested, "lossless", null);

            case QualityTargetMode.CalibratedQuality:
            {
                float q = quality.Quality!.Value;
                string json = WriteJsonObject(w => w.WriteNumber("calibrated_quality", q));
                string eff = string.Format(CultureInfo.InvariantCulture, "calibrated_quality({0:F1})", q);
                return new ResolvedVariantCoding(json, requested, eff, null);
            }

            case QualityTargetMode.QualityProfile:
            {
                string level = MediaWireTokens.ToWire(quality.ProfileLevel!.Value);
                float dpr = quality.Dpr;
                string json = WriteJsonObject(w =>
                {
                    w.WriteStartObject("quality_profile");
                    w.WriteString("level", level);
                    w.WriteNumber("dpr", dpr);
                    w.WriteEndObject();
                });
                string eff = string.Format(CultureInfo.InvariantCulture, "profile({0}@{1:F1}x)", level, dpr);
                return new ResolvedVariantCoding(json, requested, eff, null);
            }

            case QualityTargetMode.VerifyZensimA:
            {
                float target = quality.Target!.Value;
                bool canVerify = hasZensimAIqa && format switch
                {
                    OutputFormat.Jpeg => true,
                    OutputFormat.Webp => workspacePolicy == WorkspaceAdmission.BestEffort,
                    _ => false,
                };
                if (canVerify)
                {
                    string json = WriteJsonObject(w =>
                    {
                        w.WriteStartObject("verify");
                        w.WriteString("metric", "zensim_a");
                        w.WriteNumber("value", target);
                        w.WriteEndObject();
                    });
                    string eff = string.Format(CultureInfo.InvariantCulture, "verify(zensim_a>={0:F1})", target);
                    return new ResolvedVariantCoding(json, requested, eff, null);
                }

                string reason = !hasZensimAIqa
                    ? "iqa_zensim_a_unavailable"
                    : format switch
                    {
                        OutputFormat.Webp => "webp_verify_requires_best_effort_workspace",
                        OutputFormat.Avif => "avif_encoder_verify_unavailable",
                        OutputFormat.Jxl => "jxl_encoder_verify_unavailable",
                        _ => "format_verify_unavailable",
                    };
                if (fallbackPolicy == QualityFallbackPolicy.StrictVerify)
                {
                    throw new InvalidOperationException(
                        $"strict_verify refused format '{MediaWireTokens.ToWire(format)}' under workspace '{MediaWireTokens.ToWire(workspacePolicy)}': {reason}");
                }
                string fbJson = WriteJsonObject(w => w.WriteNumber("calibrated_quality", target));
                string fbEff = string.Format(CultureInfo.InvariantCulture, "calibrated_quality({0:F1})", target);
                return new ResolvedVariantCoding(fbJson, requested, fbEff, reason);
            }

            case QualityTargetMode.AimForZensimA:
            {
                float target = quality.Target!.Value;
                // In pinned zenpipe, AimFor computes a calibrated quality seed from the encoder's
                // fidelity model without requiring a registered Verify IQA scorer (JPEG + WebP).
                bool canAim = format == OutputFormat.Jpeg || format == OutputFormat.Webp;
                if (canAim)
                {
                    string json = WriteJsonObject(w =>
                    {
                        w.WriteStartObject("aim_for");
                        w.WriteString("metric", "zensim_a");
                        w.WriteNumber("value", target);
                        w.WriteEndObject();
                    });
                    string eff = string.Format(CultureInfo.InvariantCulture, "aim_for(zensim_a~{0:F1}) [estimated]", target);
                    return new ResolvedVariantCoding(json, requested, eff, null);
                }
                if (fallbackPolicy == QualityFallbackPolicy.StrictVerify)
                {
                    throw new InvalidOperationException(
                        $"strict_verify refused aim_for(zensim_a) on format '{MediaWireTokens.ToWire(format)}'.");
                }
                string fbJson = WriteJsonObject(w => w.WriteNumber("calibrated_quality", target));
                string fbEff = string.Format(CultureInfo.InvariantCulture, "calibrated_quality({0:F1})", target);
                return new ResolvedVariantCoding(fbJson, requested, fbEff, "format_aim_for_unavailable");
            }

            default:
                throw new ArgumentOutOfRangeException(nameof(quality));
        }
    }

    /// <summary>
    /// Plan deduplicated, no-upscale target breakpoints for upright source dimensions
    /// (<paramref name="srcWidth"/> x <paramref name="srcHeight"/>), matching Rust
    /// <c>imageflow_tool_lib::bulk::plan_breakpoints</c>.
    /// </summary>
    public static IReadOnlyList<PlannedBreakpoint> PlanBreakpoints(
        int srcWidth,
        int srcHeight,
        ResponsivePreset preset)
    {
        ArgumentNullException.ThrowIfNull(preset);
        int sw = Math.Max(1, srcWidth);
        int sh = Math.Max(1, srcHeight);

        int[] sortedWidths = preset.WidthsPx.Distinct().OrderBy(w => w).ToArray();
        var planned = new List<PlannedBreakpoint>(sortedWidths.Length);

        foreach (int reqW in sortedWidths)
        {
            int tw;
            int th;
            bool bypass;
            if (preset.AspectRatio is AspectRatio ar)
            {
                long idealH = Math.Max(1L, ((long)reqW * ar.Height + ar.Width / 2) / ar.Width);
                if (preset.AllowUpscale)
                {
                    tw = reqW;
                    th = checked((int)idealH);
                    bypass = tw == sw && th == sh;
                }
                else
                {
                    int maxWFromH = (int)Math.Max(1L, (long)sh * ar.Width / ar.Height);
                    int clampedW = Math.Max(1, Math.Min(reqW, Math.Min(sw, maxWFromH)));
                    long clampedHL = ((long)clampedW * ar.Height + ar.Width / 2) / ar.Width;
                    int clampedH = (int)Math.Clamp(clampedHL, 1L, sh);
                    tw = clampedW;
                    th = clampedH;
                    bypass = tw == sw && th == sh;
                }
            }
            else
            {
                tw = Math.Max(1, preset.AllowUpscale ? reqW : Math.Min(reqW, sw));
                long thL = Math.Max(1L, ((long)tw * sh + sw / 2) / sw);
                th = checked((int)(preset.AllowUpscale ? thL : Math.Min(thL, sh)));
                bypass = tw == sw && th == sh;
            }

            bool dup = false;
            for (int i = 0; i < planned.Count; i++)
            {
                if (planned[i].Width == tw && planned[i].Height == th)
                {
                    dup = true;
                    break;
                }
            }
            if (!dup)
            {
                planned.Add(new PlannedBreakpoint(tw, th, preset.FitMode, bypass));
            }
        }

        if (planned.Count == 0)
        {
            planned.Add(new PlannedBreakpoint(sw, sh, preset.FitMode, BypassResize: true));
        }
        return planned;
    }

    /// <summary>
    /// Build a canonical single-decode, single-resize-per-breakpoint, multi-format fan-out
    /// <c>v2/execute</c> JSON envelope (zero reflection, NativeAOT safe).
    /// </summary>
    public static (string EnvelopeJson, IReadOnlyList<PlannedOutputSlot> Slots) BuildFanoutEnvelope(
        IReadOnlyList<PlannedBreakpoint> breakpoints,
        ResponsivePreset preset,
        BulkLimits limits,
        bool selectFirstPresentation = false,
        bool hasZensimAIqa = true)
    {
        ArgumentNullException.ThrowIfNull(breakpoints);
        ArgumentNullException.ThrowIfNull(preset);
        ArgumentNullException.ThrowIfNull(limits);

        var slots = new List<PlannedOutputSlot>(breakpoints.Count * preset.Formats.Count);
        var edges = new List<(string From, string To)>();

        var buffer = new ArrayBufferWriter<byte>(4096);
        using (var w = new Utf8JsonWriter(buffer))
        {
            w.WriteStartObject();
            w.WriteStartObject("inputs");
            w.WriteNumber("src", 0);
            w.WriteEndObject();

            int nextIoId = 1;
            var slotRows = new List<(int BreakpointIndex, PlannedBreakpoint Breakpoint, string Upstream, PlannedOutputSlot Slot)>();

            // Pre-plan output slots and edge topology.
            for (int bi = 0; bi < breakpoints.Count; bi++)
            {
                PlannedBreakpoint bp = breakpoints[bi];
                string upstream = "dec";
                if (!bp.BypassResize)
                {
                    string geoName = $"geo_{bi}";
                    edges.Add((upstream, geoName));
                    upstream = geoName;
                    if (preset.SharpenAmount is float amt && amt > 0.0f)
                    {
                        string sharpName = $"sharp_{bi}";
                        edges.Add((upstream, sharpName));
                        upstream = sharpName;
                    }
                }

                foreach (OutputFormat fmt in preset.Formats)
                {
                    ResolvedVariantCoding resolved = ResolveVariantCoding(
                        fmt, preset.Quality, preset.QualityFallback, preset.WorkspacePolicy, hasZensimAIqa);
                    string ext = MediaWireTokens.Extension(fmt);
                    string slotName = $"v_{bi}_{ext}";
                    string encName = $"enc_{bi}_{ext}";
                    int ioId = nextIoId++;
                    var slot = new PlannedOutputSlot(slotName, ioId, bp, fmt, resolved);
                    slots.Add(slot);
                    edges.Add((upstream, encName));
                    slotRows.Add((bi, bp, encName, slot));
                }
            }

            w.WriteStartObject("outputs");
            foreach (PlannedOutputSlot slot in slots)
            {
                w.WriteNumber(slot.SlotName, slot.IoId);
            }
            w.WriteEndObject();

            w.WriteStartObject("limits");
            w.WriteNumber("max_memory_bytes", limits.MaxMemoryBytes);
            w.WriteNumber("timeout_ms", limits.TimeoutMs);
            w.WriteNumber("max_dimension", limits.MaxDimension);
            w.WriteEndObject();

            w.WriteStartObject("pipeline");
            w.WriteStartObject("graph");
            w.WriteStartObject("nodes");

            // Decode node
            w.WriteStartObject("dec");
            w.WriteStartObject("decode");
            w.WriteString("input", "src");
            w.WriteString("orientation", "apply");
            if (selectFirstPresentation)
            {
                w.WriteStartObject("select");
                w.WriteStartObject("presentation_index");
                w.WriteNumber("index", 0);
                w.WriteEndObject();
                w.WriteEndObject();
            }
            w.WriteEndObject();
            w.WriteEndObject();

            // Geometry & optional sharpen nodes
            for (int bi = 0; bi < breakpoints.Count; bi++)
            {
                PlannedBreakpoint bp = breakpoints[bi];
                if (!bp.BypassResize)
                {
                    w.WriteStartObject($"geo_{bi}");
                    w.WriteStartObject("geometry");
                    w.WriteStartObject("constrain");
                    w.WriteString("mode", MediaWireTokens.ToWire(bp.FitMode));
                    w.WriteNumber("width", bp.Width);
                    w.WriteNumber("height", bp.Height);
                    w.WriteEndObject();
                    w.WriteEndObject();
                    w.WriteEndObject();

                    if (preset.SharpenAmount is float amt && amt > 0.0f)
                    {
                        w.WriteStartObject($"sharp_{bi}");
                        w.WriteStartObject("filter");
                        w.WriteStartObject("sharpen");
                        w.WriteNumber("amount", amt);
                        w.WriteNumber("radius", 1.0);
                        w.WriteEndObject();
                        w.WriteEndObject();
                        w.WriteEndObject();
                    }
                }
            }

            // Encode nodes
            foreach ((_, _, string encName, PlannedOutputSlot slot) in slotRows)
            {
                w.WriteStartObject(encName);
                w.WriteStartObject("encode");
                w.WriteString("output", slot.SlotName);
                w.WriteString("format", MediaWireTokens.WireFormat(slot.Format));
                w.WritePropertyName("coding");
                w.WriteRawValue(slot.ResolvedCoding.CodingJson);
                w.WriteString("effort", MediaWireTokens.ToWire(preset.Effort));
                w.WriteString("metadata", MediaWireTokens.ToWire(preset.Metadata));
                w.WriteEndObject();
                w.WriteEndObject();
            }

            w.WriteEndObject(); // nodes

            w.WriteStartArray("edges");
            foreach ((string from, string to) in edges)
            {
                w.WriteStartObject();
                w.WriteString("from", from);
                w.WriteString("to", to);
                w.WriteEndObject();
            }
            w.WriteEndArray();

            w.WriteEndObject(); // graph
            w.WriteEndObject(); // pipeline
            w.WriteEndObject();
        }

        return (Encoding.UTF8.GetString(buffer.WrittenSpan), slots);
    }

    /// <summary>Contextually escape HTML text and attribute strings.</summary>
    public static string EscapeHtml(string input)
    {
        if (string.IsNullOrEmpty(input))
        {
            return string.Empty;
        }
        var sb = new StringBuilder(input.Length + 8);
        foreach (char ch in input)
        {
            switch (ch)
            {
                case '&': sb.Append("&amp;"); break;
                case '<': sb.Append("&lt;"); break;
                case '>': sb.Append("&gt;"); break;
                case '"': sb.Append("&quot;"); break;
                case '\'': sb.Append("&#39;"); break;
                default: sb.Append(ch); break;
            }
        }
        return sb.ToString();
    }

    /// <summary>
    /// Render a standards-compliant responsive HTML <c>&lt;picture&gt;</c> element from actual emitted
    /// variants, ordering modern formats (<c>avif</c>, <c>jxl</c>, <c>webp</c>) ahead of <c>&lt;img&gt;</c>.
    /// </summary>
    public static string RenderPictureSnippet(
        ManifestAsset asset,
        string urlPrefix = "",
        string sizesAttr = "(max-width: 768px) 100vw, 50vw",
        string? altText = null,
        bool eager = false)
    {
        ArgumentNullException.ThrowIfNull(asset);
        if (asset.Variants.Count == 0)
        {
            return string.Empty;
        }

        var groups = asset.Variants
            .GroupBy(v => v.Format)
            .OrderBy(g => MediaWireTokens.PictureSourcePriority(g.Key))
            .ThenBy(g => g.Key)
            .Select(g => (Format: g.Key, Items: g.OrderBy(v => v.Width).ToList()))
            .ToList();

        OutputFormat fallbackFormat = groups.Any(g => g.Format == OutputFormat.Jpeg)
            ? OutputFormat.Jpeg
            : groups.Any(g => g.Format == OutputFormat.Png)
                ? OutputFormat.Png
                : groups[^1].Format;

        string JoinUrl(string rel)
        {
            if (string.IsNullOrEmpty(urlPrefix))
            {
                return rel;
            }
            return $"{urlPrefix.TrimEnd('/')}/{rel.TrimStart('/')}";
        }

        string FormatSrcset(List<ManifestVariant> list) =>
            string.Join(", ", list.Select(v => string.Create(CultureInfo.InvariantCulture, $"{EscapeHtml(JoinUrl(v.RelativePath))} {v.Width}w")));

        string sizesEsc = EscapeHtml(sizesAttr);
        var sb = new StringBuilder(512);
        sb.Append("<picture>\n");
        foreach ((OutputFormat fmt, List<ManifestVariant> list) in groups)
        {
            if (fmt == fallbackFormat)
            {
                continue;
            }
            sb.Append($"  <source type=\"{MediaWireTokens.MimeType(fmt)}\" srcset=\"{FormatSrcset(list)}\" sizes=\"{sizesEsc}\">\n");
        }

        List<ManifestVariant> fbList = groups.First(g => g.Format == fallbackFormat).Items;
        ManifestVariant defaultV = fbList[^1];
        string loading = eager ? "eager" : "lazy";
        string alt = altText ?? asset.SourceRelativePath;
        sb.Append(string.Create(
            CultureInfo.InvariantCulture,
            $"  <img src=\"{EscapeHtml(JoinUrl(defaultV.RelativePath))}\" srcset=\"{FormatSrcset(fbList)}\" sizes=\"{sizesEsc}\" width=\"{defaultV.Width}\" height=\"{defaultV.Height}\" alt=\"{EscapeHtml(alt)}\" loading=\"{loading}\" decoding=\"async\">\n"));
        sb.Append("</picture>");
        return sb.ToString();
    }

    /// <summary>
    /// Execute a bulk responsive media job using caller-supplied Imageflow contexts via
    /// <see cref="MediaJob"/>, atomic staged file publication, and incremental content-hash caching.
    /// </summary>
    public static async Task<BulkProcessingReport> ProcessAsync(
        Func<SafeHandle> contextFactory,
        BulkProcessOptions options,
        IProgress<BulkItemOutcome>? progress = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(contextFactory);
        ArgumentNullException.ThrowIfNull(options);
        if (options.Inputs is null || options.Inputs.Count == 0)
        {
            throw new ArgumentException("At least one input file or directory is required.", nameof(options));
        }
        if (string.IsNullOrWhiteSpace(options.OutputDirectory))
        {
            throw new ArgumentException("OutputDirectory is required.", nameof(options));
        }
        options.Preset.Validate();

        // Query live native capabilities once so Verify/AimFor preflight reflects the loaded build.
        (string engineFp, bool hasZensimAIqa) = QueryEngineCapabilities(contextFactory);

        // Pre-validate every requested format coding UP FRONT so StrictVerify fails before touching disk.
        foreach (OutputFormat fmt in options.Preset.Formats)
        {
            _ = ResolveVariantCoding(fmt, options.Preset.Quality, options.Preset.QualityFallback, options.Preset.WorkspacePolicy, hasZensimAIqa);
        }

        var sw = Stopwatch.StartNew();
        string outputRoot = Path.GetFullPath(options.OutputDirectory);
        FileStagedOutputAdapter.CreateConfinedDirectory(outputRoot, outputRoot);

        List<DiscoveredMediaFile> discovered = DiscoverMediaFiles(
            options.Inputs, outputRoot, options.Recursive, options.FollowSymlinks);

        string recipeHash = ComputeRecipeFingerprint(options.Preset, options.OutputLayout);

        string cachePath = Path.Combine(outputRoot, CacheFileName);
        BulkCacheDocument cache = LoadCache(cachePath, engineFp, options.IncrementalCache && !options.ForceRebuild);

        var outcomes = new BulkItemOutcome?[discovered.Count];
        var newCacheEntries = new ConcurrentDictionary<string, CachedAssetEntry>(StringComparer.Ordinal);
        // Under FailFast with mutable layouts (Flatten / PreserveTree), defer overwriting prior assets
        // until ALL sources succeed so a late failure never leaves the old manifest pointing at new bytes!
        bool deferBatchPublish = options.OnError == FailurePolicy.FailFast
            && options.OutputLayout != OutputPathLayout.ContentAddressed;
        var deferredBatchSinks = new ConcurrentBag<FileStagedOutputAdapter>();
        int processedCount = 0;
        int cachedCount = 0;
        int failedCount = 0;
        var errors = new ConcurrentQueue<string>();

        using var linkedCts = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        var parallelOpts = new ParallelOptions
        {
            MaxDegreeOfParallelism = Math.Clamp(options.Concurrency, 1, 64),
            CancellationToken = linkedCts.Token,
        };

        try
        {
            await Parallel.ForEachAsync(
                Enumerable.Range(0, discovered.Count),
                parallelOpts,
                async (index, ct) =>
                {
                    ct.ThrowIfCancellationRequested();
                    DiscoveredMediaFile file = discovered[index];
                    bool itemCommitted = false;
                    try
                    {
                        cache.Entries.TryGetValue(file.RelativeKey, out CachedAssetEntry? cachedEntry);
                        (ManifestAsset asset, bool wasCached, string srcSha, List<FileStagedOutputAdapter> deferred) = await ProcessSingleAsync(
                            contextFactory,
                            file,
                            outputRoot,
                            options,
                            recipeHash,
                            hasZensimAIqa,
                            cachedEntry,
                            deferBatchPublish,
                            () => itemCommitted = true,
                            ct).ConfigureAwait(false);

                        foreach (FileStagedOutputAdapter s in deferred)
                        {
                            deferredBatchSinks.Add(s);
                        }

                        newCacheEntries[file.RelativeKey] = new CachedAssetEntry
                        {
                            SourceSha256 = srcSha,
                            RecipeSha256 = recipeHash,
                            Asset = asset,
                        };

                        long outBytes = asset.Variants.Sum(v => v.Bytes) + asset.VideoVariants.Sum(v => v.Bytes);
                        if (wasCached)
                        {
                            Interlocked.Increment(ref cachedCount);
                        }
                        else
                        {
                            Interlocked.Increment(ref processedCount);
                        }

                        var outcome = new BulkItemOutcome(
                            index + 1,
                            discovered.Count,
                            file.RelativeKey,
                            wasCached ? BulkCommitState.ReusedFromCache : BulkCommitState.Committed,
                            asset.Variants.Count + asset.VideoVariants.Count,
                            asset.SourceBytes,
                            outBytes,
                            null,
                            asset);
                        outcomes[index] = outcome;
                        progress?.Report(outcome);
                    }
                    catch (OperationCanceledException)
                    {
                        throw;
                    }
                    catch (Exception ex)
                    {
                        Interlocked.Increment(ref failedCount);
                        string msg = $"{file.RelativeKey}: {ex.Message}";
                        errors.Enqueue(msg);
                        var outcome = new BulkItemOutcome(
                            index + 1,
                            discovered.Count,
                            file.RelativeKey,
                            itemCommitted ? BulkCommitState.Committed : BulkCommitState.RolledBack,
                            0,
                            file.SizeBytes,
                            0,
                            msg,
                            null);
                        outcomes[index] = outcome;
                        progress?.Report(outcome);

                        if (options.OnError == FailurePolicy.FailFast)
                        {
                            await linkedCts.CancelAsync().ConfigureAwait(false);
                        }
                    }
                }).ConfigureAwait(false);
        }
        catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
        {
            await DisposeSinksAsync(deferredBatchSinks).ConfigureAwait(false);
            throw;
        }
        catch (OperationCanceledException) when (options.OnError == FailurePolicy.FailFast && !errors.IsEmpty)
        {
            // Fail-fast cancellation triggered by item failure; report below.
        }

        if (options.OnError == FailurePolicy.FailFast && !errors.IsEmpty)
        {
            await DisposeSinksAsync(deferredBatchSinks).ConfigureAwait(false);
            throw new InvalidOperationException($"Bulk processing aborted (fail_fast): {string.Join("; ", errors)}");
        }

        if (!deferredBatchSinks.IsEmpty)
        {
            FileStagedOutputAdapter[] batchSinks = deferredBatchSinks.ToArray();
            try
            {
                var batchTx = new AtomicFileCommitTransaction(batchSinks, options.BeforeCommitPublishAsync);
                await batchTx.CommitAllAsync().ConfigureAwait(false);
            }
            finally
            {
                await DisposeSinksAsync(batchSinks).ConfigureAwait(false);
            }
        }

        var assets = outcomes
            .Where(o => o?.Asset is not null)
            .Select(o => o!.Asset!)
            .OrderBy(a => a.SourceRelativePath, StringComparer.Ordinal)
            .ToList();

        long totalIn = assets.Sum(a => a.SourceBytes);
        long totalOut = assets.Sum(a => a.Variants.Sum(v => v.Bytes) + a.VideoVariants.Sum(v => v.Bytes));

        var manifest = new BulkManifest
        {
            Schema = MediaSchemaVersions.Manifest,
            GeneratedAtUtc = DateTimeOffset.UtcNow.ToString("yyyy-MM-ddTHH:mm:ssZ", CultureInfo.InvariantCulture),
            Generator = "Imageflow.Media",
            Preset = options.Preset,
            TotalInputBytes = totalIn,
            TotalOutputBytes = totalOut,
            Assets = assets,
        };

        // Stage metadata files atomically confined inside outputRoot.
        if (options.IncrementalCache)
        {
            foreach (KeyValuePair<string, CachedAssetEntry> kv in newCacheEntries)
            {
                cache.Entries[kv.Key] = kv.Value;
            }
            cache.Schema = CacheSchema;
            cache.EngineFingerprint = engineFp;
            byte[] cacheBytes = JsonSerializer.SerializeToUtf8Bytes(cache, MediaBulkJsonContext.Default.BulkCacheDocument);
            await AtomicFileCommitTransaction.WriteFileAtomicAsync(cachePath, cacheBytes, null, cancellationToken, outputRoot).ConfigureAwait(false);
        }

        string? manifestPath = null;
        if (options.WriteManifest)
        {
            manifestPath = Path.Combine(outputRoot, "manifest.json");
            byte[] manifestBytes = JsonSerializer.SerializeToUtf8Bytes(manifest, MediaBulkJsonContext.Default.BulkManifest);
            await AtomicFileCommitTransaction.WriteFileAtomicAsync(manifestPath, manifestBytes, null, cancellationToken, outputRoot).ConfigureAwait(false);
        }

        string? snippetsPath = null;
        if (options.WriteSnippets)
        {
            snippetsPath = Path.Combine(outputRoot, "snippets.html");
            var sb = new StringBuilder(assets.Count * 512);
            sb.Append("<!-- Generated by Imageflow.Media responsive bulk processor -->\n");
            foreach (ManifestAsset a in assets)
            {
                string safeId = SanitizeAsciiId(a.Id);
                string snippet = RenderPictureSnippet(a, "", "(max-width: 768px) 100vw, 50vw", safeId, eager: false);
                sb.Append($"<!-- asset: {safeId} -->\n");
                sb.Append(snippet);
                sb.Append("\n\n");
            }
            byte[] snipBytes = Encoding.UTF8.GetBytes(sb.ToString());
            await AtomicFileCommitTransaction.WriteFileAtomicAsync(snippetsPath, snipBytes, null, cancellationToken, outputRoot).ConfigureAwait(false);
        }

        return new BulkProcessingReport(
            discovered.Count,
            processedCount,
            cachedCount,
            failedCount,
            totalIn,
            totalOut,
            sw.Elapsed,
            manifestPath,
            snippetsPath,
            errors.ToArray(),
            manifest);
    }

    private static async Task DisposeSinksAsync(IEnumerable<FileStagedOutputAdapter> sinks)
    {
        foreach (FileStagedOutputAdapter s in sinks)
        {
            await s.DisposeAsync().ConfigureAwait(false);
        }
    }

    private static async Task<(ManifestAsset Asset, bool WasCached, string SourceSha, List<FileStagedOutputAdapter> DeferredSinks)> ProcessSingleAsync(
        Func<SafeHandle> contextFactory,
        DiscoveredMediaFile item,
        string outputRoot,
        BulkProcessOptions options,
        string recipeHash,
        bool hasZensimAIqa,
        CachedAssetEntry? cached,
        bool deferBatchPublish,
        Action markCommitted,
        CancellationToken ct)
    {
        byte[] sourceBytes = options.FollowSymlinks
            ? await File.ReadAllBytesAsync(item.AbsolutePath, ct).ConfigureAwait(false)
            : await FileStagedOutputAdapter.ReadConfinedRegularFileAsync(item.RootDirectory, item.AbsolutePath, ct).ConfigureAwait(false);
        string srcSha = Sha256Hex(sourceBytes);

        if (options.IncrementalCache
            && !options.ForceRebuild
            && cached?.Asset is not null
            && string.Equals(cached.SourceSha256, srcSha, StringComparison.Ordinal)
            && string.Equals(cached.RecipeSha256, recipeHash, StringComparison.Ordinal)
            && VerifyCachedOnDisk(cached.Asset, outputRoot))
        {
            // Always regenerate PictureHtml from validated, confined variant paths (never trust cached HTML).
            ManifestAsset reused = cached.Asset with { Id = item.Id };
            reused = reused with { PictureHtml = RenderPictureSnippet(reused) };
            return (reused, true, srcSha, new List<FileStagedOutputAdapter>());
        }

        (int srcW, int srcH, string mediaKind, string sourceFormat) = await InspectSourceAsync(
            contextFactory, sourceBytes, item.IsVideoExtension, options.Preset.WorkspacePolicy, options.Limits, ct).ConfigureAwait(false);

        bool isSequence = string.Equals(mediaKind, "video", StringComparison.Ordinal)
            || string.Equals(mediaKind, "animation", StringComparison.Ordinal);

        IReadOnlyList<PlannedBreakpoint> breakpoints = PlanBreakpoints(srcW, srcH, options.Preset);
        (string envelopeJson, IReadOnlyList<PlannedOutputSlot> slots) = BuildFanoutEnvelope(
            breakpoints, options.Preset, options.Limits, selectFirstPresentation: isSequence, hasZensimAIqa: hasZensimAIqa);

        string stagingDir = Path.Combine(outputRoot, "assets");
        FileStagedOutputAdapter.CreateConfinedDirectory(outputRoot, stagingDir);

        var allSourceSinks = new List<FileStagedOutputAdapter>(slots.Count + options.Preset.VideoFormats.Count);
        var posterSinks = new List<FileStagedOutputAdapter>(slots.Count);
        var destMap = new Dictionary<int, IMediaOutputAdapter>(slots.Count);
        var plannedRelBySlot = new Dictionary<int, string>(slots.Count);
        bool transferSinksToCaller = false;

        using SafeHandle execCtx = contextFactory();
        ConfigureContext(execCtx, options.Preset.WorkspacePolicy, options.Limits);

        using var inputMs = new MemoryStream(sourceBytes, writable: false);
        var inputAdapter = new StreamInputAdapter(inputMs, leaveOpen: true);

        try
        {
            foreach (PlannedOutputSlot slot in slots)
            {
                PlannedOutputSlot captured = slot;
                // Use immutable final paths resolved strictly BEFORE native commit (zero post-commit moves!).
                var sink = FileStagedOutputAdapter.ForResolvedPath(
                    stagingDir,
                    shaHex =>
                    {
                        string rel = VariantRelativePath(
                            options.OutputLayout,
                            item.RelativeKey,
                            captured.Breakpoint.Width,
                            captured.Breakpoint.Height,
                            MediaWireTokens.Extension(captured.Format),
                            shaHex[..12]);
                        plannedRelBySlot[captured.IoId] = rel;
                        return Path.Combine(outputRoot, rel.Replace('/', Path.DirectorySeparatorChar));
                    },
                    confinedRoot: outputRoot);
                posterSinks.Add(sink);
                allSourceSinks.Add(sink);
                destMap[slot.IoId] = sink;
            }

            // Stage posters & requested video transcodes together via DeferredCommitTransaction so a
            // late video transcode failure NEVER leaves partial poster variants published!
            var posterTx = new DeferredCommitTransaction(posterSinks);
            MediaResult result = await MediaJob.ExecuteAsync(
                execCtx,
                envelopeJson,
                new Dictionary<int, IMediaInputAdapter> { [0] = inputAdapter },
                destMap,
                posterTx,
                ct).ConfigureAwait(false);

            if (!result.Succeeded || !posterTx.NativeAuthorized)
            {
                await posterTx.RollbackAllAsync().ConfigureAwait(false);
                throw new InvalidOperationException($"v2/execute failed: {result.ResultJson}");
            }

            var outputMetaBySlot = ParseExecuteOutputs(result.ResultJson);
            var variants = new List<ManifestVariant>(slots.Count);
            for (int i = 0; i < slots.Count; i++)
            {
                PlannedOutputSlot slot = slots[i];
                FileStagedOutputAdapter sink = posterSinks[i];
                outputMetaBySlot.TryGetValue(slot.SlotName, out ExecuteOutputMeta? meta);

                int actualW = meta is { Width: > 0 } ? meta.Width : slot.Breakpoint.Width;
                int actualH = meta is { Height: > 0 } ? meta.Height : slot.Breakpoint.Height;
                string rel = plannedRelBySlot[slot.IoId];

                variants.Add(new ManifestVariant
                {
                    RelativePath = rel,
                    Format = slot.Format,
                    MimeType = MediaWireTokens.MimeType(slot.Format),
                    Width = actualW,
                    Height = actualH,
                    Bytes = sink.BytesWritten,
                    RequestedCoding = slot.ResolvedCoding.RequestedLabel,
                    EffectiveCoding = slot.ResolvedCoding.EffectiveLabel,
                    FallbackReason = slot.ResolvedCoding.FallbackReason,
                    VerifiedMetric = meta?.VerifiedMetric,
                    VerifiedScore = meta?.VerifiedScore,
                });
            }

            var videoVariants = new List<ManifestVariant>();
            if (isSequence && options.Preset.VideoFormats.Count > 0)
            {
                int? maxW = options.Preset.WidthsPx.Count > 0 ? options.Preset.WidthsPx.Max() : null;
                foreach (OutputFormat vfmt in options.Preset.VideoFormats)
                {
                    (ManifestVariant vv, FileStagedOutputAdapter vSink) = await StageVideoVariantAsync(
                        contextFactory, sourceBytes, item.RelativeKey, srcW, srcH, vfmt, maxW, outputRoot, stagingDir, options, ct).ConfigureAwait(false);
                    allSourceSinks.Add(vSink);
                    videoVariants.Add(vv);
                }
            }

            if (deferBatchPublish)
            {
                transferSinksToCaller = true;
            }
            else
            {
                var commitTx = new AtomicFileCommitTransaction(allSourceSinks, options.BeforeCommitPublishAsync);
                await commitTx.CommitAllAsync().ConfigureAwait(false);
                if (commitTx.IsCommitted)
                {
                    markCommitted();
                }
            }

            var asset = new ManifestAsset
            {
                Id = item.Id,
                SourceRelativePath = item.RelativeKey,
                SourceSha256 = srcSha,
                SourceBytes = sourceBytes.Length,
                MediaKind = mediaKind,
                SourceFormat = sourceFormat,
                OriginalWidth = srcW,
                OriginalHeight = srcH,
                Variants = variants,
                VideoVariants = videoVariants,
                PictureHtml = string.Empty,
            };
            asset = asset with { PictureHtml = RenderPictureSnippet(asset) };
            return (asset, false, srcSha, transferSinksToCaller ? allSourceSinks : new List<FileStagedOutputAdapter>());
        }
        finally
        {
            await inputAdapter.DisposeAsync().ConfigureAwait(false);
            if (!transferSinksToCaller)
            {
                foreach (FileStagedOutputAdapter sink in allSourceSinks)
                {
                    await sink.DisposeAsync().ConfigureAwait(false);
                }
            }
        }
    }

    private static async Task<(ManifestVariant Variant, FileStagedOutputAdapter StagedSink)> StageVideoVariantAsync(
        Func<SafeHandle> contextFactory,
        byte[] sourceBytes,
        string relKey,
        int srcW,
        int srcH,
        OutputFormat vfmt,
        int? maxWidth,
        string outputRoot,
        string stagingDir,
        BulkProcessOptions options,
        CancellationToken ct)
    {
        string vcodec = MediaWireTokens.VideoCodec(vfmt)
            ?? throw new InvalidOperationException($"Format '{vfmt}' is not a video format.");
        string envJson = WriteJsonObject(w =>
        {
            w.WriteStartObject("inputs");
            w.WriteNumber("src", 0);
            w.WriteEndObject();
            w.WriteStartObject("outputs");
            w.WriteNumber("vid", 1);
            w.WriteEndObject();
            w.WriteStartObject("limits");
            w.WriteNumber("max_memory_bytes", options.Limits.MaxMemoryBytes);
            w.WriteNumber("timeout_ms", options.Limits.TimeoutMs);
            w.WriteNumber("max_dimension", options.Limits.MaxDimension);
            w.WriteEndObject();
            w.WriteStartObject("pipeline");
            w.WriteStartArray("steps");
            w.WriteStartObject();
            w.WriteStartObject("decode");
            w.WriteString("input", "src");
            w.WriteString("select", "all");
            w.WriteEndObject();
            w.WriteEndObject();
            if (maxWidth is int mw)
            {
                w.WriteStartObject();
                w.WriteStartObject("geometry");
                w.WriteStartObject("constrain");
                w.WriteString("mode", "within");
                w.WriteNumber("width", mw);
                w.WriteEndObject();
                w.WriteEndObject();
                w.WriteEndObject();
            }
            w.WriteStartObject();
            w.WriteStartObject("encode");
            w.WriteString("output", "vid");
            w.WriteString("format", MediaWireTokens.WireFormat(vfmt));
            w.WriteString("video_codec", vcodec);
            w.WriteString("tracks", "drop_non_video");
            w.WriteEndObject();
            w.WriteEndObject();
            w.WriteEndArray();
            w.WriteEndObject();
        });

        using SafeHandle vctx = contextFactory();
        ConfigureContext(vctx, options.Preset.WorkspacePolicy, options.Limits);
        using var inMs = new MemoryStream(sourceBytes, writable: false);
        var inAdapter = new StreamInputAdapter(inMs, leaveOpen: true);
        string resolvedRel = string.Empty;
        var outSink = FileStagedOutputAdapter.ForResolvedPath(
            stagingDir,
            shaHex =>
            {
                resolvedRel = VariantRelativePath(
                    options.OutputLayout, relKey, srcW, srcH, MediaWireTokens.Extension(vfmt), shaHex[..12]);
                return Path.Combine(outputRoot, resolvedRel.Replace('/', Path.DirectorySeparatorChar));
            },
            confinedRoot: outputRoot);
        bool succeeded = false;
        try
        {
            var tx = new DeferredCommitTransaction(new[] { outSink });
            MediaResult vres = await MediaJob.ExecuteAsync(
                vctx,
                envJson,
                new Dictionary<int, IMediaInputAdapter> { [0] = inAdapter },
                new Dictionary<int, IMediaOutputAdapter> { [1] = outSink },
                tx,
                ct).ConfigureAwait(false);
            if (!vres.Succeeded || !tx.NativeAuthorized)
            {
                await tx.RollbackAllAsync().ConfigureAwait(false);
                throw new InvalidOperationException($"Video transcode ({vfmt}) failed: {vres.ResultJson}");
            }
            var metaMap = ParseExecuteOutputs(vres.ResultJson);
            metaMap.TryGetValue("vid", out ExecuteOutputMeta? meta);
            succeeded = true;
            return (new ManifestVariant
            {
                RelativePath = resolvedRel,
                Format = vfmt,
                MimeType = MediaWireTokens.MimeType(vfmt),
                Width = meta is { Width: > 0 } ? meta.Width : srcW,
                Height = meta is { Height: > 0 } ? meta.Height : srcH,
                Bytes = outSink.BytesWritten,
                RequestedCoding = "video_default",
                EffectiveCoding = $"{MediaWireTokens.WireFormat(vfmt)}/{vcodec}",
            }, outSink);
        }
        finally
        {
            await inAdapter.DisposeAsync().ConfigureAwait(false);
            if (!succeeded)
            {
                await outSink.DisposeAsync().ConfigureAwait(false);
            }
        }
    }

    private static async Task<(int Width, int Height, string Kind, string Container)> InspectSourceAsync(
        Func<SafeHandle> contextFactory,
        byte[] sourceBytes,
        bool isVideoExt,
        WorkspaceAdmission workspacePolicy,
        BulkLimits limits,
        CancellationToken ct)
    {
        using SafeHandle ctx = contextFactory();
        ConfigureContext(ctx, workspacePolicy, limits);

        string inspJson = WriteJsonObject(w =>
        {
            w.WriteStartObject("inputs");
            w.WriteNumber("src", 0);
            w.WriteEndObject();
            w.WriteString("input", "src");
            if (isVideoExt)
            {
                string container = (sourceBytes.Length >= 4 && sourceBytes[0] == 0x1A && sourceBytes[1] == 0x45 && sourceBytes[2] == 0xDF && sourceBytes[3] == 0xA3)
                    ? "webm"
                    : (sourceBytes.Length >= 4 && sourceBytes[0] == (byte)'D' && sourceBytes[1] == (byte)'K' && sourceBytes[2] == (byte)'I' && sourceBytes[3] == (byte)'F')
                        ? "ivf"
                        : "mp4";
                w.WriteStartObject("video");
                w.WriteString("container", container);
                w.WriteEndObject();
            }
            w.WriteString("scan", "headers");
        });

        using var ms = new MemoryStream(sourceBytes, writable: false);
        var srcAdapter = new StreamInputAdapter(ms, leaveOpen: true);
        try
        {
            MediaResult res = await MediaJob.InspectAsync(
                ctx,
                inspJson,
                new Dictionary<int, IMediaInputAdapter> { [0] = srcAdapter },
                ct).ConfigureAwait(false);
            if (!res.Succeeded)
            {
                throw new InvalidOperationException($"v2/inspect failed: {res.ResultJson}");
            }

            using JsonDocument doc = JsonDocument.Parse(res.ResultJson);
            JsonElement root = doc.RootElement.TryGetProperty("data", out JsonElement d) ? d : doc.RootElement;
            string container = root.TryGetProperty("container", out JsonElement cEl) ? cEl.GetString() ?? "unknown" : "unknown";
            if (!root.TryGetProperty("tracks", out JsonElement tracks) || tracks.ValueKind != JsonValueKind.Array || tracks.GetArrayLength() == 0)
            {
                throw new InvalidOperationException("v2/inspect returned no visual tracks.");
            }

            JsonElement chosen = tracks[0];
            foreach (JsonElement t in tracks.EnumerateArray())
            {
                if (t.TryGetProperty("kind", out JsonElement kEl))
                {
                    string? ks = kEl.GetString();
                    if (ks is "video" or "image" or "animation")
                    {
                        chosen = t;
                        break;
                    }
                }
            }

            string kind = chosen.TryGetProperty("kind", out JsonElement kindEl) ? kindEl.GetString() ?? "image" : "image";
            JsonElement dims = chosen.TryGetProperty("display", out JsonElement dispEl) && dispEl.ValueKind == JsonValueKind.Object
                ? dispEl
                : chosen.GetProperty("coded");
            int w = dims.GetProperty("width").GetInt32();
            int h = dims.GetProperty("height").GetInt32();
            if (w <= 0 || h <= 0)
            {
                throw new InvalidOperationException("v2/inspect returned zero dimensions.");
            }
            return (w, h, kind, container);
        }
        finally
        {
            await srcAdapter.DisposeAsync().ConfigureAwait(false);
        }
    }

    private static void ConfigureContext(SafeHandle ctx, WorkspaceAdmission workspacePolicy, BulkLimits limits)
    {
        string payload = WriteJsonObject(w =>
        {
            w.WriteString("workspace_policy", MediaWireTokens.ToWire(workspacePolicy));
            w.WriteStartObject("limits");
            w.WriteNumber("max_memory_bytes", limits.MaxMemoryBytes);
            w.WriteNumber("timeout_ms", limits.TimeoutMs);
            w.WriteNumber("max_dimension", limits.MaxDimension);
            w.WriteEndObject();
        });
        (long code, string json, bool hasErr) = Native.SendContextJson(ctx, "v2/configure_context", payload);
        if (code != 200 || hasErr)
        {
            throw new InvalidOperationException($"v2/configure_context failed (HTTP {code}): {json}");
        }
    }

    private static (string Fingerprint, bool HasZensimAIqa) QueryEngineCapabilities(Func<SafeHandle> contextFactory)
    {
        using SafeHandle ctx = contextFactory();
        (_, string json, _) = Native.SendContextJson(ctx, "v2/capabilities", "{}");
        bool hasZensimA = false;
        try
        {
            using JsonDocument doc = JsonDocument.Parse(json);
            JsonElement root = doc.RootElement.TryGetProperty("data", out JsonElement d) ? d : doc.RootElement;
            if (root.TryGetProperty("verify_metrics", out JsonElement vm) && vm.ValueKind == JsonValueKind.Array)
            {
                foreach (JsonElement el in vm.EnumerateArray())
                {
                    if (string.Equals(el.GetString(), "zensim_a", StringComparison.Ordinal))
                    {
                        hasZensimA = true;
                        break;
                    }
                }
            }
        }
        catch
        {
            hasZensimA = false;
        }
        return (Sha256Hex(Encoding.UTF8.GetBytes(json))[..16], hasZensimA);
    }

    private static string ComputeRecipeFingerprint(ResponsivePreset preset, OutputPathLayout layout)
    {
        string presetJson = JsonSerializer.Serialize(preset, MediaBulkJsonContext.Default.ResponsivePreset);
        string combined = presetJson + "|" + MediaWireTokens.ToWire(layout);
        return Sha256Hex(Encoding.UTF8.GetBytes(combined));
    }

    private static BulkCacheDocument LoadCache(string cachePath, string expectedEngineFp, bool enabled)
    {
        if (!enabled || !File.Exists(cachePath))
        {
            return new BulkCacheDocument { Schema = CacheSchema, EngineFingerprint = expectedEngineFp };
        }
        try
        {
            byte[] raw = File.ReadAllBytes(cachePath);
            BulkCacheDocument? loaded = JsonSerializer.Deserialize(raw, MediaBulkJsonContext.Default.BulkCacheDocument);
            if (loaded is not null
                && string.Equals(loaded.Schema, CacheSchema, StringComparison.Ordinal)
                && string.Equals(loaded.EngineFingerprint, expectedEngineFp, StringComparison.Ordinal))
            {
                return loaded;
            }
        }
        catch
        {
            // Ignore corrupt/stale cache; rebuild cleanly.
        }
        return new BulkCacheDocument { Schema = CacheSchema, EngineFingerprint = expectedEngineFp };
    }

    private static bool IsSafeVariantRelativePath(string rel)
    {
        if (string.IsNullOrEmpty(rel) || !rel.StartsWith("assets/", StringComparison.Ordinal)
            || rel.Contains("..", StringComparison.Ordinal) || rel.Contains('\\') || rel.Contains(':'))
        {
            return false;
        }
        foreach (char c in rel)
        {
            if (!(char.IsAsciiLetterOrDigit(c) || c is '/' or '-' or '_' or '.'))
            {
                return false;
            }
        }
        return true;
    }

    private static bool VerifyCachedOnDisk(ManifestAsset asset, string outputRoot)
    {
        if (asset.Variants.Count == 0)
        {
            return false;
        }
        foreach (ManifestVariant v in asset.Variants.Concat(asset.VideoVariants))
        {
            if (!IsSafeVariantRelativePath(v.RelativePath))
            {
                return false;
            }
            string full = Path.Combine(outputRoot, v.RelativePath.Replace('/', Path.DirectorySeparatorChar));
            try
            {
                FileStagedOutputAdapter.VerifyConfinedNoSymlinks(outputRoot, full);
            }
            catch
            {
                return false;
            }
            var fi = new FileInfo(full);
            if (!fi.Exists || IsSymlink(fi) || fi.Length != v.Bytes)
            {
                return false;
            }
        }
        return true;
    }

    private sealed record ExecuteOutputMeta(int Width, int Height, string? VerifiedMetric, double? VerifiedScore);

    private static Dictionary<string, ExecuteOutputMeta> ParseExecuteOutputs(string resultJson)
    {
        var map = new Dictionary<string, ExecuteOutputMeta>(StringComparer.Ordinal);
        using JsonDocument doc = JsonDocument.Parse(resultJson);
        JsonElement root = doc.RootElement.TryGetProperty("data", out JsonElement d) ? d : doc.RootElement;
        if (!root.TryGetProperty("outputs", out JsonElement outputs) || outputs.ValueKind != JsonValueKind.Array)
        {
            return map;
        }
        foreach (JsonElement item in outputs.EnumerateArray())
        {
            if (!item.TryGetProperty("slot", out JsonElement slotEl))
            {
                continue;
            }
            string? slotName = slotEl.GetString();
            if (string.IsNullOrEmpty(slotName))
            {
                continue;
            }
            int w = item.TryGetProperty("width", out JsonElement wEl) && wEl.ValueKind == JsonValueKind.Number ? wEl.GetInt32() : 0;
            int h = item.TryGetProperty("height", out JsonElement hEl) && hEl.ValueKind == JsonValueKind.Number ? hEl.GetInt32() : 0;
            string? metric = item.TryGetProperty("verified_metric", out JsonElement mEl) && mEl.ValueKind == JsonValueKind.String ? mEl.GetString() : null;
            double? score = item.TryGetProperty("verified_score_f64", out JsonElement sEl) && sEl.ValueKind == JsonValueKind.Number ? sEl.GetDouble() : null;
            map[slotName] = new ExecuteOutputMeta(w, h, metric, score);
        }
        return map;
    }

    internal sealed record DiscoveredMediaFile(
        string Id,
        string RootDirectory,
        string AbsolutePath,
        string RelativeKey,
        bool IsVideoExtension,
        long SizeBytes);

    internal static List<DiscoveredMediaFile> DiscoverMediaFiles(
        IReadOnlyList<string> inputs,
        string? outputRoot,
        bool recursive,
        bool followSymlinks)
    {
        string? normOut = string.IsNullOrEmpty(outputRoot)
            ? null
            : Path.GetFullPath(outputRoot).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar) + Path.DirectorySeparatorChar;
        var seen = new HashSet<string>(StringComparer.Ordinal);
        var raw = new List<(string RelKey, string RootDir, string AbsPath, bool IsVideo, long Size)>();

        foreach (string input in inputs)
        {
            string full = Path.GetFullPath(input);
            if (File.Exists(full))
            {
                var fi = new FileInfo(full);
                if (!followSymlinks && IsSymlink(fi))
                {
                    continue;
                }
                if (TryClassifyExt(full, out bool isVid) && seen.Add(full))
                {
                    raw.Add((fi.Name, fi.DirectoryName ?? ".", full, isVid, fi.Length));
                }
                continue;
            }

            if (!Directory.Exists(full))
            {
                throw new DirectoryNotFoundException($"Input path not found: '{input}'.");
            }
            var rootInfo = new DirectoryInfo(full);
            if (!followSymlinks && IsSymlink(rootInfo))
            {
                continue;
            }

            WalkDirectory(rootInfo, full, normOut, recursive, followSymlinks, seen, raw);
        }

        raw.Sort((a, b) =>
        {
            int cmp = StringComparer.Ordinal.Compare(a.RelKey, b.RelKey);
            return cmp != 0 ? cmp : StringComparer.Ordinal.Compare(a.AbsPath, b.AbsPath);
        });

        var result = new List<DiscoveredMediaFile>(raw.Count);
        for (int i = 0; i < raw.Count; i++)
        {
            (string rel, string rootDir, string abs, bool isVid, long size) = raw[i];
            result.Add(new DiscoveredMediaFile(string.Create(CultureInfo.InvariantCulture, $"a{i + 1:D4}"), rootDir, abs, rel, isVid, size));
        }
        return result;
    }

    private static void WalkDirectory(
        DirectoryInfo current,
        string rootFull,
        string? normOut,
        bool recursive,
        bool followSymlinks,
        HashSet<string> seen,
        List<(string RelKey, string RootDir, string AbsPath, bool IsVideo, long Size)> raw)
    {
        foreach (FileSystemInfo entry in current.EnumerateFileSystemInfos())
        {
            if (entry.Name.StartsWith('.'))
            {
                continue;
            }
            if (!followSymlinks && IsSymlink(entry))
            {
                continue;
            }
            string full = Path.GetFullPath(entry.FullName);
            if (normOut is not null && (full + Path.DirectorySeparatorChar).StartsWith(normOut, StringComparison.Ordinal))
            {
                continue;
            }

            if (entry is DirectoryInfo subDir)
            {
                if (recursive)
                {
                    WalkDirectory(subDir, rootFull, normOut, recursive, followSymlinks, seen, raw);
                }
            }
            else if (entry is FileInfo fi && TryClassifyExt(full, out bool isVid))
            {
                if (seen.Add(full))
                {
                    string relRaw = Path.GetRelativePath(rootFull, full);
                    string rel = OperatingSystem.IsWindows() ? relRaw.Replace('\\', '/') : relRaw;
                    raw.Add((rel, rootFull, full, isVid, fi.Length));
                }
            }
        }
    }

    private static bool IsSymlink(FileSystemInfo fsi) =>
        fsi.LinkTarget is not null || (fsi.Attributes & FileAttributes.ReparsePoint) != 0;

    private static bool TryClassifyExt(string path, out bool isVideo)
    {
        string ext = Path.GetExtension(path);
        if (SupportedImageExts.Contains(ext))
        {
            isVideo = false;
            return true;
        }
        if (SupportedVideoExts.Contains(ext))
        {
            isVideo = true;
            return true;
        }
        isVideo = false;
        return false;
    }

    private static string SanitizeStem(string relKey)
    {
        string stem = Path.GetFileNameWithoutExtension(relKey);
        if (string.IsNullOrEmpty(stem))
        {
            return "asset";
        }
        var sb = new StringBuilder(Math.Min(stem.Length, 64));
        foreach (char c in stem)
        {
            if (sb.Length >= 64)
            {
                break;
            }
            sb.Append(char.IsAsciiLetterOrDigit(c) || c == '-' || c == '_' ? c : '-');
        }
        return sb.Length > 0 ? sb.ToString() : "asset";
    }

    private static string SanitizeAsciiId(string id)
    {
        var sb = new StringBuilder(id.Length);
        foreach (char c in id)
        {
            if (char.IsAsciiLetterOrDigit(c) || c is '-' or '_')
            {
                sb.Append(c);
            }
        }
        return sb.Length > 0 ? sb.ToString() : "asset";
    }

    private static string SanitizeSegment(string seg)
    {
        var sb = new StringBuilder(Math.Min(seg.Length, 64));
        foreach (char c in seg)
        {
            if (sb.Length >= 64)
            {
                break;
            }
            sb.Append(char.IsAsciiLetterOrDigit(c) || c is '-' or '_' ? c : '-');
        }
        string clean = sb.ToString();
        return string.IsNullOrEmpty(clean) || clean is "." or ".." ? "dir" : clean;
    }

    private static string VariantRelativePath(
        OutputPathLayout layout,
        string relKey,
        int width,
        int height,
        string ext,
        string contentHash12)
    {
        string stem = SanitizeStem(relKey);
        switch (layout)
        {
            case OutputPathLayout.ContentAddressed:
                // Immutable content-addressed filename independent of rounded output dimensions (zero post-commit renames).
                return string.Create(CultureInfo.InvariantCulture, $"assets/{stem}-{contentHash12}.{ext}");
            case OutputPathLayout.Flatten:
                return string.Create(CultureInfo.InvariantCulture, $"assets/{stem}-{width}x{height}.{ext}");
            case OutputPathLayout.PreserveTree:
            {
                string? dir = Path.GetDirectoryName(relKey);
                if (string.IsNullOrEmpty(dir))
                {
                    return string.Create(CultureInfo.InvariantCulture, $"assets/{stem}-{width}x{height}.{ext}");
                }
                char[] seps = OperatingSystem.IsWindows() ? new[] { '/', '\\' } : new[] { '/' };
                string[] safeParts = dir
                    .Split(seps, StringSplitOptions.RemoveEmptyEntries)
                    .Where(s => s is not "." and not "..")
                    .Select(SanitizeSegment)
                    .ToArray();
                return safeParts.Length == 0
                    ? string.Create(CultureInfo.InvariantCulture, $"assets/{stem}-{width}x{height}.{ext}")
                    : string.Create(CultureInfo.InvariantCulture, $"assets/{string.Join("/", safeParts)}/{stem}-{width}x{height}.{ext}");
            }
            default:
                throw new ArgumentOutOfRangeException(nameof(layout));
        }
    }

    private static string Sha256Hex(byte[] data) =>
        Convert.ToHexString(SHA256.HashData(data)).ToLowerInvariant();

    private static string WriteJsonObject(Action<Utf8JsonWriter> writeProperties)
    {
        var buffer = new ArrayBufferWriter<byte>(256);
        using (var writer = new Utf8JsonWriter(buffer))
        {
            writer.WriteStartObject();
            writeProperties(writer);
            writer.WriteEndObject();
        }
        return Encoding.UTF8.GetString(buffer.WrittenSpan);
    }
}
