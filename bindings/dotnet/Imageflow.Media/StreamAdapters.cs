using System;
using System.IO;
using System.Threading;
using System.Threading.Tasks;

namespace Imageflow.Media;

/// <summary>
/// <see cref="Stream"/> input adapter with leave-open default and serialized seek+read for range access.
/// </summary>
public sealed class StreamInputAdapter : IMediaInputAdapter
{
    private readonly Stream _stream;
    private readonly bool _leaveOpen;
    private readonly long _origin;
    private readonly SemaphoreSlim _posLock = new(1, 1);

    public StreamInputAdapter(Stream stream, bool leaveOpen = true)
    {
        _stream = stream ?? throw new ArgumentNullException(nameof(stream));
        _leaveOpen = leaveOpen;
        _origin = stream.CanSeek ? stream.Position : 0;
    }

    public bool SupportsRange => _stream.CanSeek;
    public bool HasKnownLength => _stream.CanSeek;

    public ValueTask<long?> GetLengthAsync(CancellationToken ct)
    {
        ct.ThrowIfCancellationRequested();
        return _stream.CanSeek ? ValueTask.FromResult<long?>(_stream.Length - _origin) : ValueTask.FromResult<long?>(null);
    }

    public ValueTask<int> ReadForwardAsync(Memory<byte> dst, CancellationToken ct) =>
        _stream.ReadAsync(dst, ct);

    public async ValueTask<int> ReadAtAsync(long offset, Memory<byte> dst, CancellationToken ct)
    {
        if (!_stream.CanSeek)
        {
            throw new NotSupportedException("Underlying Stream does not support seeking.");
        }
        await _posLock.WaitAsync(ct).ConfigureAwait(false);
        try
        {
            _stream.Seek(_origin + offset, SeekOrigin.Begin);
            return await _stream.ReadAsync(dst, ct).ConfigureAwait(false);
        }
        finally
        {
            _posLock.Release();
        }
    }

    public async ValueTask DisposeAsync()
    {
        _posLock.Dispose();
        if (!_leaveOpen)
        {
            await _stream.DisposeAsync().ConfigureAwait(false);
        }
    }
}

/// <summary>
/// Incremental <see cref="Stream"/> output adapter supporting origin-relative seek/patch/restore.
/// </summary>
public sealed class StreamOutputAdapter : IMediaOutputAdapter
{
    private readonly Stream _stream;
    private readonly bool _leaveOpen;
    private readonly long _origin;
    private long _endPos;

    public StreamOutputAdapter(Stream stream, bool leaveOpen = true)
    {
        _stream = stream ?? throw new ArgumentNullException(nameof(stream));
        _leaveOpen = leaveOpen;
        _origin = stream.CanSeek ? stream.Position : 0;
        _endPos = _origin;
    }

    public bool SupportsPatch => _stream.CanSeek;
    public bool IsIncremental => true;

    public async ValueTask WriteAppendAsync(ReadOnlyMemory<byte> chunk, CancellationToken ct)
    {
        if (_stream.CanSeek && _stream.Position != _endPos)
        {
            _stream.Seek(_endPos, SeekOrigin.Begin);
        }
        await _stream.WriteAsync(chunk, ct).ConfigureAwait(false);
        _endPos += chunk.Length;
    }

    public async ValueTask WritePatchAsync(long offset, ReadOnlyMemory<byte> chunk, CancellationToken ct)
    {
        if (!_stream.CanSeek)
        {
            throw new NotSupportedException("Underlying Stream does not support seek-back patching.");
        }
        _stream.Seek(_origin + offset, SeekOrigin.Begin);
        try
        {
            await _stream.WriteAsync(chunk, ct).ConfigureAwait(false);
        }
        finally
        {
            _stream.Seek(_endPos, SeekOrigin.Begin);
        }
    }

    public ValueTask FlushAsync(CancellationToken ct) => new(_stream.FlushAsync(ct));
    public ValueTask CommitAsync() => ValueTask.CompletedTask;
    public ValueTask RollbackAsync() => ValueTask.CompletedTask;

    public async ValueTask DisposeAsync()
    {
        if (!_leaveOpen)
        {
            await _stream.DisposeAsync().ConfigureAwait(false);
        }
    }
}
