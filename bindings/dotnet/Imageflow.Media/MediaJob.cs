using System;
using System.Buffers;
using System.Collections.Generic;
using System.IO;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Threading.Tasks;

namespace Imageflow.Media;

/// <summary>
/// Input source adapter bound to one numeric <c>io_id</c> on a streaming media job.
///
/// Ownership: callers retain ownership of adapters passed to <see cref="MediaJob"/> and
/// are responsible for disposing them (e.g., via <c>await using</c>) after the returned
/// <see cref="Task{TResult}"/> completes.
/// </summary>
public interface IMediaInputAdapter : IAsyncDisposable
{
    bool SupportsRange { get; }
    bool HasKnownLength { get; }
    ValueTask<long?> GetLengthAsync(CancellationToken ct);
    ValueTask<int> ReadForwardAsync(Memory<byte> dst, CancellationToken ct);
    ValueTask<int> ReadAtAsync(long offset, Memory<byte> dst, CancellationToken ct);
}

/// <summary>
/// Output sink adapter bound to one numeric <c>io_id</c> on a streaming media job.
///
/// Ownership: callers retain ownership of adapters passed to <see cref="MediaJob"/> and
/// are responsible for disposing them after the returned <see cref="Task{TResult}"/> completes.
/// </summary>
public interface IMediaOutputAdapter : IAsyncDisposable
{
    bool SupportsPatch { get; }
    bool IsIncremental { get; }
    ValueTask WriteAppendAsync(ReadOnlyMemory<byte> chunk, CancellationToken ct);
    ValueTask WritePatchAsync(long offset, ReadOnlyMemory<byte> chunk, CancellationToken ct);
    ValueTask FlushAsync(CancellationToken ct);
    ValueTask CommitAsync();
    ValueTask RollbackAsync();
}

/// <summary>
/// Whole-job atomic publication transaction required when a streaming job declares two or
/// more staged (<c>IsIncremental == false</c>) output sinks.
/// </summary>
public interface IStagedCommitTransaction
{
    /// <summary>
    /// Atomically publish all staged outputs for the job (called only after native
    /// <c>IFX_EVENT_COMMIT</c> authorization).
    /// </summary>
    ValueTask CommitAllAsync();

    /// <summary>
    /// Roll back all staged outputs if the job fails, cancels, or throws before commit completes.
    /// </summary>
    ValueTask RollbackAllAsync();
}

/// <summary>
/// Completed terminal payload from <see cref="MediaJob"/> (<c>OutputV2</c>, inspect, or estimate JSON).
/// </summary>
public sealed record MediaResult(string ResultJson, bool Succeeded);

/// <summary>Lifecycle status of a tracked media job's native progress observer.</summary>
public enum MediaProgressStatus : uint
{
    Disabled = 0,
    NotStarted = 1,
    Running = 2,
    Succeeded = 3,
    Failed = 4,
    Cancelled = 5,
    Abandoned = 6,
}

/// <summary>
/// Allocation-free point-in-time native progress snapshot read from <c>how-far-along</c> via
/// <c>imageflow_stream_job_progress_v1</c>.
/// </summary>
public readonly record struct MediaProgress(
    MediaProgressStatus Status,
    double? Fraction,
    double UnresolvedFraction,
    bool IsTerminal,
    ulong Generation);

/// <summary>
/// Pull-only, zero-callback progress monitor for <see cref="MediaJob"/>.
///
/// Passing a <see cref="MediaProgressMonitor"/> without an <see cref="IProgress{T}"/> enables
/// lock-free atomic counters in native <c>how-far-along</c> with ZERO progress callbacks or wake
/// notifications; UI/caller timers can poll <see cref="TryRead"/> or <see cref="TryReadSnapshotJson"/>
/// at any cadence without blocking native execution.
/// </summary>
public sealed class MediaProgressMonitor
{
    private readonly object _lock = new();
    private JobHandle? _job;
    private MediaProgress _finalSummary = new(MediaProgressStatus.NotStarted, null, 1.0, false, 0);
    private bool _hasFinalSummary;
    private string? _finalJson;
    private bool _detached;

    internal void Attach(JobHandle job)
    {
        lock (_lock)
        {
            if (_job is not null)
            {
                throw new InvalidOperationException(
                    "This MediaProgressMonitor is already attached to an active MediaJob. Create a separate MediaProgressMonitor instance for each concurrent job.");
            }
            _job = job;
            _detached = false;
            _finalSummary = new(MediaProgressStatus.NotStarted, null, 1.0, false, 0);
            _hasFinalSummary = false;
            _finalJson = null;
        }
    }

    internal void DetachAndSnapshot(JobHandle expectedJob, bool snapshotNative)
    {
        JobHandle? job;
        lock (_lock)
        {
            if (!ReferenceEquals(_job, expectedJob) || _detached)
            {
                return;
            }
            job = _job;
        }
        try
        {
            if (snapshotNative && job is not null)
            {
                // Snapshot final terminal state strictly BEFORE JobHandle is disposed so
                // post-completion reads still observe the frozen terminal summary and phase tree!
                try
                {
                    if (TryReadFromHandle(job, out MediaProgress finalP))
                    {
                        lock (_lock)
                        {
                            if (ReferenceEquals(_job, expectedJob))
                            {
                                _finalSummary = finalP;
                                _hasFinalSummary = true;
                            }
                        }
                    }
                    if (TryReadJsonFromHandle(job, out string? finalJ))
                    {
                        lock (_lock)
                        {
                            if (ReferenceEquals(_job, expectedJob))
                            {
                                _finalJson = finalJ;
                            }
                        }
                    }
                }
                catch (EntryPointNotFoundException)
                {
                    // Older native library without progress exports; detach cleanly without throwing.
                }
            }
        }
        finally
        {
            lock (_lock)
            {
                if (ReferenceEquals(_job, expectedJob))
                {
                    _job = null;
                    _detached = true;
                }
            }
        }
    }

    /// <summary>
    /// Non-blocking, allocation-free read of the current progress summary. Safe to call from any
    /// thread while the job is running or after it finishes.
    /// </summary>
    public bool TryRead(out MediaProgress progress)
    {
        JobHandle? job;
        lock (_lock)
        {
            if (_detached)
            {
                if (_hasFinalSummary)
                {
                    progress = _finalSummary;
                    return true;
                }
                progress = new MediaProgress(MediaProgressStatus.NotStarted, null, 1.0, false, 0);
                return false;
            }
            job = _job;
        }
        if (job is null)
        {
            progress = new MediaProgress(MediaProgressStatus.NotStarted, null, 1.0, false, 0);
            return false;
        }
        return TryReadFromHandle(job, out progress);
    }

