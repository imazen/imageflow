using System;
using System.Buffers;
using System.Collections.Generic;
using System.IO.Pipelines;
using System.Threading;
using System.Threading.Tasks;

namespace Imageflow.Media;

/// <summary>
/// Forward-only <see cref="PipeReader"/> input adapter enforcing exact <c>AdvanceTo(consumed, examined)</c>.
/// </summary>
public sealed class PipeReaderInputAdapter : IMediaInputAdapter
{
    private readonly PipeReader _reader;
    private readonly bool _leaveOpen;

    public PipeReaderInputAdapter(PipeReader reader, bool leaveOpen = true)
    {
        _reader = reader ?? throw new ArgumentNullException(nameof(reader));
        _leaveOpen = leaveOpen;
    }

    public bool SupportsRange => false;
    public bool HasKnownLength => false;

    public ValueTask<long?> GetLengthAsync(CancellationToken ct) => ValueTask.FromResult<long?>(null);

    public async ValueTask<int> ReadForwardAsync(Memory<byte> dst, CancellationToken ct)
    {
        while (true)
        {
            ReadResult result = await _reader.ReadAsync(ct).ConfigureAwait(false);
            ReadOnlySequence<byte> buffer = result.Buffer;
            if (result.IsCanceled)
            {
                // PipeReader contract requires AdvanceTo after every successful ReadAsync, including canceled reads.
                _reader.AdvanceTo(buffer.Start, buffer.Start);
                throw new OperationCanceledException(ct);
            }
            if (buffer.IsEmpty)
            {
                _reader.AdvanceTo(buffer.End, buffer.End);
                if (result.IsCompleted)
                {
                    return 0;
                }
                continue;
            }
            int take = (int)Math.Min(buffer.Length, dst.Length);
            ReadOnlySequence<byte> slice = buffer.Slice(0, take);
            slice.CopyTo(dst.Span);
            // Leave unconsumed tail untouched with exact examined boundary!
            _reader.AdvanceTo(slice.End, slice.End);
            return take;
        }
    }

    public ValueTask<int> ReadAtAsync(long offset, Memory<byte> dst, CancellationToken ct) =>
        throw new NotSupportedException("PipeReader is forward-only.");

    public async ValueTask DisposeAsync()
    {
        if (!_leaveOpen)
        {
            await _reader.CompleteAsync().ConfigureAwait(false);
        }
    }
}

/// <summary>
/// Incremental forward-only <see cref="PipeWriter"/> output adapter with explicit <c>FlushAsync</c> backpressure.
/// </summary>
public sealed class PipeWriterOutputAdapter : IMediaOutputAdapter
{
    private readonly PipeWriter _writer;
    private readonly bool _leaveOpen;

    public PipeWriterOutputAdapter(PipeWriter writer, bool leaveOpen = true)
    {
        _writer = writer ?? throw new ArgumentNullException(nameof(writer));
        _leaveOpen = leaveOpen;
    }

    public bool SupportsPatch => false;
    public bool IsIncremental => true;

    public async ValueTask WriteAppendAsync(ReadOnlyMemory<byte> chunk, CancellationToken ct)
    {
        Memory<byte> dst = _writer.GetMemory(chunk.Length);
        chunk.CopyTo(dst);
        _writer.Advance(chunk.Length);
        FlushResult flush = await _writer.FlushAsync(ct).ConfigureAwait(false);
        if (flush.IsCanceled) throw new OperationCanceledException(ct);
        if (flush.IsCompleted) throw new InvalidOperationException("PipeWriter reader completed early.");
    }

    public ValueTask WritePatchAsync(long offset, ReadOnlyMemory<byte> chunk, CancellationToken ct) =>
        throw new NotSupportedException("PipeWriter does not support seek-back patches.");

    public async ValueTask FlushAsync(CancellationToken ct)
    {
        FlushResult flush = await _writer.FlushAsync(ct).ConfigureAwait(false);
        if (flush.IsCanceled) throw new OperationCanceledException(ct);
    }

    public ValueTask CommitAsync() => ValueTask.CompletedTask;
    public ValueTask RollbackAsync() => ValueTask.CompletedTask;

    public async ValueTask DisposeAsync()
    {
        if (!_leaveOpen)
        {
            await _writer.CompleteAsync().ConfigureAwait(false);
        }
    }
}

/// <summary>
/// Forward-only <see cref="IAsyncEnumerable{T}"/> input adapter with empty-element loop guard.
/// Callers own this adapter and MUST dispose it (<c>await using</c>) after <see cref="MediaJob"/>
/// completes so the underlying enumerator is always disposed cleanly.
/// </summary>
public sealed class AsyncSequenceInputAdapter : IMediaInputAdapter
{
    private const int MaxConsecutiveEmptyChunks = 1024;
    private readonly IAsyncEnumerable<ReadOnlyMemory<byte>> _sequence;
    private IAsyncEnumerator<ReadOnlyMemory<byte>>? _enumerator;
    private ReadOnlyMemory<byte> _pending;

    public AsyncSequenceInputAdapter(IAsyncEnumerable<ReadOnlyMemory<byte>> sequence)
    {
        _sequence = sequence ?? throw new ArgumentNullException(nameof(sequence));
    }

    public bool SupportsRange => false;
    public bool HasKnownLength => false;

    public ValueTask<long?> GetLengthAsync(CancellationToken ct) => ValueTask.FromResult<long?>(null);

    public async ValueTask<int> ReadForwardAsync(Memory<byte> dst, CancellationToken ct)
    {
        _enumerator ??= _sequence.GetAsyncEnumerator(ct);
        int emptyStreak = 0;
        while (_pending.IsEmpty)
        {
            if (!await _enumerator.MoveNextAsync().ConfigureAwait(false))
            {
                return 0;
            }
            _pending = _enumerator.Current;
            if (_pending.IsEmpty && ++emptyStreak > MaxConsecutiveEmptyChunks)
            {
                throw new InvalidOperationException("Async input sequence yielded too many consecutive empty chunks.");
            }
        }
        int take = Math.Min(_pending.Length, dst.Length);
        _pending.Slice(0, take).CopyTo(dst);
        _pending = _pending.Slice(take);
        return take;
    }

    public ValueTask<int> ReadAtAsync(long offset, Memory<byte> dst, CancellationToken ct) =>
        throw new NotSupportedException("IAsyncEnumerable input is forward-only.");

    public async ValueTask DisposeAsync()
    {
        if (_enumerator is not null)
        {
            await _enumerator.DisposeAsync().ConfigureAwait(false);
            _enumerator = null;
        }
    }
}
