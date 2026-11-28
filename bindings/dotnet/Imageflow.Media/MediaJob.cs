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
    /// The context handle is ref-counted across <c>imageflow_stream_job_create_v1</c> and may safely
    /// be disposed immediately after this call returns (or even while the returned task runs).
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
            cancellationToken);

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
            cancellationToken);

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
            cancellationToken);

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
            cancellationToken);

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
            cancellationToken);

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
            cancellationToken);

    private static Task<MediaResult> RunWithSafeContextAsync(
        SafeHandle context,
        uint operation,
        string envelopeJson,
        IReadOnlyDictionary<int, IMediaInputAdapter> sources,
        IReadOnlyDictionary<int, IMediaOutputAdapter> destinations,
        IStagedCommitTransaction? commitTransaction,
        CancellationToken caller)
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
        return RunCreatedJobAsync(job, sources, destinations, commitTransaction, caller);
    }

    private static Task<MediaResult> RunOperationAsync(
        nint context,
        uint operation,
        string envelopeJson,
        IReadOnlyDictionary<int, IMediaInputAdapter> sources,
        IReadOnlyDictionary<int, IMediaOutputAdapter> destinations,
        IStagedCommitTransaction? commitTransaction,
        CancellationToken caller)
    {
        if (context == nint.Zero)
        {
            throw new ArgumentException("Native context handle must be non-zero.", nameof(context));
        }
        WireConstants.AssertLayouts();
        caller.ThrowIfCancellationRequested();
        ValidateStagedTransactionRequirement(operation, destinations, commitTransaction);

        JobHandle job = CreateJob(context, operation, envelopeJson, sources, destinations);
        return RunCreatedJobAsync(job, sources, destinations, commitTransaction, caller);
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
        CancellationToken caller)
    {
        var job = createdJob;
        var pump = new Pump(job, sources, destinations, commitTransaction);

        using var ioStop = CancellationTokenSource.CreateLinkedTokenSource(caller);
        CancellationTokenRegistration registration = caller.UnsafeRegister(
            static state => { _ = Native.Cancel((JobHandle)state!); },
            job);

        Task? watchStop = null;
        bool started = false;
        try
        {
            int accepted = StartAsyncNative(pump);
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
            // Reverse-order cleanup:
            // 1. Unregister caller cancel hook BEFORE disposing JobHandle (prevents callback racing release).
            // 2. Dispose JobHandle BEFORE awaiting NativeReleased so an abandoned in-flight Commit
            //    triggers native abandonment and guarantees worker quiescence!
            registration.Dispose();
            job.Dispose();
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

    private static unsafe int StartAsyncNative(Pump pump)
    {
        var root = GCHandle.Alloc(pump);
        var hooks = new HooksV1
        {
            Size = (uint)sizeof(HooksV1),
            Flags = 0,
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
        private readonly object _sigLock = new();
        private bool _eventPending;
        private TaskCompletionSource<bool>? _waiter;
        private bool _committed;
        private bool _rolledBack;

        internal JobHandle Job { get; }
        internal TaskCompletionSource<bool> NativeReleased { get; } =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
        internal TaskCompletionSource<bool> StopLatched { get; } =
            new(TaskCreationOptions.RunContinuationsAsynchronously);

        internal Pump(
            JobHandle job,
            IReadOnlyDictionary<int, IMediaInputAdapter> sources,
            IReadOnlyDictionary<int, IMediaOutputAdapter> sinks,
            IStagedCommitTransaction? commitTransaction)
        {
            Job = job;
            _sources = sources;
            _sinks = sinks;
            _commitTransaction = commitTransaction;
        }

        internal void Enqueue(uint flags)
        {
            if ((flags & WireConstants.NotifyStop) != 0)
            {
                StopLatched.TrySetResult(true);
            }
            if ((flags & (WireConstants.NotifyEvent | WireConstants.NotifyStop)) != 0)
            {
                TaskCompletionSource<bool>? toWake = null;
                lock (_sigLock)
                {
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
                        return new MediaResult(ReadResultJson(), Succeeded: true);

                    case WireConstants.EventFailed:
                        await RollbackStagedIfUncommittedAsync().ConfigureAwait(false);
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