    /// <summary>
    /// Non-blocking export of <c>how-far-along</c>'s versioned (<c>schema_version: 2</c>)
    /// hierarchical phase-tree snapshot JSON (stages: <c>input</c>, <c>process</c>, <c>output</c>
    /// plus codec sub-phases).
    /// </summary>
    public bool TryReadSnapshotJson(out string? snapshotJson)
    {
        JobHandle? job;
        lock (_lock)
        {
            if (_detached)
            {
                snapshotJson = _finalJson;
                return snapshotJson is not null;
            }
            job = _job;
        }
        if (job is null)
        {
            snapshotJson = null;
            return false;
        }
        return TryReadJsonFromHandle(job, out snapshotJson);
    }

    internal static unsafe bool TryReadFromHandle(JobHandle job, out MediaProgress progress)
    {
        bool addedRef = false;
        try
        {
            if (job.IsClosed || job.IsInvalid)
            {
                progress = default;
                return false;
            }
            job.DangerousAddRef(ref addedRef);
            var raw = new ProgressV1 { Size = (uint)sizeof(ProgressV1) };
            int rc = Native.Progress(job, &raw);
            if (rc != WireConstants.Ok)
            {
                progress = default;
                return false;
            }
            bool hasFrac = (raw.Flags & WireConstants.ProgressFlagHasFraction) != 0;
            bool isTerm = (raw.Flags & WireConstants.ProgressFlagTerminal) != 0;
            progress = new MediaProgress(
                (MediaProgressStatus)raw.Status,
                hasFrac ? raw.Fraction : null,
                raw.UnresolvedFraction,
                isTerm,
                raw.Generation.Value);
            return true;
        }
        catch (ObjectDisposedException)
        {
            progress = default;
            return false;
        }
        finally
        {
            if (addedRef)
            {
                job.DangerousRelease();
            }
        }
    }

    internal static unsafe bool TryReadJsonFromHandle(JobHandle job, out string? json)
    {
        bool addedRef = false;
        try
        {
            if (job.IsClosed || job.IsInvalid)
            {
                json = null;
                return false;
            }
            job.DangerousAddRef(ref addedRef);
            nuint needed = 0;
            int rc = Native.ProgressJson(job, null, 0, &needed);
            if (rc == WireConstants.Ok && needed == 0)
            {
                json = string.Empty;
                return true;
            }
            if (rc != WireConstants.BufferTooSmall || needed == 0)
            {
                json = null;
                return false;
            }
            byte[] buf = new byte[checked((int)needed)];
            fixed (byte* p = buf)
            {
                nuint actual = 0;
                rc = Native.ProgressJson(job, p, (nuint)buf.Length, &actual);
                if (rc != WireConstants.Ok)
                {
                    json = null;
                    return false;
                }
                json = Encoding.UTF8.GetString(buf, 0, checked((int)actual));
                return true;
            }
        }
        catch (ObjectDisposedException)
        {
            json = null;
            return false;
        }
        finally
        {
            if (addedRef)
            {
                job.DangerousRelease();
            }
        }
    }
}

/// <summary>
/// I/O exception raised when a managed input/output adapter fails during streaming, preserving
/// BOTH the original adapter exception (<see cref="Exception.InnerException"/>) AND the native
/// terminal publication receipt (<see cref="TerminalReceiptJson"/>, carrying exact
/// <c>accepted_bytes</c>, <c>exact_unknown_tail</c>, and <c>committed</c> per sink).
/// </summary>
public sealed class MediaIoException : IOException
{
    public string? TerminalReceiptJson { get; }

    public MediaIoException(string message, string? terminalReceiptJson, Exception innerException)
        : base(message, innerException)
    {
        TerminalReceiptJson = terminalReceiptJson;
    }
}

/// <summary>
/// Opt-in progress observation configuration for <see cref="MediaJob.ExecuteWithProgressAsync"/>,
/// <see cref="MediaJob.InspectWithProgressAsync"/>, and <see cref="MediaJob.EstimateWithProgressAsync"/>.
/// Callers that use the standard <c>ExecuteAsync</c>/<c>InspectAsync</c>/<c>EstimateAsync</c>
/// methods incur zero progress allocation or callback overhead.
/// </summary>
public sealed class MediaProgressOptions
{
    public IProgress<MediaProgress>? Progress { get; }
    public MediaProgressMonitor? Monitor { get; }

    private MediaProgressOptions(IProgress<MediaProgress>? progress, MediaProgressMonitor? monitor)
    {
        Progress = progress;
        Monitor = monitor;
    }

    /// <summary>
    /// Enable lock-free atomic native tracking with ZERO progress callbacks or wake notifications.
    /// Poll <paramref name="monitor"/> at any cadence you choose.
    /// </summary>
    public static MediaProgressOptions PullOnly(MediaProgressMonitor monitor) =>
        new(progress: null, monitor: monitor ?? throw new ArgumentNullException(nameof(monitor)));

    /// <summary>
    /// Enable paced, coalesced <see cref="IProgress{T}"/> delivery (with an optional
    /// <see cref="MediaProgressMonitor"/> for phase-tree JSON inspection).
    /// </summary>
    public static MediaProgressOptions PacedNotify(
        IProgress<MediaProgress> progress,
        MediaProgressMonitor? monitor = null) =>
        new(progress: progress ?? throw new ArgumentNullException(nameof(progress)), monitor: monitor);
}

/// <summary>
/// High-level hardened async driver over Imageflow streaming ABI v1 (<c>imageflow_stream_v1.h</c>).
///
/// Key lifetime, atomicity &amp; concurrency invariants:
/// 1. Every P/Invoke uses <see cref="JobHandle"/> (<see cref="SafeHandle"/>) so concurrent cancellation
///    cannot race handle release. Prefer the <see cref="SafeHandle"/> context overloads so the native
///    context cannot be finalized during <c>imageflow_stream_job_create_v1</c>.
/// 2. Zero pointers escape <c>fixed</c> scopes; bytes are copied synchronously at the boundary.
/// 3. <c>GCHandle</c> rooting <c>Pump</c> has a single owner: freed ONLY on start rejection or inside
///    the guaranteed once-only <c>ReleaseUser</c> native quiescence callback.
/// 4. Zero 50 ms polling: native <c>Notify(IFX_NOTIFY_STOP)</c> trips <c>ioStop</c> immediately via
///    <see cref="TaskCreationOptions.RunContinuationsAsynchronously"/>.
/// 5. Whole-job staged atomicity: jobs with 2+ staged sinks MUST supply an
///    <see cref="IStagedCommitTransaction"/>; native <c>EventFailed</c> ALWAYS rolls back staged
///    sinks before returning; every <c>Native.Reply</c> status is validated so rejected tickets can
///    never be serviced twice.
/// 6. Adapter ownership: <see cref="MediaJob"/> never disposes caller-provided adapters; dispose
///    them after awaiting the returned <see cref="Task{MediaResult}"/>.
/// </summary>
public static class MediaJob
{
    /// <summary>
    /// Execute a streaming pipeline using a <see cref="SafeHandle"/>-protected native context lease.
    /// Zero progress overhead (stop token only).
    /// </summary>
    public static Task<MediaResult> ExecuteAsync(
        SafeHandle context,
        string envelopeJson,
        IReadOnlyDictionary<int, IMediaInputAdapter> sources,
        IReadOnlyDictionary<int, IMediaOutputAdapter> destinations,
        IStagedCommitTransaction? commitTransaction = null,
        CancellationToken cancellationToken = default) =>
        RunWithSafeContextAsync(
            context,
            WireConstants.OpExecuteV1,
            envelopeJson,
            sources,
            destinations,
            commitTransaction,
            cancellationToken,
            progress: null,
            progressMonitor: null);

    /// <summary>
    /// Execute a streaming pipeline with opt-in <c>how-far-along</c> progress tracking
    /// (<see cref="MediaProgressOptions.PullOnly"/> for zero-callback polling, or
    /// <see cref="MediaProgressOptions.PacedNotify"/> for coalesced <see cref="IProgress{T}"/>).
    /// </summary>
    public static Task<MediaResult> ExecuteWithProgressAsync(
        SafeHandle context,
        string envelopeJson,
        IReadOnlyDictionary<int, IMediaInputAdapter> sources,
        IReadOnlyDictionary<int, IMediaOutputAdapter> destinations,
        MediaProgressOptions progressOptions,
        IStagedCommitTransaction? commitTransaction = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(progressOptions);
        return RunWithSafeContextAsync(
            context,
            WireConstants.OpExecuteV1,
            envelopeJson,
            sources,
            destinations,
            commitTransaction,
            cancellationToken,
            progressOptions.Progress,
            progressOptions.Monitor);
    }

    /// <summary>
    /// Execute a streaming pipeline using a raw native context pointer.
    /// Caller MUST guarantee <paramref name="context"/> remains valid until job creation completes.
    /// </summary>
    public static Task<MediaResult> ExecuteAsync(
        nint context,
        string envelopeJson,
        IReadOnlyDictionary<int, IMediaInputAdapter> sources,
        IReadOnlyDictionary<int, IMediaOutputAdapter> destinations,
        IStagedCommitTransaction? commitTransaction = null,
        CancellationToken cancellationToken = default) =>
        RunOperationAsync(
            context,
            WireConstants.OpExecuteV1,
            envelopeJson,
            sources,
            destinations,
            commitTransaction,
            cancellationToken,
            progress: null,
            progressMonitor: null);

    /// <summary>
    /// Execute a streaming pipeline using a raw native context pointer with opt-in progress tracking.
    /// </summary>
    public static Task<MediaResult> ExecuteWithProgressAsync(
        nint context,
        string envelopeJson,
        IReadOnlyDictionary<int, IMediaInputAdapter> sources,
        IReadOnlyDictionary<int, IMediaOutputAdapter> destinations,
        MediaProgressOptions progressOptions,
        IStagedCommitTransaction? commitTransaction = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(progressOptions);
        return RunOperationAsync(
            context,
            WireConstants.OpExecuteV1,
            envelopeJson,
            sources,
            destinations,
            commitTransaction,
            cancellationToken,
            progressOptions.Progress,
            progressOptions.Monitor);
    }

    /// <summary>
    /// Inspect media container headers/index using a <see cref="SafeHandle"/>-protected native context.
    /// </summary>
    public static Task<MediaResult> InspectAsync(
        SafeHandle context,
        string envelopeJson,
        IReadOnlyDictionary<int, IMediaInputAdapter> sources,
        CancellationToken cancellationToken = default) =>
        RunWithSafeContextAsync(
            context,
            WireConstants.OpInspectV1,
            envelopeJson,
            sources,
            new Dictionary<int, IMediaOutputAdapter>(),
            null,
            cancellationToken,
            progress: null,
            progressMonitor: null);

    /// <summary>
    /// Inspect media container headers/index with opt-in progress tracking.
    /// </summary>
    public static Task<MediaResult> InspectWithProgressAsync(
        SafeHandle context,
        string envelopeJson,
        IReadOnlyDictionary<int, IMediaInputAdapter> sources,
        MediaProgressOptions progressOptions,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(progressOptions);
        return RunWithSafeContextAsync(
            context,
            WireConstants.OpInspectV1,
            envelopeJson,
            sources,
            new Dictionary<int, IMediaOutputAdapter>(),
            null,
            cancellationToken,
            progressOptions.Progress,
            progressOptions.Monitor);
    }

    /// <summary>
    /// Inspect media container headers/index using a raw native context pointer.
    /// Caller MUST guarantee <paramref name="context"/> remains valid until job creation completes.
    /// </summary>
    public static Task<MediaResult> InspectAsync(
        nint context,
        string envelopeJson,
        IReadOnlyDictionary<int, IMediaInputAdapter> sources,
        CancellationToken cancellationToken = default) =>
        RunOperationAsync(
            context,
            WireConstants.OpInspectV1,
            envelopeJson,
            sources,
            new Dictionary<int, IMediaOutputAdapter>(),
            null,
            cancellationToken,
            progress: null,
            progressMonitor: null);

    /// <summary>
    /// Inspect media container headers/index using a raw native context pointer with opt-in progress tracking.
    /// </summary>
    public static Task<MediaResult> InspectWithProgressAsync(
        nint context,
        string envelopeJson,
        IReadOnlyDictionary<int, IMediaInputAdapter> sources,
        MediaProgressOptions progressOptions,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(progressOptions);
        return RunOperationAsync(
            context,
            WireConstants.OpInspectV1,
            envelopeJson,
            sources,
            new Dictionary<int, IMediaOutputAdapter>(),
            null,
            cancellationToken,
            progressOptions.Progress,
            progressOptions.Monitor);
    }

    /// <summary>
    /// Estimate pipeline admission and startup dependencies using a <see cref="SafeHandle"/>-protected context.
    /// </summary>
    public static Task<MediaResult> EstimateAsync(
        SafeHandle context,
        string envelopeJson,
        IReadOnlyDictionary<int, IMediaInputAdapter> sources,
        IReadOnlyDictionary<int, IMediaOutputAdapter> destinations,
        CancellationToken cancellationToken = default) =>
        RunWithSafeContextAsync(
            context,
            WireConstants.OpEstimateV1,
            envelopeJson,
            sources,
            destinations,
            null,
            cancellationToken,
            progress: null,
            progressMonitor: null);

    /// <summary>
    /// Estimate pipeline admission and startup dependencies with opt-in progress tracking.
    /// </summary>
    public static Task<MediaResult> EstimateWithProgressAsync(
        SafeHandle context,
        string envelopeJson,
        IReadOnlyDictionary<int, IMediaInputAdapter> sources,
        IReadOnlyDictionary<int, IMediaOutputAdapter> destinations,
        MediaProgressOptions progressOptions,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(progressOptions);
        return RunWithSafeContextAsync(
            context,
            WireConstants.OpEstimateV1,
            envelopeJson,
            sources,
            destinations,
            null,
            cancellationToken,
            progressOptions.Progress,
            progressOptions.Monitor);
    }

    /// <summary>
    /// Estimate pipeline admission and startup dependencies using a raw native context pointer.
    /// Caller MUST guarantee <paramref name="context"/> remains valid until job creation completes.
    /// </summary>
    public static Task<MediaResult> EstimateAsync(
        nint context,
        string envelopeJson,
        IReadOnlyDictionary<int, IMediaInputAdapter> sources,
        IReadOnlyDictionary<int, IMediaOutputAdapter> destinations,
        CancellationToken cancellationToken = default) =>
        RunOperationAsync(
            context,
            WireConstants.OpEstimateV1,
            envelopeJson,
            sources,
            destinations,
            null,
            cancellationToken,
            progress: null,
            progressMonitor: null);

    /// <summary>
    /// Estimate pipeline admission and startup dependencies using a raw native context pointer with opt-in progress tracking.
    /// </summary>
    public static Task<MediaResult> EstimateWithProgressAsync(
        nint context,
        string envelopeJson,
        IReadOnlyDictionary<int, IMediaInputAdapter> sources,
        IReadOnlyDictionary<int, IMediaOutputAdapter> destinations,
        MediaProgressOptions progressOptions,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(progressOptions);
        return RunOperationAsync(
            context,
            WireConstants.OpEstimateV1,
            envelopeJson,
            sources,
            destinations,
            null,
            cancellationToken,
            progressOptions.Progress,
            progressOptions.Monitor);
    }

    private static Task<MediaResult> RunWithSafeContextAsync(
        SafeHandle context,
        uint operation,
        string envelopeJson,
        IReadOnlyDictionary<int, IMediaInputAdapter> sources,
        IReadOnlyDictionary<int, IMediaOutputAdapter> destinations,
        IStagedCommitTransaction? commitTransaction,
        CancellationToken caller,
        IProgress<MediaProgress>? progress,
        MediaProgressMonitor? progressMonitor)
    {
        ArgumentNullException.ThrowIfNull(context);
        WireConstants.AssertLayouts();
        caller.ThrowIfCancellationRequested();
        ValidateStagedTransactionRequirement(operation, destinations, commitTransaction);

        JobHandle job;
        bool addedRef = false;
        try
        {
            context.DangerousAddRef(ref addedRef);
            job = CreateJob(context.DangerousGetHandle(), operation, envelopeJson, sources, destinations);
        }
        finally
        {
            if (addedRef)
            {
                context.DangerousRelease();
            }
        }
        return RunCreatedJobAsync(job, sources, destinations, commitTransaction, caller, progress, progressMonitor);
    }

    private static Task<MediaResult> RunOperationAsync(
        nint context,
        uint operation,
        string envelopeJson,
        IReadOnlyDictionary<int, IMediaInputAdapter> sources,
        IReadOnlyDictionary<int, IMediaOutputAdapter> destinations,
        IStagedCommitTransaction? commitTransaction,
        CancellationToken caller,
        IProgress<MediaProgress>? progress,
        MediaProgressMonitor? progressMonitor)
    {
        if (context == nint.Zero)
        {
            throw new ArgumentException("Native context handle must be non-zero.", nameof(context));
        }
        WireConstants.AssertLayouts();
        caller.ThrowIfCancellationRequested();
        ValidateStagedTransactionRequirement(operation, destinations, commitTransaction);

        JobHandle job = CreateJob(context, operation, envelopeJson, sources, destinations);
        return RunCreatedJobAsync(job, sources, destinations, commitTransaction, caller, progress, progressMonitor);
    }

    private static void ValidateStagedTransactionRequirement(
        uint operation,
        IReadOnlyDictionary<int, IMediaOutputAdapter> destinations,
        IStagedCommitTransaction? commitTransaction)
    {
        if (operation != WireConstants.OpExecuteV1) return;
        int stagedCount = 0;
        foreach (var sink in destinations.Values)
        {
            if (!sink.IsIncremental) stagedCount++;
        }
        if (stagedCount > 1 && commitTransaction is null)
        {
            throw new ArgumentException(
                $"Job declares {stagedCount} staged output sinks; an explicit {nameof(IStagedCommitTransaction)} is required to guarantee whole-job atomic commit.",
                nameof(commitTransaction));
        }
    }

    private static async Task<MediaResult> RunCreatedJobAsync(
        JobHandle createdJob,
        IReadOnlyDictionary<int, IMediaInputAdapter> sources,
        IReadOnlyDictionary<int, IMediaOutputAdapter> destinations,
        IStagedCommitTransaction? commitTransaction,
        CancellationToken caller,
        IProgress<MediaProgress>? progress,
        MediaProgressMonitor? progressMonitor)
    {
        var job = createdJob;
        var pump = new Pump(job, sources, destinations, commitTransaction, progress);

        uint hookFlags = 0;
        if (progress is not null)
        {
            hookFlags |= WireConstants.HookProgressNotify;
        }
        else if (progressMonitor is not null)
        {
            hookFlags |= WireConstants.HookProgressPull;
        }

        using var ioStop = CancellationTokenSource.CreateLinkedTokenSource(caller);
        CancellationTokenRegistration registration = caller.UnsafeRegister(
            static state => { _ = Native.Cancel((JobHandle)state!); },
            job);

        Task? watchStop = null;
        bool started = false;
        bool monitorAttached = false;
        try
        {
            // Claim exclusive monitor ownership BEFORE starting native work so overlapping reuse
            // fails immediately without touching the already-running job's monitor attachment!
            if (progressMonitor is not null)
            {
                progressMonitor.Attach(job);
                monitorAttached = true;
            }
            int accepted = StartAsyncNative(pump, hookFlags);
            if (accepted != WireConstants.Ok)
            {
                throw new InvalidOperationException($"imageflow_stream_job_start_async_v1 failed with code {accepted}");
            }
            started = true;
            watchStop = pump.ObserveStopAsync(ioStop);
            return await pump.DriveAsync(ioStop.Token).ConfigureAwait(false);
        }
        catch (MediaIoException)
        {
            // Ticket was resolved, native terminal receipt was drained, and staged sinks were rolled back.
            throw;
        }
        catch
        {
            if (started)
            {
                _ = Native.Cancel(job);
            }
            await pump.RollbackStagedIfUncommittedAsync().ConfigureAwait(false);
            throw;
        }
        finally
        {
            // Reverse-order cleanup with guaranteed handle disposal even if progress exports are absent:
            // 1. Detach monitor (and snapshot ONLY if native start actually succeeded!).
            // 2. Flush final progress (only if native start succeeded).
            // 3. Unregister caller cancel hook BEFORE disposing JobHandle.
            // 4. Dispose JobHandle BEFORE awaiting NativeReleased so abandoned Commit quiesces.
            try
            {
                if (monitorAttached)
                {
                    progressMonitor!.DetachAndSnapshot(job, snapshotNative: started);
                }
            }
            catch
            {
                // Never skip cancel unregister / JobHandle dispose on observer cleanup error.
            }
            if (started)
            {
                pump.FlushFinalProgressNoThrow();
            }
            try
            {
                registration.Dispose();
            }
            finally
            {
                job.Dispose();
            }
            if (started)
            {
                await pump.NativeReleased.Task.ConfigureAwait(false);
                if (watchStop is not null)
                {
                    await watchStop.ConfigureAwait(false);
                }
            }
        }
    }

    private static unsafe JobHandle CreateJob(
        nint context,
        uint operation,
        string envelopeJson,
        IReadOnlyDictionary<int, IMediaInputAdapter> sources,
        IReadOnlyDictionary<int, IMediaOutputAdapter> destinations)
    {
        byte[] jsonBytes = Encoding.UTF8.GetBytes(envelopeJson);
        var bindings = new BindingV1[sources.Count + destinations.Count];
        int i = 0;
        foreach (var kv in sources)
        {
            uint flags = WireConstants.BindSource;
            if (kv.Value.SupportsRange) flags |= WireConstants.BindSourceRange;
            if (kv.Value.HasKnownLength) flags |= WireConstants.BindSourceKnownLen;
            bindings[i++] = new BindingV1
            {
                Size = (uint)sizeof(BindingV1),
                IoId = kv.Key,
                Flags = flags,
                Reserved = 0,
            };
        }
        foreach (var kv in destinations)
        {
            uint flags = WireConstants.BindSink;
            if (kv.Value.SupportsPatch) flags |= WireConstants.BindSinkPatch;
            if (kv.Value.IsIncremental) flags |= WireConstants.BindSinkIncremental;
            bindings[i++] = new BindingV1
            {
                Size = (uint)sizeof(BindingV1),
                IoId = kv.Key,
                Flags = flags,
                Reserved = 0,
            };
        }

        fixed (byte* pJson = jsonBytes)
        fixed (BindingV1* pBind = bindings)
        {
            int rc = Native.Create(
                context,
                operation,
                pJson,
                (nuint)jsonBytes.Length,
                bindings.Length == 0 ? null : pBind,
                (nuint)bindings.Length,
                out nint rawJob);
            if (rc != WireConstants.Ok || rawJob == nint.Zero)
            {
                throw new InvalidOperationException($"imageflow_stream_job_create_v1 failed with code {rc}");
            }
            return new JobHandle(rawJob);
        }
    }

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    private static void OnNotify(nint cookie, uint flags)
    {
        Pump? pump = null;
        try
        {
            pump = (Pump)GCHandle.FromIntPtr(cookie).Target!;
            pump.Enqueue(flags);
        }
        catch
        {
            pump?.SignalInteropFailureNoThrow();
        }
    }

    [UnmanagedCallersOnly(CallConvs = new[] { typeof(CallConvCdecl) })]
    private static void OnReleased(nint cookie)
    {
        try
        {
            var root = GCHandle.FromIntPtr(cookie);
            var pump = (Pump)root.Target!;
            root.Free();
            pump.NativeReleased.TrySetResult(true);
        }
        catch
        {
            // Single-release contract guarantees cookie validity; never throw across FFI.
        }
    }

    private static unsafe int StartAsyncNative(Pump pump, uint hookFlags)
    {
        var root = GCHandle.Alloc(pump);
        var hooks = new HooksV1
        {
            Size = (uint)sizeof(HooksV1),
            Flags = hookFlags,
            User = GCHandle.ToIntPtr(root),
            Notify = &OnNotify,
            ReleaseUser = &OnReleased,
        };
        bool accepted = false;
        try
        {
            int rc = Native.StartAsync(pump.Job, &hooks);
            accepted = rc == WireConstants.Ok;
            return rc;
        }
        finally
        {
            if (!accepted) root.Free();
        }
    }

    private sealed class Pump
    {
        private readonly IReadOnlyDictionary<int, IMediaInputAdapter> _sources;
        private readonly IReadOnlyDictionary<int, IMediaOutputAdapter> _sinks;
        private readonly IStagedCommitTransaction? _commitTransaction;
        private readonly IProgress<MediaProgress>? _progress;
        private readonly object _sigLock = new();
        private bool _eventPending;
        private bool _progressPending;
        private TaskCompletionSource<bool>? _waiter;
        private bool _committed;
        private bool _rolledBack;
        private bool _hasLastProgress;
        private MediaProgress _lastProgress;

        internal JobHandle Job { get; }
        internal TaskCompletionSource<bool> NativeReleased { get; } =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        internal TaskCompletionSource<bool> StopLatched { get; } =
            new(TaskCreationOptions.RunContinuationsAsynchronously);

        internal Pump(
            JobHandle job,
            IReadOnlyDictionary<int, IMediaInputAdapter> sources,
            IReadOnlyDictionary<int, IMediaOutputAdapter> sinks,
            IStagedCommitTransaction? commitTransaction,
            IProgress<MediaProgress>? progress)
        {
            Job = job;
            _sources = sources;
            _sinks = sinks;
            _commitTransaction = commitTransaction;
            _progress = progress;
        }

        internal void Enqueue(uint flags)
        {
            if ((flags & WireConstants.NotifyStop) != 0)
            {
                StopLatched.TrySetResult(true);
            }
            bool hasEventOrStop = (flags & (WireConstants.NotifyEvent | WireConstants.NotifyStop)) != 0;
            bool hasProgress = (flags & WireConstants.NotifyProgress) != 0 && _progress is not null;
            if (hasEventOrStop || hasProgress)
            {
                TaskCompletionSource<bool>? toWake = null;
                lock (_sigLock)
                {
                    if (hasProgress)
                    {
                        _progressPending = true;
                    }
                    if (_waiter is not null)
                    {
                        toWake = _waiter;
                        _waiter = null;
                    }
                    else
                    {
                        _eventPending = true;
                    }
                }
                toWake?.TrySetResult(true);
            }
        }

        internal void FlushFinalProgressNoThrow()
        {
            if (_progress is null) return;
            try
            {
                ReportProgressIfChanged(force: true);
            }
            catch
            {
                // Never mask terminal result or cleanup with a user IProgress callback exception.
            }
        }

        private void ReportProgressIfChanged(bool force)
        {
            if (_progress is null) return;
            bool shouldRead = force;
            if (!shouldRead)
            {
                lock (_sigLock)
                {
                    if (_progressPending)
                    {
                        _progressPending = false;
                        shouldRead = true;
                    }
                }
            }
            if (!shouldRead) return;
            if (!MediaProgressMonitor.TryReadFromHandle(Job, out MediaProgress cur))
            {
                return;
            }
            if (!_hasLastProgress || cur != _lastProgress)
            {
                _hasLastProgress = true;
                _lastProgress = cur;
                try
                {
                    _progress.Report(cur);
                }
                catch
                {
                    // Isolate observer exceptions so a throwing IProgress<MediaProgress> handler
                    // can never abort DriveAsync or turn a committed result into a faulted task.
                }
            }
        }

        internal void SignalInteropFailureNoThrow()
        {
            StopLatched.TrySetResult(true);
            Enqueue(WireConstants.NotifyEvent);
        }

        internal async Task ObserveStopAsync(CancellationTokenSource ioStop)
        {
            Task finished = await Task.WhenAny(StopLatched.Task, NativeReleased.Task).ConfigureAwait(false);
            if (ReferenceEquals(finished, StopLatched.Task))
            {
                try { ioStop.Cancel(); } catch (ObjectDisposedException) { }
            }
        }

        private Task WaitForSignalAsync()
        {
            lock (_sigLock)
            {
                if (_eventPending)
                {
                    _eventPending = false;
                    return Task.CompletedTask;
                }
                _waiter ??= new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
                return _waiter.Task;
            }
        }

        internal async Task<MediaResult> DriveAsync(CancellationToken ioCt)
        {
            while (true)
            {
                ReportProgressIfChanged(force: false);
                int rc = PollNext(out EventV1 ev);
                if (rc == WireConstants.WouldBlock)
                {
                    await WaitForSignalAsync().ConfigureAwait(false);
                    continue;
                }
                if (rc != WireConstants.Ok)
                {
                    throw new InvalidOperationException($"imageflow_stream_job_next_v1 failed with code {rc}");
                }

                switch (ev.Kind)
                {
                    case WireConstants.EventComplete:
                        ReportProgressIfChanged(force: true);
                        return new MediaResult(ReadResultJson(), Succeeded: true);

                    case WireConstants.EventFailed:
                        await RollbackStagedIfUncommittedAsync().ConfigureAwait(false);
                        ReportProgressIfChanged(force: true);
                        return new MediaResult(ReadResultJson(), Succeeded: false);

                    case WireConstants.EventNeedAtLeast:
                        await HandleNeedAtLeastAsync(ev, ioCt).ConfigureAwait(false);
                        break;

                    case WireConstants.EventNeedRange:
                        await HandleNeedRangeAsync(ev, ioCt).ConfigureAwait(false);
                        break;

                    case WireConstants.EventNeedLen:
                        await HandleNeedLenAsync(ev, ioCt).ConfigureAwait(false);
                        break;

                    case WireConstants.EventOutput:
                        await HandleOutputAsync(ev, isPatch: false, ioCt).ConfigureAwait(false);
                        break;

                    case WireConstants.EventPatch:
                        await HandleOutputAsync(ev, isPatch: true, ioCt).ConfigureAwait(false);
                        break;

                    case WireConstants.EventFlush:
                        await HandleFlushAsync(ev, ioCt).ConfigureAwait(false);
                        break;

                    case WireConstants.EventCommit:
                        await HandleCommitAsync(ev).ConfigureAwait(false);
                        break;

                    case WireConstants.EventYield:
                        break;

                    default:
                        throw new InvalidOperationException($"unexpected streaming event kind {ev.Kind}");
                }
            }
        }

        internal async Task RollbackStagedIfUncommittedAsync()
        {
            if (_committed || _rolledBack) return;
            _rolledBack = true;

            if (_commitTransaction is not null)
            {
                try { await _commitTransaction.RollbackAllAsync().ConfigureAwait(false); } catch { }
                return;
            }
            foreach (var sink in _sinks.Values)
            {
                if (!sink.IsIncremental)
                {
                    try { await sink.RollbackAsync().ConfigureAwait(false); } catch { }
                }
            }
        }

        private unsafe int PollNext(out EventV1 ev)
        {
            ev = new EventV1 { Size = (uint)sizeof(EventV1) };
            fixed (EventV1* p = &ev)
            {
                return Native.Next(Job, 0, p);
            }
        }

        private async Task HandleNeedAtLeastAsync(EventV1 ev, CancellationToken ct)
        {
            var src = _sources[ev.IoId];
            int cap = checked((int)Math.Min(ev.TransferMax, 256u * 1024u));
            byte[] rented = ArrayPool<byte>.Shared.Rent(cap);
            try
            {
                // Honor absolute offset whenever the source supports seeking/ReadAt!
                int read = src.SupportsRange
                    ? await src.ReadAtAsync(checked((long)ev.Offset.Value), rented.AsMemory(0, cap), ct)
                        .ConfigureAwait(false)
                    : await src.ReadForwardAsync(rented.AsMemory(0, cap), ct).ConfigureAwait(false);
                if (read == 0)
                {
                    CheckReplyStatus(SendEnd(ev.Ticket, null), "NeedAtLeast End");
                }
                else
                {
                    CheckReplyStatus(SendBytes(ev.Ticket, rented.AsSpan(0, read)), "NeedAtLeast Bytes");
                }
            }
            catch (Exception ex)
            {
                throw await FailWithNativeReceiptAsync(ev.Ticket, "NeedAtLeast", ex).ConfigureAwait(false);
            }
            finally
            {
                ArrayPool<byte>.Shared.Return(rented);
            }
        }

        private async Task HandleNeedRangeAsync(EventV1 ev, CancellationToken ct)
        {
            var src = _sources[ev.IoId];
            int cap = checked((int)Math.Min(ev.TransferMax, 256u * 1024u));
            byte[] rented = ArrayPool<byte>.Shared.Rent(cap);
            try
            {
                int read = await src.ReadAtAsync(checked((long)ev.Offset.Value), rented.AsMemory(0, cap), ct)
                    .ConfigureAwait(false);
                if (read == 0)
                {
                    CheckReplyStatus(SendEnd(ev.Ticket, null), "NeedRange End");
                }
                else
                {
                    CheckReplyStatus(SendBytes(ev.Ticket, rented.AsSpan(0, read)), "NeedRange Bytes");
                }
            }
            catch (Exception ex)
            {
                throw await FailWithNativeReceiptAsync(ev.Ticket, "NeedRange", ex).ConfigureAwait(false);
            }
            finally
            {
                ArrayPool<byte>.Shared.Return(rented);
            }
        }

        private async Task HandleNeedLenAsync(EventV1 ev, CancellationToken ct)
        {
            var src = _sources[ev.IoId];
            try
            {
                long? len = await src.GetLengthAsync(ct).ConfigureAwait(false);
                int rc = len.HasValue && len.Value >= 0
                    ? SendSimpleReply(ev.Ticket, WireConstants.ReplyLength, (ulong)len.Value, 0)
                    : SendSimpleReply(ev.Ticket, WireConstants.ReplyUnknownLength, 0, 0);
                CheckReplyStatus(rc, "NeedLen");
            }
            catch (Exception ex)
            {
                throw await FailWithNativeReceiptAsync(ev.Ticket, "NeedLen", ex).ConfigureAwait(false);
            }
        }

        private async Task HandleOutputAsync(EventV1 ev, bool isPatch, CancellationToken ct)
        {
            var sink = _sinks[ev.IoId];
            int len = checked((int)ev.Length.Value);
            byte[] rented = ArrayPool<byte>.Shared.Rent(len);
            try
            {
                int copied = CopyPendingBytes(ev.Ticket, rented.AsSpan(0, len));
                if (isPatch)
                {
                    await sink.WritePatchAsync(checked((long)ev.Offset.Value), rented.AsMemory(0, copied), ct)
                        .ConfigureAwait(false);
                }
                else
                {
                    await sink.WriteAppendAsync(rented.AsMemory(0, copied), ct).ConfigureAwait(false);
                }
                int rc = SendSimpleReply(ev.Ticket, WireConstants.ReplyAck, (ulong)copied, 0);
                CheckReplyStatus(rc, isPatch ? "Patch Ack" : "Output Ack");
            }
            catch (Exception ex)
            {
                throw await FailWithNativeReceiptAsync(ev.Ticket, isPatch ? "Patch" : "Output", ex)
                    .ConfigureAwait(false);
            }
            finally
            {
                ArrayPool<byte>.Shared.Return(rented);
            }
        }

        private async Task HandleFlushAsync(EventV1 ev, CancellationToken ct)
        {
            var sink = _sinks[ev.IoId];
            try
            {
                await sink.FlushAsync(ct).ConfigureAwait(false);
                CheckReplyStatus(SendSimpleReply(ev.Ticket, WireConstants.ReplyAck, 0, 0), "Flush Ack");
            }
            catch (Exception ex)
            {
                throw await FailWithNativeReceiptAsync(ev.Ticket, "Flush", ex).ConfigureAwait(false);
            }
        }

        private async Task HandleCommitAsync(EventV1 ev)
        {
            // Commit has already won the Running -> Committing race; complete atomically regardless of later cancel.
            try
            {
                if (_commitTransaction is not null)
                {
                    await _commitTransaction.CommitAllAsync().ConfigureAwait(false);
                }
                else
                {
                    IMediaOutputAdapter? singleStaged = null;
                    foreach (var sink in _sinks.Values)
                    {
                        if (!sink.IsIncremental)
                        {
                            if (singleStaged is not null)
                            {
                                throw new InvalidOperationException(
                                    "Multiple staged sinks require an explicit IStagedCommitTransaction.");
                            }
                            singleStaged = sink;
                        }
                    }
                    if (singleStaged is not null)
                    {
                        await singleStaged.CommitAsync().ConfigureAwait(false);
                    }
                }
                _committed = true;
                CheckReplyStatus(SendSimpleReply(ev.Ticket, WireConstants.ReplyAck, 0, 0), "Commit Ack");
            }
            catch (Exception ex)
            {
                throw await FailWithNativeReceiptAsync(ev.Ticket, "Commit", ex).ConfigureAwait(false);
            }
        }

        /// <summary>
        /// Resolve the active ticket with <c>IFX_REPLY_IO_ERROR</c> (checking status), drain the
        /// native terminal <c>StreamErrorReportV2</c> publication receipt BEFORE handle disposal,
        /// roll back any uncommitted staged sinks, and return a <see cref="MediaIoException"/>
        /// preserving BOTH the original failure and exact native receipt!
        ///
        /// Special case for authorized <c>Commit</c>: ordinary <c>Native.Cancel</c> cannot abort
        /// a committed race winner, so if <c>IFX_REPLY_IO_ERROR</c> is rejected during Commit we
        /// attempt <c>IFX_REPLY_ABORTED</c> on the active ticket; if that is also rejected (e.g.
        /// stale ticket), we perform a non-blocking receipt check and return so <c>finally</c>
        /// disposes <see cref="Job"/> and abandons the native worker cleanly without hanging.
        /// </summary>
        private async Task<MediaIoException> FailWithNativeReceiptAsync(
            U64Words ticket,
            string operation,
            Exception cause)
        {
            bool isCommit = string.Equals(operation, "Commit", StringComparison.Ordinal);
            bool unresolvedCommit = false;
            int replyRc = SendIoError(ticket, null, cause.Message);
            if (replyRc != WireConstants.Ok && replyRc != WireConstants.Closed)
            {
                if (isCommit)
                {
                    int abortRc = SendSimpleReply(ticket, WireConstants.ReplyAborted, 0, 0);
                    if (abortRc != WireConstants.Ok && abortRc != WireConstants.Closed)
                    {
                        unresolvedCommit = true;
                    }
                }
                else
                {
                    _ = Native.Cancel(Job);
                }
            }

            string? receiptJson = await DrainTerminalReceiptAsync(unresolvedCommit, isCommit)
                .ConfigureAwait(false);
            await RollbackStagedIfUncommittedAsync().ConfigureAwait(false);
            return new MediaIoException(
                $"Managed media I/O failed during {operation} (reply status {replyRc}): {cause.Message}",
                receiptJson,
                cause);
        }

        private async Task<string?> DrainTerminalReceiptAsync(bool unresolvedCommit, bool isCommit)
        {
            try
            {
                if (unresolvedCommit)
                {
                    // Ticket could not be resolved during authorized Commit; read any already-published
                    // terminal receipt immediately without blocking so `finally` can dispose & abandon.
                    int immediateRc = PollNext(out EventV1 immediateEv);
                    if (immediateRc == WireConstants.Ok
                        && (immediateEv.Kind == WireConstants.EventFailed
                            || immediateEv.Kind == WireConstants.EventComplete))
                    {
                        return ReadResultJson();
                    }
                    return null;
                }

                bool cancelSent = false;
                while (true)
                {
                    int rc = PollNext(out EventV1 ev);
                    if (rc == WireConstants.Ok)
                    {
                        if (ev.Kind == WireConstants.EventFailed || ev.Kind == WireConstants.EventComplete)
                        {
                            return ReadResultJson();
                        }
                        // A stale level-triggered non-terminal event may still be visible briefly
                        // before the worker publishes terminal Failed (e.g. if cancellation raced
                        // `SendIoError`). Never busy-spin on it: ensure cancellation is latched
                        // (for non-Commit) and await actual publication progress or worker release!
                        if (!isCommit && ev.Kind != WireConstants.EventCommit && !cancelSent)
                        {
                            cancelSent = true;
                            _ = Native.Cancel(Job);
                        }
                        if (ev.Kind == WireConstants.EventCommit)
                        {
                            // Attempt once to resolve an active Commit ticket with ReplyAborted; if
                            // rejected, bail out so `finally` disposes & abandons cleanly.
                            int abortRc = SendSimpleReply(ev.Ticket, WireConstants.ReplyAborted, 0, 0);
                            if (abortRc != WireConstants.Ok && abortRc != WireConstants.Closed)
                            {
                                return null;
                            }
                        }
                    }
                    else if (rc != WireConstants.WouldBlock)
                    {
                        return null;
                    }

                    if (NativeReleased.Task.IsCompleted)
                    {
                        int lastRc = PollNext(out EventV1 lastEv);
                        if (lastRc == WireConstants.Ok
                            && (lastEv.Kind == WireConstants.EventFailed
                                || lastEv.Kind == WireConstants.EventComplete))
                        {
                            return ReadResultJson();
                        }
                        return null;
                    }

                    Task waitSig = WaitForSignalAsync();
                    Task finished = await Task.WhenAny(waitSig, NativeReleased.Task).ConfigureAwait(false);
                    if (ReferenceEquals(finished, NativeReleased.Task))
                    {
                        int lastRc = PollNext(out EventV1 lastEv);
                        if (lastRc == WireConstants.Ok
                            && (lastEv.Kind == WireConstants.EventFailed
                                || lastEv.Kind == WireConstants.EventComplete))
                        {
                            return ReadResultJson();
                        }
                        return null;
                    }
                }
            }
            catch
            {
                // Preserve original I/O failure even if native receipt read encounters an error.
            }
            return null;
        }

        private static void CheckReplyStatus(int rc, string operation)
        {
            if (rc == WireConstants.Ok || rc == WireConstants.Closed)
            {
                return;
            }
            throw new InvalidOperationException(
                $"imageflow_stream_job_reply_v1 ({operation}) was rejected with code {rc}");
        }

        private unsafe int CopyPendingBytes(U64Words ticket, Span<byte> dst)
        {
            nuint written = 0;
            fixed (byte* pDst = dst)
            {
                int rc = Native.CopyPending(Job, &ticket, pDst, (nuint)dst.Length, &written);
                if (rc != WireConstants.Ok)
                {
                    throw new InvalidOperationException($"imageflow_stream_job_copy_pending_v1 failed with code {rc}");
                }
            }
            return checked((int)written);
        }

        private unsafe int SendBytes(U64Words ticket, ReadOnlySpan<byte> bytes)
        {
            var reply = new ReplyV1
            {
                Size = (uint)sizeof(ReplyV1),
                Code = WireConstants.ReplyBytes,
                Ticket = ticket,
            };
            fixed (byte* ptr = bytes)
            {
                return Native.Reply(Job, &reply, ptr, (nuint)bytes.Length);
            }
        }

        private unsafe int SendEnd(U64Words ticket, ulong? totalLen)
        {
            var reply = new ReplyV1
            {
                Size = (uint)sizeof(ReplyV1),
                Code = WireConstants.ReplyEnd,
                Flags = totalLen.HasValue ? WireConstants.ReplyFlagHasValue : 0u,
                Ticket = ticket,
                Value = totalLen.HasValue ? U64Words.FromU64(totalLen.Value) : default,
            };
            return Native.Reply(Job, &reply, null, 0);
        }

        private unsafe int SendSimpleReply(U64Words ticket, uint code, ulong value, uint flags)
        {
            var reply = new ReplyV1
            {
                Size = (uint)sizeof(ReplyV1),
                Code = code,
                Flags = flags,
                Ticket = ticket,
                Value = U64Words.FromU64(value),
            };
            return Native.Reply(Job, &reply, null, 0);
        }

        private unsafe int SendIoError(U64Words ticket, ulong? acceptedPrefix, string diagnostic)
        {
            byte[] utf8 = Encoding.UTF8.GetBytes(diagnostic);
            int cap = Math.Min(utf8.Length, 512);
            var reply = new ReplyV1
            {
                Size = (uint)sizeof(ReplyV1),
                Code = WireConstants.ReplyIoError,
                Flags = acceptedPrefix.HasValue ? WireConstants.ReplyFlagHasValue : 0u,
                Ticket = ticket,
                Value = acceptedPrefix.HasValue ? U64Words.FromU64(acceptedPrefix.Value) : default,
            };
            fixed (byte* ptr = utf8)
            {
                return Native.Reply(Job, &reply, cap == 0 ? null : ptr, (nuint)cap);
            }
        }

        private unsafe string ReadResultJson()
        {
            nuint needed = 0;
            int q = Native.ResultJson(Job, null, 0, &needed);
            if (q == WireConstants.Ok && needed == 0) return "{}";
            if (q != WireConstants.BufferTooSmall && q != WireConstants.Ok)
            {
                throw new InvalidOperationException($"imageflow_stream_job_result_json_v1 query failed: {q}");
            }
            byte[] buf = new byte[checked((int)needed)];
            fixed (byte* ptr = buf)
            {
                int rc = Native.ResultJson(Job, ptr, (nuint)buf.Length, &needed);
                if (rc != WireConstants.Ok)
                {
                    throw new InvalidOperationException($"imageflow_stream_job_result_json_v1 copy failed: {rc}");
                }
            }
            return Encoding.UTF8.GetString(buf, 0, checked((int)needed));
        }
    }
}
