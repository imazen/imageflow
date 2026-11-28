using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using Microsoft.Win32.SafeHandles;

namespace Imageflow.Media;

/// <summary>
/// Staged (<see cref="IsIncremental"/> = <see langword="false"/>) file output adapter that streams
/// encoded bytes into a hidden sibling temporary file on the destination filesystem and only makes
/// the final file visible upon explicit commit.
/// </summary>
public sealed class FileStagedOutputAdapter : IMediaOutputAdapter
{
    private static long s_tempCounter;

    private readonly string _stagingDirectory;
    private readonly string _confinedRoot;
    private readonly Func<string, string> _destinationPathResolver;
    private readonly string? _privateUnixStageDir;
    private readonly string _tempPath;
    private readonly StagedFileIdentity? _stagedIdentity;
    private FileStream? _stream;
    private long _endPos;
    private bool _committed;
    private bool _rolledBack;
    private bool _prepared;

    /// <summary>Final published path once <see cref="PrepareForCommitAsync"/> or <see cref="CommitAsync"/> runs.</summary>
    public string? FinalDestinationPath { get; private set; }

    /// <summary>Lowercase hex SHA-256 of the staged output, populated once prepared or committed.</summary>
    public string? ContentSha256Hex { get; private set; }

    /// <summary>Exact byte count written to the staged file.</summary>
    public long BytesWritten => _endPos;

    internal string ConfinedRoot => _confinedRoot;
    internal string StagingDirectory => _stagingDirectory;
    internal string TempFilePath => _tempPath;
    internal string? PrivateUnixStageDir => _privateUnixStageDir;
    internal StagedFileIdentity? StagedIdentity => _stagedIdentity;

    /// <summary>Create a staged output adapter targeting a fixed <paramref name="destinationPath"/>.</summary>
    public static FileStagedOutputAdapter ForFixedPath(string destinationPath, string? confinedRoot = null) =>
        new(
            Path.GetDirectoryName(Path.GetFullPath(destinationPath)) ?? ".",
            _ => Path.GetFullPath(destinationPath),
            confinedRoot);

    /// <summary>
    /// Create a staged output adapter whose final path is resolved from the staged SHA-256 hex digest.
    /// </summary>
    public static FileStagedOutputAdapter ForResolvedPath(
        string stagingDirectory,
        Func<string, string> destinationPathResolver,
        string? confinedRoot = null) =>
        new(stagingDirectory, destinationPathResolver, confinedRoot);

    /// <summary>Create a staged output adapter targeting a fixed <paramref name="destinationPath"/>.</summary>
    public FileStagedOutputAdapter(string destinationPath)
        : this(
            Path.GetDirectoryName(Path.GetFullPath(destinationPath)) ?? ".",
            _ => Path.GetFullPath(destinationPath),
            null)
    {
    }

    /// <summary>
    /// Create a staged output adapter whose final path is computed from the exact staged SHA-256
    /// hex digest (for example, content-addressed responsive variants).
    /// </summary>
    public FileStagedOutputAdapter(
        string stagingDirectory,
        Func<string, string> destinationPathResolver,
        string? confinedRoot = null)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(stagingDirectory);
        ArgumentNullException.ThrowIfNull(destinationPathResolver);
        _stagingDirectory = Path.GetFullPath(stagingDirectory);
        _confinedRoot = Path.GetFullPath(confinedRoot ?? _stagingDirectory);
        _destinationPathResolver = destinationPathResolver;
        CreateConfinedDirectory(_confinedRoot, _stagingDirectory);
        long seq = Interlocked.Increment(ref s_tempCounter);
        string nonce = Convert.ToHexString(RandomNumberGenerator.GetBytes(8)).ToLowerInvariant();

        if (OperatingSystem.IsLinux() || OperatingSystem.IsMacOS())
        {
            // Stage inside an exclusively-created private 0700 directory (with inherited macOS ACLs
            // stripped) and capture exact (st_dev, st_ino) identity so neither another user nor a
            // non-sticky parent directory swap can substitute the staged payload before renameat!
            string privDirName = $".imageflow-stage-{Environment.ProcessId}-{nonce}-{seq}";
            _privateUnixStageDir = Path.Combine(_stagingDirectory, privDirName);
            (SafeFileHandle handle, string createdLeafName, StagedFileIdentity identity) =
                PosixConfinedFs.CreatePrivateStagedFile(_confinedRoot, _privateUnixStageDir, $"payload-{nonce}.tmp");
            _tempPath = Path.Combine(_privateUnixStageDir, createdLeafName);
            _stagedIdentity = identity;
            _stream = new FileStream(handle, FileAccess.ReadWrite, bufferSize: 65_536, isAsync: false);
        }
        else if (OperatingSystem.IsWindows())
        {
            string tempName = $".imageflow-stage-{Environment.ProcessId}-{nonce}-{seq}.tmp";
            _tempPath = Path.Combine(_stagingDirectory, tempName);
            SafeFileHandle handle = WindowsConfinedFs.CreateConfinedExclusiveFile(_confinedRoot, _tempPath);
            _stream = new FileStream(handle, FileAccess.ReadWrite, bufferSize: 65_536, isAsync: false);
        }
        else
        {
            string tempName = $".imageflow-stage-{Environment.ProcessId}-{nonce}-{seq}.tmp";
            _tempPath = Path.Combine(_stagingDirectory, tempName);
            _stream = new FileStream(
                _tempPath,
                FileMode.CreateNew,
                FileAccess.ReadWrite,
                FileShare.None,
                bufferSize: 65_536,
                FileOptions.Asynchronous);
        }
    }

    public bool SupportsPatch => true;
    public bool IsIncremental => false;

    public async ValueTask WriteAppendAsync(ReadOnlyMemory<byte> chunk, CancellationToken ct)
    {
        FileStream stream = EnsureOpenStream();
        if (stream.Position != _endPos)
        {
            stream.Seek(_endPos, SeekOrigin.Begin);
        }
        await stream.WriteAsync(chunk, ct).ConfigureAwait(false);
        _endPos += chunk.Length;
    }

    public async ValueTask WritePatchAsync(long offset, ReadOnlyMemory<byte> chunk, CancellationToken ct)
    {
        if (offset < 0)
        {
            throw new ArgumentOutOfRangeException(nameof(offset), "Patch offset must be non-negative.");
        }
        FileStream stream = EnsureOpenStream();
        stream.Seek(offset, SeekOrigin.Begin);
        try
        {
            await stream.WriteAsync(chunk, ct).ConfigureAwait(false);
            long patchEnd = offset + chunk.Length;
            if (patchEnd > _endPos)
            {
                _endPos = patchEnd;
            }
        }
        finally
        {
            stream.Seek(_endPos, SeekOrigin.Begin);
        }
    }

    public async ValueTask FlushAsync(CancellationToken ct)
    {
        if (_stream is not null)
        {
            await _stream.FlushAsync(ct).ConfigureAwait(false);
        }
    }

    /// <summary>
    /// Flush, hash, and close the staged temporary file so it is ready for atomic publication.
    /// </summary>
    internal async ValueTask PrepareForCommitAsync(CancellationToken ct = default)
    {
        if (_prepared)
        {
            return;
        }
        if (_rolledBack)
        {
            throw new InvalidOperationException("Cannot prepare a rolled-back staged output.");
        }
        FileStream stream = EnsureOpenStream();
        await stream.FlushAsync(ct).ConfigureAwait(false);
        stream.Seek(0, SeekOrigin.Begin);
        byte[] digest = await SHA256.HashDataAsync(stream, ct).ConfigureAwait(false);
        ContentSha256Hex = Convert.ToHexString(digest).ToLowerInvariant();
        FinalDestinationPath = Path.GetFullPath(_destinationPathResolver(ContentSha256Hex));
        VerifyConfinedNoSymlinks(_confinedRoot, FinalDestinationPath);
        string? destDir = Path.GetDirectoryName(FinalDestinationPath);
        if (!string.IsNullOrEmpty(destDir))
        {
            CreateConfinedDirectory(_confinedRoot, destDir);
        }
        stream.Flush(flushToDisk: true);
        // Always close the stream upon preparation so deferred batches retain ZERO open FDs
        // (no EMFILE risk) and callbacks like beforePublishAsync cannot mutate hashed bytes!
        await stream.DisposeAsync().ConfigureAwait(false);
        _stream = null;
        _prepared = true;
    }

    internal static void VerifyConfinedNoSymlinks(string rootDir, string targetPath)
    {
        string root = Path.GetFullPath(rootDir).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        string target = Path.GetFullPath(targetPath).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);

        if (Directory.Exists(root))
        {
            var ri = new DirectoryInfo(root);
            if (ri.LinkTarget is not null || (ri.Attributes & FileAttributes.ReparsePoint) != 0)
            {
                throw new IOException($"Refusing symlinked root directory '{root}'.");
            }
        }

        if (!string.Equals(target, root, StringComparison.Ordinal)
            && !target.StartsWith(root + Path.DirectorySeparatorChar, StringComparison.Ordinal))
        {
            throw new IOException($"Destination path '{target}' escapes confined root '{root}'.");
        }

        string rel = Path.GetRelativePath(root, target);
        if (rel == "." || string.IsNullOrEmpty(rel))
        {
            return;
        }
        string cur = root;
        foreach (string seg in rel.Split(new[] { Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar }, StringSplitOptions.RemoveEmptyEntries))
        {
            if (seg is "." or "..")
            {
                throw new IOException($"Disallowed traversal segment '{seg}' in '{target}'.");
            }
            cur = Path.Combine(cur, seg);
            if (File.Exists(cur))
            {
                var fi = new FileInfo(cur);
                if (fi.LinkTarget is not null || (fi.Attributes & FileAttributes.ReparsePoint) != 0)
                {
                    throw new IOException($"Refusing symlinked path component '{cur}'.");
                }
            }
            else if (Directory.Exists(cur))
            {
                var di = new DirectoryInfo(cur);
                if (di.LinkTarget is not null || (di.Attributes & FileAttributes.ReparsePoint) != 0)
                {
                    throw new IOException($"Refusing symlinked directory component '{cur}'.");
                }
            }
        }
    }

    /// <summary>
    /// Open and read a regular source file strictly confined beneath <paramref name="rootDir"/>:
    /// - On Linux and macOS: directory-handle-anchored <c>openat(..., O_DIRECTORY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC)</c>
    ///   traversal with seekability / <c>S_ISREG</c> verification (never blocks on FIFOs; immune to parent swaps).
    /// - On Windows: post-open <c>GetFinalPathNameByHandleW</c> verification against both the opened file
    ///   handle and root directory handle.
    /// </summary>
    internal static async Task<byte[]> ReadConfinedRegularFileAsync(
        string rootDir,
        string filePath,
        CancellationToken ct)
    {
        if (OperatingSystem.IsLinux() || OperatingSystem.IsMacOS())
        {
            using SafeFileHandle handle = PosixConfinedFs.OpenConfinedRegularFileReadOnly(rootDir, filePath);
            await using var fs = new FileStream(handle, FileAccess.Read, bufferSize: 65_536, isAsync: false);
            if (!fs.CanSeek)
            {
                throw new IOException($"Refusing non-regular file '{filePath}'.");
            }
            using var ms = new MemoryStream(checked((int)Math.Min(Math.Max(fs.Length, 0L), 64L * 1024L * 1024L)));
            await fs.CopyToAsync(ms, ct).ConfigureAwait(false);
            return ms.ToArray();
        }

        VerifyConfinedNoSymlinks(rootDir, filePath);
        using SafeFileHandle winHandle = File.OpenHandle(
            filePath,
            FileMode.Open,
            FileAccess.Read,
            FileShare.Read,
            FileOptions.Asynchronous);
        if (OperatingSystem.IsWindows())
        {
            WindowsConfinedFs.VerifyHandleConfined(rootDir, winHandle, filePath);
        }
        await using var wfs = new FileStream(winHandle, FileAccess.Read, bufferSize: 65_536, isAsync: true);
        using var wms = new MemoryStream(checked((int)Math.Min(Math.Max(wfs.Length, 0L), 64L * 1024L * 1024L)));
        await wfs.CopyToAsync(wms, ct).ConfigureAwait(false);
        return wms.ToArray();
    }

    internal static void CreateConfinedDirectory(string rootDir, string dirPath)
    {
        if (OperatingSystem.IsLinux() || OperatingSystem.IsMacOS())
        {
            PosixConfinedFs.EnsureConfinedDirectory(rootDir, dirPath);
            return;
        }
        VerifyConfinedNoSymlinks(rootDir, dirPath);
        string root = Path.GetFullPath(rootDir).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        string target = Path.GetFullPath(dirPath).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        if (!Directory.Exists(root))
        {
            Directory.CreateDirectory(root);
        }
        string rel = Path.GetRelativePath(root, target);
        if (rel == "." || string.IsNullOrEmpty(rel))
        {
            return;
        }
        string cur = root;
        foreach (string seg in rel.Split(new[] { Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar }, StringSplitOptions.RemoveEmptyEntries))
        {
            cur = Path.Combine(cur, seg);
            if (Directory.Exists(cur))
            {
                var di = new DirectoryInfo(cur);
                if (di.LinkTarget is not null || (di.Attributes & FileAttributes.ReparsePoint) != 0)
                {
                    throw new IOException($"Refusing symlinked directory component '{cur}'.");
                }
            }
            else if (File.Exists(cur))
            {
                throw new IOException($"Expected directory but found file at '{cur}'.");
            }
            else
            {
                Directory.CreateDirectory(cur);
            }
        }
    }

    internal void MarkCommitted()
    {
        _committed = true;
        if (_stream is not null)
        {
            _stream.Dispose();
            _stream = null;
        }
        if (_privateUnixStageDir is not null)
        {
            PosixConfinedFs.TryRemovePrivateStageDir(
                _confinedRoot,
                _privateUnixStageDir,
                unlinkPayload: false,
                payloadFileName: Path.GetFileName(_tempPath));
        }
    }

    internal void ClearCommittedForRollback()
    {
        _committed = false;
    }

    public async ValueTask CommitAsync()
    {
        if (_committed)
        {
            return;
        }
        var tx = new AtomicFileCommitTransaction(new[] { this });
        await tx.CommitAllAsync().ConfigureAwait(false);
    }

    public async ValueTask RollbackAsync()
    {
        if (_committed || _rolledBack)
        {
            return;
        }
        _rolledBack = true;
        if (_stream is not null)
        {
            try
            {
                await _stream.DisposeAsync().ConfigureAwait(false);
            }
            finally
            {
                _stream = null;
            }
        }
        if (_privateUnixStageDir is not null)
        {
            PosixConfinedFs.TryRemovePrivateStageDir(
                _confinedRoot,
                _privateUnixStageDir,
                unlinkPayload: true,
                payloadFileName: Path.GetFileName(_tempPath));
        }
        else if (OperatingSystem.IsLinux() || OperatingSystem.IsMacOS())
        {
            PosixConfinedFs.TryUnlinkConfined(_confinedRoot, _tempPath);
        }
        else
        {
            TryDeleteFile(_tempPath);
        }
    }

    public async ValueTask DisposeAsync()
    {
        if (!_committed)
        {
            await RollbackAsync().ConfigureAwait(false);
        }
        else if (_stream is not null)
        {
            await _stream.DisposeAsync().ConfigureAwait(false);
            _stream = null;
        }
    }

    private FileStream EnsureOpenStream()
    {
        if (_prepared)
        {
            throw new InvalidOperationException("Cannot mutate a staged output after PrepareForCommitAsync has hashed and sealed it.");
        }
        if (_stream is null)
        {
            throw new ObjectDisposedException(nameof(FileStagedOutputAdapter));
        }
        return _stream;
    }

    internal static void TryDeleteFile(string path)
    {
        try
        {
            if (File.Exists(path))
            {
                File.Delete(path);
            }
        }
        catch
        {
            // Best-effort cleanup of temp/backup artifacts during rollback.
        }
    }
}

/// <summary>
/// Whole-job atomic file publication transaction (<see cref="IStagedCommitTransaction"/>) coordinating
/// 1 or more <see cref="FileStagedOutputAdapter"/> sinks.
///
/// Guarantees BOTH:
/// - Zero missing-entrypoint window: existing regular destinations are snapshotted via sibling
///   hardlinks (never moved aside before replacement) and replaced by a single handle-relative
///   <c>renameat</c> (Linux/macOS) or <c>SetFileInformationByHandle(FileRenameInfo)</c> (Windows).
/// - All-or-nothing batch rollback: destinations are pre-validated (rejecting directories, symlinks,
///   and FIFOs without blocking), and any late failure restores every earlier replaced file from its
///   hardlink snapshot. If any restore fails, its backup recovery copy is PRESERVED on disk and
///   reported; <see cref="IsRolledBack"/> remains <see langword="false"/>.
/// </summary>
public sealed class AtomicFileCommitTransaction : IStagedCommitTransaction
{
    private static long s_backupCounter;
    private readonly IReadOnlyList<FileStagedOutputAdapter> _sinks;
    private readonly Func<CancellationToken, ValueTask>? _beforePublishAsync;
    private bool _partialFailure;

    public bool IsCommitted { get; private set; }
    public bool IsRolledBack { get; private set; }

    public AtomicFileCommitTransaction(
        IReadOnlyList<FileStagedOutputAdapter> sinks,
        Func<CancellationToken, ValueTask>? beforePublishAsync = null)
    {
        _sinks = sinks ?? throw new ArgumentNullException(nameof(sinks));
        _beforePublishAsync = beforePublishAsync;
    }

    public async ValueTask CommitAllAsync()
    {
        if (IsCommitted)
        {
            return;
        }
        if (IsRolledBack)
        {
            throw new InvalidOperationException("Cannot commit an already rolled-back transaction.");
        }
        if (_partialFailure)
        {
            throw new InvalidOperationException("Cannot commit a transaction that previously suffered a partial rollback failure.");
        }

        // Phase 1: flush, hash, resolve final paths, and close every staged temp file.
        for (int i = 0; i < _sinks.Count; i++)
        {
            await _sinks[i].PrepareForCommitAsync().ConfigureAwait(false);
        }

        if (_beforePublishAsync is not null)
        {
            await _beforePublishAsync(CancellationToken.None).ConfigureAwait(false);
        }

        bool isUnix = OperatingSystem.IsLinux() || OperatingSystem.IsMacOS();
        PosixBackupRef?[] posixBackups = new PosixBackupRef?[_sinks.Count];
        string?[] winBackupNames = new string?[_sinks.Count];
        bool[] destExisted = new bool[_sinks.Count];
        uint?[] existingModes = new uint?[_sinks.Count];
        int moved = 0;
        try
        {
            // Phase 1c: pre-validate EVERY destination (rejecting existing directories/symlinks/FIFOs)
            // and snapshot existing regular destinations via sibling hardlinks BEFORE any rename!
            for (int i = 0; i < _sinks.Count; i++)
            {
                FileStagedOutputAdapter sink = _sinks[i];
                string dest = sink.FinalDestinationPath!;
                long seq = Interlocked.Increment(ref s_backupCounter);
                string nonce = Convert.ToHexString(RandomNumberGenerator.GetBytes(6)).ToLowerInvariant();
                string bakName = $".imageflow-bak-{Environment.ProcessId}-{nonce}-{seq}.tmp";
                if (isUnix)
                {
                    if (PosixConfinedFs.PrecheckAndBackupDestination(
                        sink.ConfinedRoot,
                        dest,
                        bakName,
                        out PosixBackupRef? backupRef,
                        out uint? existingMode))
                    {
                        posixBackups[i] = backupRef;
                        destExisted[i] = true;
                        existingModes[i] = existingMode;
                    }
                }
                else
                {
                    if (WindowsConfinedFs.PrecheckAndBackupDestination(sink.ConfinedRoot, dest, bakName))
                    {
                        winBackupNames[i] = bakName;
                        destExisted[i] = true;
                    }
                }
            }

            // Phase 2: single handle-relative atomic rename per destination (entrypoint last; zero missing window!).
            for (int i = 0; i < _sinks.Count; i++)
            {
                FileStagedOutputAdapter sink = _sinks[i];
                string dest = sink.FinalDestinationPath!;
                if (isUnix)
                {
                    PosixConfinedFs.AtomicRenameConfined(
                        sink.ConfinedRoot,
                        sink.TempFilePath,
                        dest,
                        sink.PrivateUnixStageDir is not null,
                        sink.StagedIdentity,
                        destExisted[i],
                        existingModes[i]);
                }
                else if (OperatingSystem.IsWindows())
                {
                    WindowsConfinedFs.AtomicRenameConfined(sink.ConfinedRoot, sink.TempFilePath, dest);
                }
                else
                {
                    FileStagedOutputAdapter.VerifyConfinedNoSymlinks(sink.ConfinedRoot, dest);
                    File.Move(sink.TempFilePath, dest, overwrite: true);
                }
                sink.MarkCommitted();
                moved = i + 1;
            }

            // Phase 3: unlink all sibling backup hardlinks / private fallback backup dirs once committed.
            for (int i = 0; i < _sinks.Count; i++)
            {
                if (isUnix)
                {
                    if (posixBackups[i] is PosixBackupRef bakRef)
                    {
                        PosixConfinedFs.TryRemoveBackup(_sinks[i].ConfinedRoot, _sinks[i].FinalDestinationPath!, bakRef);
                    }
                }
                else if (winBackupNames[i] is string bakName)
                {
                    string dest = _sinks[i].FinalDestinationPath!;
                    string bakFull = Path.Combine(Path.GetDirectoryName(dest) ?? _sinks[i].ConfinedRoot, bakName);
                    FileStagedOutputAdapter.TryDeleteFile(bakFull);
                }
            }
            IsCommitted = true;
        }
        catch (Exception publishEx)
        {
            bool allRestored = true;
            var restoreErrors = new List<string>();

            // Restore EVERY previously moved destination in reverse order from its sibling backup
            // hardlink (or private fallback backup dir), with zero missing window on restore!
            for (int j = moved - 1; j >= 0; j--)
            {
                FileStagedOutputAdapter sink = _sinks[j];
                string dest = sink.FinalDestinationPath!;
                PosixBackupRef? posixBak = posixBackups[j];
                string? winBak = winBackupNames[j];
                try
                {
                    if (isUnix)
                    {
                        PosixConfinedFs.RestoreDestination(sink.ConfinedRoot, dest, posixBak);
                    }
                    else
                    {
                        WindowsConfinedFs.RestoreDestination(sink.ConfinedRoot, dest, winBak);
                    }
                    posixBackups[j] = null;
                    winBackupNames[j] = null;
                    sink.ClearCommittedForRollback();
                }
                catch (Exception rex)
                {
                    allRestored = false;
                    // NEVER delete a recovery backup when restoration fails! Clear slot so cleanup
                    // skips it and report its exact path to the caller for manual recovery.
                    posixBackups[j] = null;
                    winBackupNames[j] = null;
                    string destDir = Path.GetDirectoryName(dest) ?? sink.ConfinedRoot;
                    string? preservedPath = isUnix
                        ? (posixBak is PosixBackupRef pb
                            ? (pb.PrivateDirName is not null
                                ? Path.Combine(destDir, pb.PrivateDirName, pb.FileName)
                                : Path.Combine(destDir, pb.FileName))
                            : null)
                        : (winBak is not null ? Path.Combine(destDir, winBak) : null);
                    string bakNote = preservedPath is not null
                        ? $" (preserved recovery backup at '{preservedPath}')"
                        : string.Empty;
                    restoreErrors.Add($"{dest}: {rex.Message}{bakNote}");
                }
            }

            // Clean up ONLY unused backups (never preserved failed-restore backups!) and unpublished temp files.
            for (int i = 0; i < _sinks.Count; i++)
            {
                if (isUnix)
                {
                    if (posixBackups[i] is PosixBackupRef unusedBak)
                    {
                        PosixConfinedFs.TryRemoveBackup(_sinks[i].ConfinedRoot, _sinks[i].FinalDestinationPath!, unusedBak);
                    }
                }
                else if (winBackupNames[i] is string unusedBak)
                {
                    string dest = _sinks[i].FinalDestinationPath!;
                    string bakFull = Path.Combine(Path.GetDirectoryName(dest) ?? _sinks[i].ConfinedRoot, unusedBak);
                    FileStagedOutputAdapter.TryDeleteFile(bakFull);
                }
            }
            for (int i = moved; i < _sinks.Count; i++)
            {
                await _sinks[i].RollbackAsync().ConfigureAwait(false);
            }

            _partialFailure = !allRestored;
            IsRolledBack = allRestored;
            if (!allRestored)
            {
                throw new IOException(
                    $"Atomic publication failed ({publishEx.Message}) AND rollback could not restore all destinations: {string.Join("; ", restoreErrors)}",
                    publishEx);
            }
            throw;
        }
    }

    public async ValueTask RollbackAllAsync()
    {
        if (_partialFailure)
        {
            throw new InvalidOperationException(
                "Transaction experienced a partial publication/rollback failure; cannot mark rolled back without restoring preserved backup files.");
        }
        IsRolledBack = true;
        foreach (FileStagedOutputAdapter sink in _sinks)
        {
            await sink.RollbackAsync().ConfigureAwait(false);
        }
    }

    /// <summary>
    /// Atomically write <paramref name="bytes"/> to <paramref name="destinationPath"/> via a sibling
    /// temp file and overwrite-move on the same filesystem.
    /// </summary>
    public static async Task WriteFileAtomicAsync(
        string destinationPath,
        ReadOnlyMemory<byte> bytes,
        Func<CancellationToken, ValueTask>? beforePublishAsync = null,
        CancellationToken ct = default,
        string? confinedRoot = null)
    {
        await using var sink = FileStagedOutputAdapter.ForFixedPath(destinationPath, confinedRoot);
        await sink.WriteAppendAsync(bytes, ct).ConfigureAwait(false);
        await sink.FlushAsync(ct).ConfigureAwait(false);
        var tx = new AtomicFileCommitTransaction(new[] { sink }, beforePublishAsync);
        await tx.CommitAllAsync().ConfigureAwait(false);
    }
}

/// <summary>
/// Internal deferred commit coordinator allowing multiple streaming jobs (e.g. still posters +
/// requested video transcodes, or whole-batch FailFast mutable layouts) to authorize native
/// commits individually while publishing all staged files together in one final transaction.
/// </summary>
internal sealed class DeferredCommitTransaction : IStagedCommitTransaction
{
    private readonly IReadOnlyList<FileStagedOutputAdapter> _sinks;

    internal bool NativeAuthorized { get; private set; }
    internal bool RolledBack { get; private set; }

    internal DeferredCommitTransaction(IReadOnlyList<FileStagedOutputAdapter> sinks)
    {
        _sinks = sinks;
    }

    public async ValueTask CommitAllAsync()
    {
        for (int i = 0; i < _sinks.Count; i++)
        {
            await _sinks[i].PrepareForCommitAsync().ConfigureAwait(false);
        }
        NativeAuthorized = true;
    }

    public async ValueTask RollbackAllAsync()
    {
        RolledBack = true;
        for (int i = 0; i < _sinks.Count; i++)
        {
            await _sinks[i].RollbackAsync().ConfigureAwait(false);
        }
    }
}

internal readonly record struct StagedFileIdentity(ulong Dev, ulong Ino);
internal readonly record struct PosixBackupRef(string FileName, string? PrivateDirName, StagedFileIdentity? Identity);

/// <summary>
/// Handle-relative POSIX confinement (<c>openat</c>/<c>mkdirat</c>/<c>mkostempsat_np</c>/<c>linkat</c>/
/// <c>renameat</c>/<c>unlinkat</c>/<c>fchmod</c>/<c>statx</c>/<c>fstat</c>/macOS <c>acl_*_fd_np</c>)
/// for Linux (x64, arm64, x86) and macOS (arm64, x64):
/// - Every file creation, preflight, hardlink backup, copy fallback, permission update, and rename
///   is strictly directory-FD-relative (zero pathname re-resolution on either Linux or macOS).
/// - Staged files live inside exclusively-created private <c>0700</c> owner-only directories (failing
///   closed if any macOS extended ACL is present) and their exact <c>(st_dev, st_ino)</c>
///   identity is verified on <c>stageFd</c> right before <c>fchmod</c> + <c>renameat</c>, blocking
///   even non-sticky parent directory swaps while retaining ZERO open FDs across deferred batches.
/// </summary>
internal static class PosixConfinedFs
{
    private const uint Mode0600 = 0x180; // 0600 octal (private while staging / backup)
    private const uint Mode0644 = 0x1A4; // 0644 octal (readable ceiling on new publication)
    private const uint Mode0700 = 0x1C0; // 0700 octal (private owner-only staging directory)
    private const uint Mode0755 = 0x1ED; // 0755 octal
    private const ushort SIfMt = 0xF000;
    private const ushort SIfDir = 0x4000;
    private const ushort SIfReg = 0x8000;
    private const int LinuxAtEmptyPath = 0x1000;
    private const uint LinuxStatxType = 0x1;
    private const uint LinuxStatxMode = 0x2;
    private const uint LinuxStatxUid = 0x8;
    private const uint LinuxStatxIno = 0x100;
    private const int DarwinAclTypeExtended = 0x100;
    private const int DarwinAclFirstEntry = 0;

    private static int AtFdCwd => OperatingSystem.IsMacOS() ? -2 : -100;
    private static int AtSymlinkNoFollow => OperatingSystem.IsMacOS() ? 0x20 : 0x100;
    private static int AtRemoveDir => OperatingSystem.IsMacOS() ? 0x80 : 0x200;
    private static int ORdWr => 0x2;
    private static int OCreat => OperatingSystem.IsMacOS() ? 0x200 : 0x40;
    private static int OExcl => OperatingSystem.IsMacOS() ? 0x800 : 0x80;
    private static int ONonBlock => OperatingSystem.IsMacOS() ? 0x4 : 0x800;
    private static int ODirectory => OperatingSystem.IsMacOS()
        ? 0x100000
        : (RuntimeInformation.ProcessArchitecture == Architecture.Arm64 ? 0x4000 : 0x10000);
    private static int ONoFollow => OperatingSystem.IsMacOS()
        ? 0x100
        : (RuntimeInformation.ProcessArchitecture == Architecture.Arm64 ? 0x8000 : 0x20000);
    private static int OCloExec => OperatingSystem.IsMacOS() ? 0x1000000 : 0x80000;
    private static int ENoEnt => 2;
    private static int EExist => 17;
    private static int EInval => 22;
    private static int ELoop => OperatingSystem.IsMacOS() ? 62 : 40;
    private static int DarwinENotSup => 45;

    [DllImport("libc", EntryPoint = "openat", SetLastError = true)]
    private static extern int OpenAt3(int dirfd, [MarshalAs(UnmanagedType.LPUTF8Str)] string pathname, int flags);

    // Used strictly on Linux (where SysV/AAPCS64 passes mode in register 4 identically).
    [DllImport("libc", EntryPoint = "openat", SetLastError = true)]
    private static extern int OpenAt4Linux(
        int dirfd,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string pathname,
        int flags,
        uint mode);

    // Non-variadic 4-arg macOS dirfd-relative exclusive 0600 file creator (macOS 10.13+; ARM64 & x64 safe).
    [DllImport("libc", EntryPoint = "mkostempsat_np", SetLastError = true)]
    private static extern unsafe int MkostempsAtNpDarwin(int dfd, byte* templatePath, int suffixLen, int oflags);

    // macOS extended ACL inspection (strictly read-only fail-closed; never mutates ACLs).
    [DllImport("libc", EntryPoint = "acl_free", SetLastError = true)]
    private static extern int AclFreeDarwin(nint obj);

    [DllImport("libc", EntryPoint = "acl_get_fd_np", SetLastError = true)]
    private static extern nint AclGetFdNpDarwin(int fd, int type);

    [DllImport("libc", EntryPoint = "acl_get_entry", SetLastError = true)]
    private static extern int AclGetEntryDarwin(nint acl, int entryId, out nint entry);

    [DllImport("libc", EntryPoint = "mkdirat", SetLastError = true)]
    private static extern int MkdirAt(int dirfd, [MarshalAs(UnmanagedType.LPUTF8Str)] string pathname, uint mode);

    [DllImport("libc", EntryPoint = "linkat", SetLastError = true)]
    private static extern int LinkAt(
        int olddirfd,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string oldpath,
        int newdirfd,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string newpath,
        int flags);

    [DllImport("libc", EntryPoint = "renameat", SetLastError = true)]
    private static extern int RenameAt(
        int olddirfd,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string oldpath,
        int newdirfd,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string newpath);

    [DllImport("libc", EntryPoint = "unlinkat", SetLastError = true)]
    private static extern int UnlinkAt(int dirfd, [MarshalAs(UnmanagedType.LPUTF8Str)] string pathname, int flags);

    [DllImport("libc", EntryPoint = "fchmod", SetLastError = true)]
    private static extern int FChmod(int fd, uint mode);

    [DllImport("libc", EntryPoint = "geteuid")]
    private static extern uint GetEuid();

    [DllImport("libc", EntryPoint = "close", SetLastError = true)]
    private static extern int CloseFd(int fd);

    // Fixed Linux kernel ABI struct statx (256 B, identical across x86, x64, and arm64).
    [StructLayout(LayoutKind.Explicit, Size = 256)]
    private struct LinuxStatx
    {
        [FieldOffset(0)] public uint StxMask;
        [FieldOffset(20)] public uint StxUid;
        [FieldOffset(28)] public ushort StxMode;
        [FieldOffset(32)] public ulong StxIno;
        [FieldOffset(136)] public uint StxDevMajor;
        [FieldOffset(140)] public uint StxDevMinor;
    }

    [DllImport("libc", EntryPoint = "statx", SetLastError = true)]
    private static extern unsafe int StatxLinux(
        int dirfd,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string pathname,
        int flags,
        uint mask,
        LinuxStatx* statxbuf);

    // Fallback fstat structs for older Linux libcs/kernels/seccomp profiles without statx:
    // x86_64: size=144, st_dev@0(u64), st_ino@8(u64), st_mode@24(u32), st_uid@28(u32);
    // arm64 (asm-generic): size=128, st_dev@0(u64), st_ino@8(u64), st_mode@16(u32), st_uid@24(u32);
    // x86 glibc fstat64 & musl legacy fstat compat: size=128, st_dev@0(u64), st_mode@16(u32), st_uid@24(u32), st_ino@88(u64).
    [StructLayout(LayoutKind.Explicit, Size = 144)]
    private struct LinuxX64Stat
    {
        [FieldOffset(0)] public ulong StDev;
        [FieldOffset(8)] public ulong StIno;
        [FieldOffset(24)] public uint StMode;
        [FieldOffset(28)] public uint StUid;
    }

    [StructLayout(LayoutKind.Explicit, Size = 128)]
    private struct LinuxArm64Stat
    {
        [FieldOffset(0)] public ulong StDev;
        [FieldOffset(8)] public ulong StIno;
        [FieldOffset(16)] public uint StMode;
        [FieldOffset(24)] public uint StUid;
    }

    [StructLayout(LayoutKind.Explicit, Size = 128)]
    private struct LinuxX86Stat
    {
        [FieldOffset(0)] public ulong StDev;
        [FieldOffset(16)] public uint StMode;
        [FieldOffset(24)] public uint StUid;
        [FieldOffset(88)] public ulong StIno;
    }

    [DllImport("libc", EntryPoint = "fstat", SetLastError = true)]
    private static extern unsafe int FStatLinuxX64(int fd, LinuxX64Stat* buf);

    [DllImport("libc", EntryPoint = "fstat", SetLastError = true)]
    private static extern unsafe int FStatLinuxArm64(int fd, LinuxArm64Stat* buf);

    [DllImport("libc", EntryPoint = "fstat64", SetLastError = true)]
    private static extern unsafe int FStatLinuxX86Glibc(int fd, LinuxX86Stat* buf);

    [DllImport("libc", EntryPoint = "fstat", SetLastError = true)]
    private static extern unsafe int FStatLinuxX86Musl(int fd, LinuxX86Stat* buf);

    [DllImport("libc", EntryPoint = "fstatat", SetLastError = true)]
    private static extern unsafe int FStatAtLinuxX64(
        int dirfd,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string pathname,
        LinuxX64Stat* buf,
        int flags);

    [DllImport("libc", EntryPoint = "fstatat", SetLastError = true)]
    private static extern unsafe int FStatAtLinuxArm64(
        int dirfd,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string pathname,
        LinuxArm64Stat* buf,
        int flags);

    [DllImport("libc", EntryPoint = "fstatat64", SetLastError = true)]
    private static extern unsafe int FStatAtLinuxX86Glibc(
        int dirfd,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string pathname,
        LinuxX86Stat* buf,
        int flags);

    [DllImport("libc", EntryPoint = "fstatat", SetLastError = true)]
    private static extern unsafe int FStatAtLinuxX86Musl(
        int dirfd,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string pathname,
        LinuxX86Stat* buf,
        int flags);

    [DllImport("libc", EntryPoint = "fstatat", SetLastError = true)]
    private static extern unsafe int FStatAtDarwinArm64(
        int dirfd,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string pathname,
        DarwinStat64* buf,
        int flags);

    [DllImport("libc", EntryPoint = "fstatat$INODE64", SetLastError = true)]
    private static extern unsafe int FStatAtDarwinX64(
        int dirfd,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string pathname,
        DarwinStat64* buf,
        int flags);

    private static unsafe int FStatAtDarwin(int dirfd, string pathname, DarwinStat64* buf, int flags) =>
        RuntimeInformation.ProcessArchitecture switch
        {
            Architecture.Arm64 => FStatAtDarwinArm64(dirfd, pathname, buf, flags),
            Architecture.X64 => FStatAtDarwinX64(dirfd, pathname, buf, flags),
            _ => throw new PlatformNotSupportedException(
                $"Unsupported macOS architecture '{RuntimeInformation.ProcessArchitecture}' for confined fstatat."),
        };

    private static unsafe void AssertRegularEntryInDirMatchesIdentity(
        int dirFd,
        string fileName,
        StagedFileIdentity expected,
        string displayPath)
    {
        if (OperatingSystem.IsLinux())
        {
            bool tryFstatAtFallback = false;
            try
            {
                LinuxStatx stx = default;
                uint reqMask = LinuxStatxType | LinuxStatxIno;
                if (StatxLinux(dirFd, fileName, AtSymlinkNoFollow, reqMask, &stx) == 0)
                {
                    if ((stx.StxMask & LinuxStatxType) == 0
                        || (stx.StxMask & LinuxStatxIno) == 0
                        || (uint)(stx.StxMode & SIfMt) != SIfReg)
                    {
                        throw new IOException($"Rollback fallback backup '{displayPath}' is not a verified regular file.");
                    }
                    ulong devKey = ((ulong)stx.StxDevMajor << 32) | stx.StxDevMinor;
                    var actual = new StagedFileIdentity(devKey, stx.StxIno);
                    if (actual != expected)
                    {
                        throw new IOException($"Rollback fallback backup '{displayPath}' inode identity mismatch; refusing restore.");
                    }
                    return;
                }
                int err = Marshal.GetLastPInvokeError();
                if (err != 38 /* ENOSYS */)
                {
                    throw new IOException($"statx failed verifying rollback fallback backup '{displayPath}' (errno {err}).");
                }
                tryFstatAtFallback = true;
            }
            catch (EntryPointNotFoundException)
            {
                tryFstatAtFallback = true;
            }

            if (tryFstatAtFallback)
            {
                uint mode;
                StagedFileIdentity actual;
                switch (RuntimeInformation.ProcessArchitecture)
                {
                    case Architecture.X64:
                    {
                        LinuxX64Stat st = default;
                        if (FStatAtLinuxX64(dirFd, fileName, &st, AtSymlinkNoFollow) != 0)
                        {
                            throw new IOException($"fstatat failed verifying rollback fallback backup '{displayPath}' (errno {Marshal.GetLastPInvokeError()}).");
                        }
                        mode = st.StMode;
                        actual = new StagedFileIdentity(st.StDev, st.StIno);
                        break;
                    }
                    case Architecture.Arm64:
                    {
                        LinuxArm64Stat st = default;
                        if (FStatAtLinuxArm64(dirFd, fileName, &st, AtSymlinkNoFollow) != 0)
                        {
                            throw new IOException($"fstatat failed verifying rollback fallback backup '{displayPath}' (errno {Marshal.GetLastPInvokeError()}).");
                        }
                        mode = st.StMode;
                        actual = new StagedFileIdentity(st.StDev, st.StIno);
                        break;
                    }
                    case Architecture.X86:
                    {
                        LinuxX86Stat st = default;
                        int rc;
                        try
                        {
                            rc = FStatAtLinuxX86Glibc(dirFd, fileName, &st, AtSymlinkNoFollow);
                        }
                        catch (EntryPointNotFoundException)
                        {
                            rc = FStatAtLinuxX86Musl(dirFd, fileName, &st, AtSymlinkNoFollow);
                        }
                        if (rc != 0)
                        {
                            throw new IOException($"fstatat failed verifying rollback fallback backup '{displayPath}' (errno {Marshal.GetLastPInvokeError()}).");
                        }
                        mode = st.StMode;
                        actual = new StagedFileIdentity(st.StDev, st.StIno);
                        break;
                    }
                    default:
                        throw new PlatformNotSupportedException(
                            $"Unsupported Linux architecture '{RuntimeInformation.ProcessArchitecture}' for confined fstatat fallback.");
                }
                if ((mode & SIfMt) != SIfReg)
                {
                    throw new IOException($"Rollback fallback backup '{displayPath}' is not a regular file.");
                }
                if (actual != expected)
                {
                    throw new IOException($"Rollback fallback backup '{displayPath}' inode identity mismatch; refusing restore.");
                }
                return;
            }
        }
        if (OperatingSystem.IsMacOS())
        {
            DarwinStat64 st = default;
            if (FStatAtDarwin(dirFd, fileName, &st, AtSymlinkNoFollow) != 0)
            {
                throw new IOException($"fstatat failed verifying rollback fallback backup '{displayPath}' (errno {Marshal.GetLastPInvokeError()}).");
            }
            if ((st.StMode & SIfMt) != SIfReg)
            {
                throw new IOException($"Rollback fallback backup '{displayPath}' is not a regular file.");
            }
            var actual = new StagedFileIdentity(unchecked((ulong)(uint)st.StDev), st.StIno);
            if (actual != expected)
            {
                throw new IOException($"Rollback fallback backup '{displayPath}' inode identity mismatch; refusing restore.");
            }
        }
    }

    private static unsafe bool TryGetLinuxFdInfo(
        int fd,
        out uint fileTypeBits,
        out uint? permBits,
        out uint? uid,
        out StagedFileIdentity? identity)
    {
        fileTypeBits = 0;
        permBits = null;
        uid = null;
        identity = null;
        bool tryFstatFallback = false;
        try
        {
            LinuxStatx stx = default;
            uint reqMask = LinuxStatxType | LinuxStatxMode | LinuxStatxUid | LinuxStatxIno;
            if (StatxLinux(fd, string.Empty, LinuxAtEmptyPath, reqMask, &stx) == 0)
            {
                if ((stx.StxMask & LinuxStatxType) == 0)
                {
                    return false;
                }
                fileTypeBits = (uint)(stx.StxMode & SIfMt);
                if ((stx.StxMask & LinuxStatxMode) != 0)
                {
                    permBits = (uint)(stx.StxMode & 0x1FFu);
                }
                if ((stx.StxMask & LinuxStatxUid) != 0)
                {
                    uid = stx.StxUid;
                }
                if ((stx.StxMask & LinuxStatxIno) != 0)
                {
                    ulong devKey = ((ulong)stx.StxDevMajor << 32) | stx.StxDevMinor;
                    identity = new StagedFileIdentity(devKey, stx.StxIno);
                }
                return true;
            }
            int err = Marshal.GetLastPInvokeError();
            if (err != 38 /* ENOSYS */)
            {
                return false;
            }
            tryFstatFallback = true;
        }
        catch (EntryPointNotFoundException)
        {
            tryFstatFallback = true;
        }

        if (!tryFstatFallback)
        {
            return false;
        }

        // Fallback for older libcs (e.g. musl < 1.2.5) or kernels/seccomp profiles without statx.
        switch (RuntimeInformation.ProcessArchitecture)
        {
            case Architecture.X64:
            {
                LinuxX64Stat st = default;
                if (FStatLinuxX64(fd, &st) == 0)
                {
                    fileTypeBits = st.StMode & SIfMt;
                    permBits = st.StMode & 0x1FFu;
                    uid = st.StUid;
                    identity = new StagedFileIdentity(st.StDev, st.StIno);
                    return true;
                }
                return false;
            }
            case Architecture.Arm64:
            {
                LinuxArm64Stat st = default;
                if (FStatLinuxArm64(fd, &st) == 0)
                {
                    fileTypeBits = st.StMode & SIfMt;
                    permBits = st.StMode & 0x1FFu;
                    uid = st.StUid;
                    identity = new StagedFileIdentity(st.StDev, st.StIno);
                    return true;
                }
                return false;
            }
            case Architecture.X86:
            {
                LinuxX86Stat st = default;
                int rc;
                try
                {
                    rc = FStatLinuxX86Glibc(fd, &st);
                }
                catch (EntryPointNotFoundException)
                {
                    // musl legacy fstat compat also places 64-bit st_ino at byte offset 88.
                    rc = FStatLinuxX86Musl(fd, &st);
                }
                if (rc == 0)
                {
                    fileTypeBits = st.StMode & SIfMt;
                    permBits = st.StMode & 0x1FFu;
                    uid = st.StUid;
                    identity = new StagedFileIdentity(st.StDev, st.StIno);
                    return true;
                }
                return false;
            }
            default:
                throw new PlatformNotSupportedException(
                    $"Unsupported Linux architecture '{RuntimeInformation.ProcessArchitecture}' for confined fstat fallback.");
        }
    }

    /// <summary>
    /// Read the process umask thread-safely on Linux from <c>/proc/self/status</c>. Returns
    /// <see langword="null"/> if procfs is unavailable or lacks <c>Umask:</c> so callers keep the
    /// conservative private staging mode rather than widening permissions blindly.
    /// </summary>
    private static uint? TryReadEffectiveNewFileModeLinux()
    {
        try
        {
            foreach (string line in File.ReadLines("/proc/self/status"))
            {
                if (line.StartsWith("Umask:", StringComparison.Ordinal))
                {
                    string raw = line["Umask:".Length..].Trim();
                    uint umaskVal = Convert.ToUInt32(raw, 8);
                    return Mode0644 & ~umaskVal;
                }
            }
        }
        catch
        {
            // Procfs unavailable: return null to preserve restrictive staging mode instead of widening.
        }
        return null;
    }

    /// <summary>
    /// Probe the process umask thread-safely on macOS by creating and immediately removing a probe
    /// subdirectory inside our already-verified private <c>0700</c> staging directory. Returns
    /// <see langword="null"/> on any failure so callers keep conservative <c>0600</c>.
    /// </summary>
    private static unsafe uint? TryReadEffectiveNewFileModeDarwin(int privateStageDirFd)
    {
        const string probeName = ".umask-probe";
        if (MkdirAt(privateStageDirFd, probeName, Mode0755) != 0)
        {
            return null;
        }
        try
        {
            int probeFd = OpenAt3(privateStageDirFd, probeName, ODirectory | ONoFollow | OCloExec);
            if (probeFd < 0)
            {
                return null;
            }
            try
            {
                DarwinStat64 st = default;
                if (FStatDarwin(probeFd, &st) != 0)
                {
                    return null;
                }
                return (uint)(st.StMode & Mode0644);
            }
            finally
            {
                _ = CloseFd(probeFd);
            }
        }
        finally
        {
            _ = UnlinkAt(privateStageDirFd, probeName, AtRemoveDir);
        }
    }

    // Exact macOS 64-bit-inode struct stat (144 B: st_dev@0 i32, st_mode@4 u16, st_ino@8 u64, st_uid@16 u32;
    // identical layout on both ARM64 `fstat` and x86_64 `fstat$INODE64`).
    [StructLayout(LayoutKind.Explicit, Size = 144)]
    private struct DarwinStat64
    {
        [FieldOffset(0)] public int StDev;
        [FieldOffset(4)] public ushort StMode;
        [FieldOffset(8)] public ulong StIno;
        [FieldOffset(16)] public uint StUid;
    }

    [DllImport("libc", EntryPoint = "fstat", SetLastError = true)]
    private static extern unsafe int FStatDarwinArm64(int fd, DarwinStat64* buf);

    [DllImport("libc", EntryPoint = "fstat$INODE64", SetLastError = true)]
    private static extern unsafe int FStatDarwinX64(int fd, DarwinStat64* buf);

    private static unsafe int FStatDarwin(int fd, DarwinStat64* buf) =>
        RuntimeInformation.ProcessArchitecture switch
        {
            Architecture.Arm64 => FStatDarwinArm64(fd, buf),
            Architecture.X64 => FStatDarwinX64(fd, buf),
            _ => throw new PlatformNotSupportedException(
                $"Unsupported macOS architecture '{RuntimeInformation.ProcessArchitecture}' for confined fstat."),
        };

    private static void AssertNoDarwinExtendedAcl(int fd, string displayPath)
    {
        nint acl = AclGetFdNpDarwin(fd, DarwinAclTypeExtended);
        if (acl == 0)
        {
            int err = Marshal.GetLastPInvokeError();
            if (err == ENoEnt || err == DarwinENotSup || err == EInval)
            {
                return;
            }
            throw new IOException($"Failed inspecting macOS extended ACL on '{displayPath}' (errno {err}).");
        }
        try
        {
            if (AclGetEntryDarwin(acl, DarwinAclFirstEntry, out _) == 0)
            {
                throw new IOException(
                    $"Refusing '{displayPath}': unexpected macOS extended ACL entries present on private staging resource.");
            }
        }
        finally
        {
            _ = AclFreeDarwin(acl);
        }
    }

    private static unsafe (int Fd, string CreatedName) CreateExclusive0600InDirFd(
        int dirFd,
        string linuxFileName,
        string darwinPrefix,
        string displayDir)
    {
        if (OperatingSystem.IsLinux())
        {
            int fd = OpenAt4Linux(dirFd, linuxFileName, ORdWr | OCreat | OExcl | ONoFollow | OCloExec, Mode0600);
            if (fd < 0)
            {
                int err = Marshal.GetLastPInvokeError();
                throw new IOException(
                    $"Failed creating handle-confined 0600 file '{linuxFileName}' in '{displayDir}' (errno {err}).");
            }
            return (fd, linuxFileName);
        }

        // macOS: strictly dirfd-relative, non-variadic, exclusive 0600 with O_CLOEXEC at creation time!
        byte[] tmpl = Encoding.UTF8.GetBytes($"{darwinPrefix}-XXXXXX.tmp\0");
        fixed (byte* tmplPtr = tmpl)
        {
            int fd = MkostempsAtNpDarwin(dirFd, tmplPtr, 4, OCloExec);
            if (fd < 0)
            {
                int err = Marshal.GetLastPInvokeError();
                throw new IOException(
                    $"Failed creating dirfd-relative 0600 file in '{displayDir}' via mkostempsat_np (errno {err}).");
            }
            string createdName = Encoding.UTF8.GetString(tmpl, 0, tmpl.Length - 1);
            return (fd, createdName);
        }
    }

    private static (string Root, string ParentRel, string FileName) SplitConfinedPath(string rootDir, string fullPath)
    {
        string root = Path.GetFullPath(rootDir).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        string target = Path.GetFullPath(fullPath).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        if (!target.StartsWith(root + Path.DirectorySeparatorChar, StringComparison.Ordinal))
        {
            throw new IOException($"Path '{target}' escapes confined root '{root}'.");
        }
        string rel = Path.GetRelativePath(root, target);
        string? fileName = Path.GetFileName(rel);
        if (string.IsNullOrEmpty(fileName) || fileName is "." or "..")
        {
            throw new IOException($"Invalid leaf filename in '{target}'.");
        }
        string parentRel = Path.GetDirectoryName(rel) ?? string.Empty;
        return (root, parentRel, fileName);
    }

    private static int OpenConfinedDirFd(string rootDir, string relDir, bool createMissing)
    {
        string root = Path.GetFullPath(rootDir).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        if (createMissing && !Directory.Exists(root))
        {
            Directory.CreateDirectory(root);
        }
        int dirFlags = ODirectory | ONoFollow | OCloExec;
        int cur = OpenAt3(AtFdCwd, root, dirFlags);
        if (cur < 0)
        {
            int err = Marshal.GetLastPInvokeError();
            throw new IOException($"Refusing symlink or inaccessible confined root '{root}' (errno {err}).");
        }
        if (string.IsNullOrEmpty(relDir) || relDir == ".")
        {
            return cur;
        }
        try
        {
            foreach (string seg in relDir.Split(new[] { Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar }, StringSplitOptions.RemoveEmptyEntries))
            {
                if (seg is "." or "..")
                {
                    throw new IOException($"Disallowed path traversal segment '{seg}' beneath '{root}'.");
                }
                int next = OpenAt3(cur, seg, dirFlags);
                if (next < 0)
                {
                    int err = Marshal.GetLastPInvokeError();
                    if (createMissing && err == ENoEnt)
                    {
                        if (MkdirAt(cur, seg, Mode0755) != 0)
                        {
                            int merr = Marshal.GetLastPInvokeError();
                            if (merr != EExist)
                            {
                                throw new IOException($"Failed creating confined dir '{seg}' beneath '{root}' (errno {merr}).");
                            }
                        }
                        next = OpenAt3(cur, seg, dirFlags);
                        if (next < 0)
                        {
                            int oerr = Marshal.GetLastPInvokeError();
                            throw new IOException($"Refusing symlink or non-directory component '{seg}' beneath '{root}' (errno {oerr}).");
                        }
                    }
                    else
                    {
                        throw new IOException($"Refusing symlink or invalid path component '{seg}' beneath '{root}' (errno {err}).");
                    }
                }
                _ = CloseFd(cur);
                cur = next;
            }
            return cur;
        }
        catch
        {
            _ = CloseFd(cur);
            throw;
        }
    }

    private static unsafe (uint? Mode, StagedFileIdentity Identity) AssertRegularFdAndGetInfo(int fd, string displayPath)
    {
        if (OperatingSystem.IsLinux())
        {
            if (!TryGetLinuxFdInfo(fd, out uint fileTypeBits, out uint? permBits, out _, out StagedFileIdentity? id)
                || fileTypeBits != SIfReg
                || !id.HasValue)
            {
                throw new IOException($"Refusing non-regular or unidentifiable file '{displayPath}'.");
            }
            return (permBits, id.Value);
        }
        if (OperatingSystem.IsMacOS())
        {
            DarwinStat64 st = default;
            if (FStatDarwin(fd, &st) != 0 || (st.StMode & SIfMt) != SIfReg)
            {
                throw new IOException($"Refusing non-regular file '{displayPath}'.");
            }
            return ((uint)(st.StMode & 0x1FFu), new StagedFileIdentity(unchecked((ulong)(uint)st.StDev), st.StIno));
        }
        throw new PlatformNotSupportedException("PosixConfinedFs is only supported on Linux and macOS.");
    }

    /// <summary>
    /// Verify that a private staging directory FD is owned by the current effective UID with zero
    /// group/other permissions (<c>0700</c>) and (on macOS) zero extended ACL entries, preventing
    /// any other local user from accessing or modifying entries inside it between staging and <c>renameat</c>.
    /// </summary>
    private static unsafe void AssertPrivateOwnedDirFd(int dirFd, string displayPath)
    {
        uint myUid = GetEuid();
        if (OperatingSystem.IsLinux())
        {
            if (!TryGetLinuxFdInfo(dirFd, out uint ft, out uint? perm, out uint? uid, out _)
                || ft != SIfDir
                || !perm.HasValue
                || !uid.HasValue
                || uid.Value != myUid
                || (perm.Value & 0x3Fu) != 0) // 0077 octal mask must be zero!
            {
                throw new IOException(
                    $"Refusing untrusted or non-private staging directory '{displayPath}' (expected owner uid {myUid} with mode 0700).");
            }
            return;
        }
        if (OperatingSystem.IsMacOS())
        {
            DarwinStat64 st = default;
            if (FStatDarwin(dirFd, &st) != 0
                || (st.StMode & SIfMt) != SIfDir
                || st.StUid != myUid
                || (st.StMode & 0x3Fu) != 0)
            {
                throw new IOException(
                    $"Refusing untrusted or non-private staging directory '{displayPath}' (expected owner uid {myUid} with mode 0700).");
            }
            AssertNoDarwinExtendedAcl(dirFd, displayPath);
        }
    }

    internal static (SafeFileHandle Handle, string CreatedFileName, StagedFileIdentity Identity) CreatePrivateStagedFile(
        string rootDir,
        string privateDirPath,
        string linuxLeafName)
    {
        (string root, string parentRel, string privDirName) = SplitConfinedPath(rootDir, privateDirPath);
        int parentDirFd = OpenConfinedDirFd(root, parentRel, createMissing: true);
        try
        {
            // Exclusively create a brand-new 0700 owner-only staging directory (EEXIST fails closed!).
            if (MkdirAt(parentDirFd, privDirName, Mode0700) != 0)
            {
                int merr = Marshal.GetLastPInvokeError();
                throw new IOException($"Failed creating private 0700 staging directory '{privateDirPath}' (errno {merr}).");
            }
            int privDirFd = OpenAt3(parentDirFd, privDirName, ODirectory | ONoFollow | OCloExec);
            if (privDirFd < 0)
            {
                int oerr = Marshal.GetLastPInvokeError();
                _ = UnlinkAt(parentDirFd, privDirName, AtRemoveDir);
                throw new IOException($"Failed opening private staging directory '{privateDirPath}' (errno {oerr}).");
            }
            bool created = false;
            string? createdName = null;
            try
            {
                // Strictly inspect and fail closed (never mutate ACLs on a reopened directory FD!).
                AssertPrivateOwnedDirFd(privDirFd, privateDirPath);

                (int fileFd, string leafName) = CreateExclusive0600InDirFd(privDirFd, linuxLeafName, "payload", privateDirPath);
                createdName = leafName;
                string fullPath = Path.Combine(privateDirPath, leafName);
                try
                {
                    if (OperatingSystem.IsMacOS())
                    {
                        AssertNoDarwinExtendedAcl(fileFd, fullPath);
                    }
                    (_, StagedFileIdentity identity) = AssertRegularFdAndGetInfo(fileFd, fullPath);
                    created = true;
                    return (new SafeFileHandle((nint)fileFd, ownsHandle: true), leafName, identity);
                }
                catch
                {
                    _ = CloseFd(fileFd);
                    throw;
                }
            }
            finally
            {
                if (!created && createdName is not null)
                {
                    _ = UnlinkAt(privDirFd, createdName, 0);
                }
                _ = CloseFd(privDirFd);
                if (!created)
                {
                    _ = UnlinkAt(parentDirFd, privDirName, AtRemoveDir);
                }
            }
        }
        finally
        {
            _ = CloseFd(parentDirFd);
        }
    }

    internal static void TryRemovePrivateStageDir(
        string rootDir,
        string privateDirPath,
        bool unlinkPayload,
        string payloadFileName)
    {
        try
        {
            (string root, string parentRel, string privDirName) = SplitConfinedPath(rootDir, privateDirPath);
            int parentDirFd = OpenConfinedDirFd(root, parentRel, createMissing: false);
            try
            {
                if (unlinkPayload && !string.IsNullOrEmpty(payloadFileName))
                {
                    int privDirFd = OpenAt3(parentDirFd, privDirName, ODirectory | ONoFollow | OCloExec);
                    if (privDirFd >= 0)
                    {
                        try
                        {
                            AssertPrivateOwnedDirFd(privDirFd, privateDirPath);
                            _ = UnlinkAt(privDirFd, payloadFileName, 0);
                        }
                        finally
                        {
                            _ = CloseFd(privDirFd);
                        }
                    }
                }
                _ = UnlinkAt(parentDirFd, privDirName, AtRemoveDir);
            }
            finally
            {
                _ = CloseFd(parentDirFd);
            }
        }
        catch
        {
            // Best-effort cleanup of private staging directory.
        }
    }

    internal static void EnsureConfinedDirectory(string rootDir, string dirPath)
    {
        string root = Path.GetFullPath(rootDir).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        string target = Path.GetFullPath(dirPath).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        if (!string.Equals(target, root, StringComparison.Ordinal)
            && !target.StartsWith(root + Path.DirectorySeparatorChar, StringComparison.Ordinal))
        {
            throw new IOException($"Directory '{target}' escapes confined root '{root}'.");
        }
        string rel = Path.GetRelativePath(root, target);
        int fd = OpenConfinedDirFd(root, rel == "." ? string.Empty : rel, createMissing: true);
        _ = CloseFd(fd);
    }

    internal static SafeFileHandle OpenConfinedRegularFileReadOnly(string rootDir, string filePath)
    {
        (string root, string parentRel, string fileName) = SplitConfinedPath(rootDir, filePath);
        int dirFd = OpenConfinedDirFd(root, parentRel, createMissing: false);
        try
        {
            // Reject directory leaf before opening as file.
            int probeDir = OpenAt3(dirFd, fileName, ODirectory | ONoFollow | OCloExec);
            if (probeDir >= 0)
            {
                _ = CloseFd(probeDir);
                throw new IOException($"Refusing directory source '{filePath}'.");
            }
            int fd = OpenAt3(dirFd, fileName, ONoFollow | ONonBlock | OCloExec);
            if (fd < 0)
            {
                int err = Marshal.GetLastPInvokeError();
                throw new IOException($"Refusing symlink or inaccessible source '{filePath}' (errno {err}).");
            }
            try
            {
                _ = AssertRegularFdAndGetInfo(fd, filePath);
            }
            catch
            {
                _ = CloseFd(fd);
                throw;
            }
            return new SafeFileHandle((nint)fd, ownsHandle: true);
        }
        finally
        {
            _ = CloseFd(dirFd);
        }
    }

    /// <summary>
    /// Non-blocking destination preflight inside its confined directory FD: refuses directories,
    /// symlinks, FIFOs, sockets, and devices without ever blocking, and if a regular file already
    /// exists captures its existing permission bits and snapshots it via sibling hardlink (or
    /// dirfd-relative exclusive <c>0600</c> copy fallback) with ZERO missing window!
    /// </summary>
    internal static bool PrecheckAndBackupDestination(
        string rootDir,
        string destPath,
        string backupName,
        out PosixBackupRef? backupRef,
        out uint? existingMode)
    {
        backupRef = null;
        existingMode = null;
        (string root, string parentRel, string destName) = SplitConfinedPath(rootDir, destPath);
        int dirFd = OpenConfinedDirFd(root, parentRel, createMissing: true);
        try
        {
            int asDir = OpenAt3(dirFd, destName, ODirectory | ONoFollow | OCloExec);
            if (asDir >= 0)
            {
                _ = CloseFd(asDir);
                throw new IOException($"Refusing to overwrite existing directory at destination '{destPath}'.");
            }
            // O_NONBLOCK guarantees a FIFO at destName can NEVER block this open!
            int asFile = OpenAt3(dirFd, destName, ONoFollow | ONonBlock | OCloExec);
            if (asFile < 0)
            {
                int err = Marshal.GetLastPInvokeError();
                if (err == ENoEnt)
                {
                    return false;
                }
                if (err == ELoop)
                {
                    throw new IOException($"Refusing symlinked destination '{destPath}'.");
                }
                throw new IOException($"Cannot access destination '{destPath}' (errno {err}).");
            }

            using var existingHandle = new SafeFileHandle((nint)asFile, ownsHandle: true);
            (existingMode, _) = AssertRegularFdAndGetInfo(asFile, destPath);
            if (!existingMode.HasValue)
            {
                throw new IOException(
                    $"Cannot determine existing permission bits for destination '{destPath}'; failing closed to avoid widening permissions.");
            }

            // Regular file exists: snapshot via sibling hardlink in the same directory FD!
            if (LinkAt(dirFd, destName, dirFd, backupName, 0) == 0)
            {
                backupRef = new PosixBackupRef(backupName, PrivateDirName: null, Identity: null);
                return true;
            }

            int lerr = Marshal.GetLastPInvokeError();
            // Fallback for filesystems disallowing hardlinks: create and populate the backup inside
            // an exclusively-created private 0700 (and ACL-free on macOS) directory beneath dirFd so
            // no other principal can inherit/race read access while copying!
            if (OperatingSystem.IsMacOS())
            {
                // Refuse byte-copy fallback if the existing file itself carries an extended ACL
                // (e.g. deny ACL) that a byte-copy rollback would lose.
                AssertNoDarwinExtendedAcl(asFile, destPath);
            }
            string destDirDisplay = Path.GetDirectoryName(destPath) ?? root;
            string bakDirName = $".imageflow-bakdir-{Path.GetFileNameWithoutExtension(backupName).TrimStart('.')}";
            string bakDirDisplay = Path.Combine(destDirDisplay, bakDirName);
            if (MkdirAt(dirFd, bakDirName, Mode0700) != 0)
            {
                int merr = Marshal.GetLastPInvokeError();
                throw new IOException(
                    $"Failed creating private 0700 rollback backup directory '{bakDirDisplay}' (linkat errno {lerr}, mkdirat errno {merr}).");
            }
            int bakDirFd = OpenAt3(dirFd, bakDirName, ODirectory | ONoFollow | OCloExec);
            if (bakDirFd < 0)
            {
                int oerr = Marshal.GetLastPInvokeError();
                _ = UnlinkAt(dirFd, bakDirName, AtRemoveDir);
                throw new IOException(
                    $"Failed opening private rollback backup directory '{bakDirDisplay}' (linkat errno {lerr}, openat errno {oerr}).");
            }

            bool backupOk = false;
            string? createdBakName = null;
            try
            {
                // Strictly inspect the private 0700 directory (uid, mode 0700, zero macOS extended ACLs)
                // BEFORE creating or writing a single byte of the backup file!
                AssertPrivateOwnedDirFd(bakDirFd, bakDirDisplay);

                (int dstFd, string leafBakName) = CreateExclusive0600InDirFd(
                    bakDirFd,
                    backupName,
                    "backup",
                    bakDirDisplay);
                createdBakName = leafBakName;
                using var dstHandle = new SafeFileHandle((nint)dstFd, ownsHandle: true);
                if (OperatingSystem.IsMacOS())
                {
                    AssertNoDarwinExtendedAcl(dstFd, Path.Combine(bakDirDisplay, leafBakName));
                }
                (_, StagedFileIdentity bakId) = AssertRegularFdAndGetInfo(dstFd, Path.Combine(bakDirDisplay, leafBakName));
                byte[] copyBuf = new byte[65_536];
                long offset = 0;
                while (true)
                {
                    int n = RandomAccess.Read(existingHandle, copyBuf, offset);
                    if (n == 0) break;
                    RandomAccess.Write(dstHandle, copyBuf.AsSpan(0, n), offset);
                    offset += n;
                }
                RandomAccess.FlushToDisk(dstHandle);
                uint restoreMode = existingMode.Value & 0x1B6u;
                if (FChmod(dstFd, restoreMode) != 0)
                {
                    int ferr = Marshal.GetLastPInvokeError();
                    throw new IOException(
                        $"Failed applying prior mode 0{Convert.ToString(restoreMode, 8)} to fallback backup '{leafBakName}' (errno {ferr}).");
                }
                backupRef = new PosixBackupRef(leafBakName, PrivateDirName: bakDirName, Identity: bakId);
                backupOk = true;
                return true;
            }
            catch (Exception copyEx)
            {
                throw new IOException(
                    $"Failed creating rollback snapshot for '{destPath}' (linkat errno {lerr}, copy failed: {copyEx.Message}).",
                    copyEx);
            }
            finally
            {
                if (!backupOk && createdBakName is not null)
                {
                    _ = UnlinkAt(bakDirFd, createdBakName, 0);
                }
                _ = CloseFd(bakDirFd);
                if (!backupOk)
                {
                    _ = UnlinkAt(dirFd, bakDirName, AtRemoveDir);
                }
            }
        }
        finally
        {
            _ = CloseFd(dirFd);
        }
    }

    internal static void AtomicRenameConfined(
        string rootDir,
        string stagePath,
        string destPath,
        bool isPrivateStageDir,
        StagedFileIdentity? expectedIdentity,
        bool destinationExisted,
        uint? existingDestMode)
    {
        (string root, string stageRel, string stageName) = SplitConfinedPath(rootDir, stagePath);
        (_, string destRel, string destName) = SplitConfinedPath(rootDir, destPath);

        if (destinationExisted && !existingDestMode.HasValue)
        {
            throw new IOException(
                $"Cannot determine prior permission bits for existing destination '{destPath}'; refusing to widen permissions.");
        }

        int stageDirFd = OpenConfinedDirFd(root, stageRel, createMissing: false);
        try
        {
            if (isPrivateStageDir)
            {
                // Verify the staging directory is STILL our private owner-only 0700 directory (and ACL-free on macOS)!
                AssertPrivateOwnedDirFd(stageDirFd, Path.GetDirectoryName(stagePath) ?? stagePath);
            }

            uint? targetMode = destinationExisted
                ? (existingDestMode!.Value & 0x1B6u) // preserve non-exec permission bits (0666 mask)
                : (OperatingSystem.IsLinux()
                    ? TryReadEffectiveNewFileModeLinux()
                    : (isPrivateStageDir ? TryReadEffectiveNewFileModeDarwin(stageDirFd) : null));

            int stageFd = OpenAt3(stageDirFd, stageName, ONoFollow | ONonBlock | OCloExec);
            if (stageFd < 0)
            {
                int err = Marshal.GetLastPInvokeError();
                throw new IOException($"Cannot open staged file '{stagePath}' for publication (errno {err}).");
            }
            try
            {
                (_, StagedFileIdentity actualIdentity) = AssertRegularFdAndGetInfo(stageFd, stagePath);
                if (expectedIdentity.HasValue && actualIdentity != expectedIdentity.Value)
                {
                    throw new IOException(
                        $"Staged file '{stagePath}' inode identity mismatch (expected {expectedIdentity.Value.Dev}:{expectedIdentity.Value.Ino}, got {actualIdentity.Dev}:{actualIdentity.Ino}); refusing publication.");
                }
                if (OperatingSystem.IsMacOS() && isPrivateStageDir)
                {
                    AssertNoDarwinExtendedAcl(stageFd, stagePath);
                }
                if (targetMode.HasValue)
                {
                    if (FChmod(stageFd, targetMode.Value) != 0)
                    {
                        int ferr = Marshal.GetLastPInvokeError();
                        throw new IOException(
                            $"Failed setting publication mode 0{Convert.ToString(targetMode.Value, 8)} on '{stagePath}' (errno {ferr}).");
                    }
                }
            }
            finally
            {
                _ = CloseFd(stageFd);
            }

            int destDirFd = OpenConfinedDirFd(root, destRel, createMissing: true);
            try
            {
                if (RenameAt(stageDirFd, stageName, destDirFd, destName) != 0)
                {
                    int err = Marshal.GetLastPInvokeError();
                    throw new IOException($"Atomic renameat failed for '{destPath}' (errno {err}).");
                }
            }
            finally
            {
                _ = CloseFd(destDirFd);
            }
        }
        finally
        {
            _ = CloseFd(stageDirFd);
        }
    }

    internal static void RestoreDestination(string rootDir, string destPath, PosixBackupRef? backup)
    {
        (string root, string destRel, string destName) = SplitConfinedPath(rootDir, destPath);
        int destDirFd = OpenConfinedDirFd(root, destRel, createMissing: false);
        try
        {
            if (backup is PosixBackupRef bak)
            {
                if (bak.PrivateDirName is null)
                {
                    if (RenameAt(destDirFd, bak.FileName, destDirFd, destName) != 0)
                    {
                        int err = Marshal.GetLastPInvokeError();
                        throw new IOException($"Rollback renameat failed restoring '{destPath}' (errno {err}).");
                    }
                }
                else
                {
                    int bakDirFd = OpenAt3(destDirFd, bak.PrivateDirName, ODirectory | ONoFollow | OCloExec);
                    if (bakDirFd < 0)
                    {
                        int oerr = Marshal.GetLastPInvokeError();
                        throw new IOException($"Rollback cannot open private backup directory '{bak.PrivateDirName}' (errno {oerr}).");
                    }
                    try
                    {
                        string bakDirDisplay = Path.Combine(Path.GetDirectoryName(destPath) ?? root, bak.PrivateDirName);
                        AssertPrivateOwnedDirFd(bakDirFd, bakDirDisplay);
                        if (bak.Identity.HasValue)
                        {
                            // Use no-follow statx/fstatat metadata check (requires ZERO read bits on
                            // the backup file itself, so even mode-0040 backups restore cleanly!).
                            AssertRegularEntryInDirMatchesIdentity(
                                bakDirFd,
                                bak.FileName,
                                bak.Identity.Value,
                                Path.Combine(bakDirDisplay, bak.FileName));
                        }
                        if (RenameAt(bakDirFd, bak.FileName, destDirFd, destName) != 0)
                        {
                            int err = Marshal.GetLastPInvokeError();
                            throw new IOException($"Rollback renameat failed restoring '{destPath}' from private backup (errno {err}).");
                        }
                    }
                    finally
                    {
                        _ = CloseFd(bakDirFd);
                    }
                    _ = UnlinkAt(destDirFd, bak.PrivateDirName, AtRemoveDir);
                }
            }
            else
            {
                if (UnlinkAt(destDirFd, destName, 0) != 0)
                {
                    int err = Marshal.GetLastPInvokeError();
                    if (err != ENoEnt)
                    {
                        throw new IOException($"Rollback unlinkat failed removing '{destPath}' (errno {err}).");
                    }
                }
            }
        }
        finally
        {
            _ = CloseFd(destDirFd);
        }
    }

    internal static void TryRemoveBackup(string rootDir, string destPath, PosixBackupRef backup)
    {
        string destDir = Path.GetDirectoryName(destPath) ?? rootDir;
        if (backup.PrivateDirName is not null)
        {
            TryRemovePrivateStageDir(
                rootDir,
                Path.Combine(destDir, backup.PrivateDirName),
                unlinkPayload: true,
                payloadFileName: backup.FileName);
        }
        else
        {
            TryUnlinkConfined(rootDir, Path.Combine(destDir, backup.FileName));
        }
    }

    internal static void TryUnlinkConfined(string rootDir, string filePath)
    {
        try
        {
            (string root, string parentRel, string fileName) = SplitConfinedPath(rootDir, filePath);
            int dirFd = OpenConfinedDirFd(root, parentRel, createMissing: false);
            try
            {
                _ = UnlinkAt(dirFd, fileName, 0);
            }
            finally
            {
                _ = CloseFd(dirFd);
            }
        }
        catch
        {
            // Best-effort cleanup of temp/backup files.
        }
    }
}

/// <summary>
/// Windows handle-relative confinement, exclusive creation, hardlink backup, publication, and
/// rollback deletion helpers using <c>CreateFileW</c>, <c>GetFinalPathNameByHandleW</c>,
/// <c>SetFileInformationByHandle</c> (<c>FileRenameInfo</c>, <c>FileDispositionInfo</c>), and
/// <c>ntdll.dll</c> <c>NtCreateFile</c> / <c>NtSetInformationFile(FileLinkInformation = 11)</c>.
/// </summary>
internal static class WindowsConfinedFs
{
    private const uint FileNameNormalized = 0x0;
    private const uint DeleteAccess = 0x00010000;
    private const uint FileGenericRead = 0x00120089;
    private const uint FileGenericReadWrite = 0x0013019F; // READ | WRITE | DELETE | SYNCHRONIZE
    private const uint FileWriteAttributes = 0x00000100;
    private const uint FileListDirectory = 0x00000001;
    private const uint Synchronize = 0x00100000;
    private const uint FileShareAll = 0x7;
    private const uint OpenExisting = 3;
    private const uint FileFlagBackupSemantics = 0x02000000;
    private const uint FileFlagOpenReparsePoint = 0x00200000;
    private const int Win32FileRenameInfoClass = 3;
    private const int Win32FileDispositionInfoClass = 4;
    private const int NtFileLinkInformationClass = 11;
    private const uint ObjCaseInsensitive = 0x00000040;
    private const uint FileAttributeNormal = 0x00000080;
    private const uint NtFileCreate = 2; // exclusive create (equivalent to O_CREAT | O_EXCL)
    private const uint NtCreateFileOptions = 0x00000040 | 0x00000020 | 0x00200000; // NON_DIR | SYNC_NONALERT | OPEN_REPARSE_POINT
    private const int ErrorFileNotFound = 2;
    private const int ErrorPathNotFound = 3;

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern SafeFileHandle CreateFileW(
        string lpFileName,
        uint dwDesiredAccess,
        uint dwShareMode,
        nint lpSecurityAttributes,
        uint dwCreationDisposition,
        uint dwFlagsAndAttributes,
        nint hTemplateFile);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern uint GetFinalPathNameByHandleW(
        SafeFileHandle hFile,
        [Out] char[] lpszFilePath,
        uint cchFilePath,
        uint dwFlags);

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern unsafe bool SetFileInformationByHandle(
        SafeFileHandle hFile,
        int fileInformationClass,
        void* lpFileInformation,
        uint dwBufferSize);

    [StructLayout(LayoutKind.Sequential)]
    private struct UnicodeString
    {
        public ushort Length;
        public ushort MaximumLength;
        public nint Buffer;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct ObjectAttributes
    {
        public uint Length;
        public nint RootDirectory;
        public nint ObjectName;
        public uint Attributes;
        public nint SecurityDescriptor;
        public nint SecurityQualityOfService;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct IoStatusBlock
    {
        public nint StatusOrPointer;
        public nuint Information;
    }

    [DllImport("ntdll.dll")]
    private static extern unsafe int NtCreateFile(
        out SafeFileHandle fileHandle,
        uint desiredAccess,
        ObjectAttributes* objectAttributes,
        IoStatusBlock* ioStatusBlock,
        long* allocationSize,
        uint fileAttributes,
        uint shareAccess,
        uint createDisposition,
        uint createOptions,
        void* eaBuffer,
        uint eaLength);

    [DllImport("ntdll.dll")]
    private static extern unsafe int NtSetInformationFile(
        SafeFileHandle fileHandle,
        IoStatusBlock* ioStatusBlock,
        void* fileInformation,
        uint length,
        int fileInformationClass);

    [StructLayout(LayoutKind.Sequential)]
    private struct ByHandleFileInformation
    {
        public uint FileAttributes;
        public ulong CreationTime;
        public ulong LastAccessTime;
        public ulong LastWriteTime;
        public uint VolumeSerialNumber;
        public uint FileSizeHigh;
        public uint FileSizeLow;
        public uint NumberOfLinks;
        public uint FileIndexHigh;
        public uint FileIndexLow;
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetFileInformationByHandle(
        SafeFileHandle hFile,
        out ByHandleFileInformation lpFileInformation);

    private static string GetFinalPath(SafeFileHandle handle)
    {
        char[] buf = new char[1024];
        uint len = GetFinalPathNameByHandleW(handle, buf, (uint)buf.Length, FileNameNormalized);
        if (len == 0)
        {
            throw new IOException($"GetFinalPathNameByHandleW failed (Win32 error {Marshal.GetLastPInvokeError()}).");
        }
        if (len > buf.Length)
        {
            buf = new char[len + 1];
            len = GetFinalPathNameByHandleW(handle, buf, (uint)buf.Length, FileNameNormalized);
            if (len == 0 || len > buf.Length)
            {
                throw new IOException($"GetFinalPathNameByHandleW failed (Win32 error {Marshal.GetLastPInvokeError()}).");
            }
        }
        string raw = new(buf, 0, (int)len);
        return raw.StartsWith(@"\\?\", StringComparison.Ordinal) ? raw[4..] : raw;
    }

    private static SafeFileHandle OpenVerifiedDirHandle(string rootDir, string dirPath)
    {
        string fullRoot = Path.GetFullPath(rootDir).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        string fullDir = Path.GetFullPath(dirPath).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        using SafeFileHandle rootH = CreateFileW(
            fullRoot,
            FileListDirectory | Synchronize,
            FileShareAll,
            0,
            OpenExisting,
            FileFlagBackupSemantics | FileFlagOpenReparsePoint,
            0);
        if (rootH.IsInvalid)
        {
            throw new IOException($"Cannot open confined root handle '{fullRoot}' (Win32 error {Marshal.GetLastPInvokeError()}).");
        }
        string realRoot = GetFinalPath(rootH).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);

        SafeFileHandle dirH = CreateFileW(
            fullDir,
            FileListDirectory | Synchronize,
            FileShareAll,
            0,
            OpenExisting,
            FileFlagBackupSemantics | FileFlagOpenReparsePoint,
            0);
        if (dirH.IsInvalid)
        {
            dirH.Dispose();
            throw new IOException($"Cannot open confined directory handle '{fullDir}' (Win32 error {Marshal.GetLastPInvokeError()}).");
        }
        try
        {
            string realDir = GetFinalPath(dirH).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
            if (!string.Equals(realDir, realRoot, StringComparison.OrdinalIgnoreCase)
                && !realDir.StartsWith(realRoot + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase))
            {
                throw new IOException($"Opened directory handle '{realDir}' escaped confined root '{realRoot}'.");
            }
            return dirH;
        }
        catch
        {
            dirH.Dispose();
            throw;
        }
    }

    /// <summary>
    /// Create a new exclusive regular file strictly relative to the verified parent directory
    /// handle via <c>NtCreateFile(RootDirectory = dirH, FILE_CREATE)</c> — zero pathname creation!
    /// </summary>
    internal static unsafe SafeFileHandle CreateConfinedExclusiveFileRelativeToDir(
        SafeFileHandle dirHandle,
        string leafName,
        string displayPath)
    {
        if (string.IsNullOrEmpty(leafName) || leafName is "." or ".."
            || leafName.IndexOfAny(new[] { '\\', '/' }) >= 0)
        {
            throw new IOException($"Invalid confined leaf filename '{leafName}'.");
        }
        bool addedRef = false;
        try
        {
            dirHandle.DangerousAddRef(ref addedRef);
            fixed (char* pName = leafName)
            {
                ushort byteLen = checked((ushort)(leafName.Length * 2));
                var us = new UnicodeString
                {
                    Length = byteLen,
                    MaximumLength = checked((ushort)(byteLen + 2)),
                    Buffer = (nint)pName,
                };
                var oa = new ObjectAttributes
                {
                    Length = (uint)sizeof(ObjectAttributes),
                    RootDirectory = dirHandle.DangerousGetHandle(),
                    ObjectName = (nint)(&us),
                    Attributes = ObjCaseInsensitive,
                    SecurityDescriptor = 0,
                    SecurityQualityOfService = 0,
                };
                IoStatusBlock iosb = default;
                int status = NtCreateFile(
                    out SafeFileHandle handle,
                    FileGenericReadWrite,
                    &oa,
                    &iosb,
                    null,
                    FileAttributeNormal,
                    shareAccess: 0,
                    NtFileCreate,
                    NtCreateFileOptions,
                    null,
                    0);
                if (status != 0 || handle.IsInvalid)
                {
                    handle?.Dispose();
                    throw new IOException($"Handle-relative NtCreateFile(FILE_CREATE) failed for '{displayPath}' (NTSTATUS 0x{status:X8}).");
                }
                return handle;
            }
        }
        finally
        {
            if (addedRef)
            {
                dirHandle.DangerousRelease();
            }
        }
    }

    internal static SafeFileHandle CreateConfinedExclusiveFile(string rootDir, string fullPath)
    {
        string parentDir = Path.GetDirectoryName(Path.GetFullPath(fullPath)) ?? rootDir;
        string leafName = Path.GetFileName(fullPath);
        using SafeFileHandle dirH = OpenVerifiedDirHandle(rootDir, parentDir);
        return CreateConfinedExclusiveFileRelativeToDir(dirH, leafName, fullPath);
    }

    internal static void VerifyHandleConfined(string rootDir, SafeFileHandle handle, string originalPath)
    {
        string fullRoot = Path.GetFullPath(rootDir).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        using SafeFileHandle rootH = CreateFileW(
            fullRoot,
            FileListDirectory | Synchronize,
            FileShareAll,
            0,
            OpenExisting,
            FileFlagBackupSemantics | FileFlagOpenReparsePoint,
            0);
        if (rootH.IsInvalid)
        {
            throw new IOException($"Cannot open confined root '{fullRoot}' (Win32 error {Marshal.GetLastPInvokeError()}).");
        }
        string realRoot = GetFinalPath(rootH).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
        string finalFile = GetFinalPath(handle);
        if (!finalFile.StartsWith(realRoot + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase))
        {
            throw new IOException($"Opened handle '{finalFile}' for '{originalPath}' escaped confined root '{realRoot}'.");
        }
    }

    private static unsafe byte[] BuildNameInfoBuffer(nint dirRaw, bool replaceIfExists, string leafName)
    {
        // Shared layout of FILE_RENAME_INFO (Win32 class 3) and FILE_LINK_INFORMATION (NT class 11):
        // offset 0: BOOLEAN ReplaceIfExists (1 B) + padding to IntPtr.Size
        // offset IntPtr.Size: HANDLE RootDirectory (IntPtr.Size)
        // offset 2 * IntPtr.Size: DWORD FileNameLength (4 B, bytes without NUL)
        // offset 2 * IntPtr.Size + 4: WCHAR FileName[]
        int ptrSz = IntPtr.Size;
        int nameBytesLen = checked(leafName.Length * 2);
        int headerSz = (2 * ptrSz) + 4;
        byte[] buf = new byte[headerSz + nameBytesLen + 2];
        buf[0] = replaceIfExists ? (byte)1 : (byte)0;
        fixed (byte* pBuf = buf)
        {
            *(nint*)(pBuf + ptrSz) = dirRaw;
            *(uint*)(pBuf + (2 * ptrSz)) = (uint)nameBytesLen;
            Encoding.Unicode.GetBytes(leafName, new Span<byte>(pBuf + headerSz, nameBytesLen));
        }
        return buf;
    }

    internal static unsafe void AtomicRenameConfined(string rootDir, string stagePath, string destPath)
    {
        string destDir = Path.GetDirectoryName(Path.GetFullPath(destPath)) ?? rootDir;
        string destName = Path.GetFileName(destPath);
        if (string.IsNullOrEmpty(destName) || destName is "." or "..")
        {
            throw new IOException($"Invalid destination filename '{destPath}'.");
        }
        using SafeFileHandle destDirH = OpenVerifiedDirHandle(rootDir, destDir);
        using SafeFileHandle stageH = CreateFileW(
            stagePath,
            DeleteAccess | Synchronize,
            FileShareAll,
            0,
            OpenExisting,
            FileFlagOpenReparsePoint,
            0);
        if (stageH.IsInvalid)
        {
            throw new IOException($"Cannot open staged file handle '{stagePath}' (Win32 error {Marshal.GetLastPInvokeError()}).");
        }
        VerifyHandleConfined(rootDir, stageH, stagePath);
        bool addedRef = false;
        try
        {
            destDirH.DangerousAddRef(ref addedRef);
            byte[] buf = BuildNameInfoBuffer(destDirH.DangerousGetHandle(), replaceIfExists: true, destName);
            fixed (byte* pBuf = buf)
            {
                if (!SetFileInformationByHandle(stageH, Win32FileRenameInfoClass, pBuf, (uint)buf.Length))
                {
                    throw new IOException($"Handle-relative SetFileInformationByHandle(FileRenameInfo) failed for '{destPath}' (Win32 error {Marshal.GetLastPInvokeError()}).");
                }
            }
        }
        finally
        {
            if (addedRef)
            {
                destDirH.DangerousRelease();
            }
        }
    }

    internal static unsafe bool PrecheckAndBackupDestination(string rootDir, string destPath, string backupName)
    {
        string destDir = Path.GetDirectoryName(Path.GetFullPath(destPath)) ?? rootDir;
        using SafeFileHandle destDirH = OpenVerifiedDirHandle(rootDir, destDir);

        // Open existing destination handle WITHOUT following reparse points (handles dirs + files).
        using SafeFileHandle existingH = CreateFileW(
            destPath,
            FileGenericRead | FileWriteAttributes | Synchronize,
            FileShareAll,
            0,
            OpenExisting,
            FileFlagBackupSemantics | FileFlagOpenReparsePoint,
            0);
        if (existingH.IsInvalid)
        {
            int err = Marshal.GetLastPInvokeError();
            if (err == ErrorFileNotFound || err == ErrorPathNotFound)
            {
                return false;
            }
            throw new IOException($"Cannot inspect destination handle '{destPath}' (Win32 error {err}).");
        }
        VerifyHandleConfined(rootDir, existingH, destPath);
        if (!GetFileInformationByHandle(existingH, out ByHandleFileInformation info))
        {
            throw new IOException($"GetFileInformationByHandle failed for '{destPath}' (Win32 error {Marshal.GetLastPInvokeError()}).");
        }
        var attrs = (FileAttributes)info.FileAttributes;
        if ((attrs & FileAttributes.Directory) != 0)
        {
            throw new IOException($"Refusing to overwrite existing directory at destination '{destPath}'.");
        }
        if ((attrs & FileAttributes.ReparsePoint) != 0)
        {
            throw new IOException($"Refusing symlink/reparse destination '{destPath}'.");
        }

        // Create handle-relative hardlink backup via NT NtSetInformationFile(FileLinkInformation = 11)
        // anchored strictly to destDirH (Win32/NT exact equivalent of POSIX linkat)!
        bool linked = false;
        bool addedRef = false;
        try
        {
            destDirH.DangerousAddRef(ref addedRef);
            byte[] linkBuf = BuildNameInfoBuffer(destDirH.DangerousGetHandle(), replaceIfExists: false, backupName);
            IoStatusBlock iosb = default;
            fixed (byte* pBuf = linkBuf)
            {
                int ntStatus = NtSetInformationFile(
                    existingH,
                    &iosb,
                    pBuf,
                    (uint)linkBuf.Length,
                    NtFileLinkInformationClass);
                linked = ntStatus == 0;
            }
        }
        finally
        {
            if (addedRef)
            {
                destDirH.DangerousRelease();
            }
        }

        if (!linked)
        {
            // Fallback for non-NTFS volumes disallowing hardlinks: create exclusive backup strictly
            // relative to destDirH via NtCreateFile(RootDirectory = destDirH, FILE_CREATE) and copy
            // via RandomAccess (never transfers/disposes existingH or bakH!) so bakH stays live for
            // clean FileDispositionInfo deletion if copy/flush fails!
            string bakPath = Path.Combine(destDir, backupName);
            using SafeFileHandle bakH = CreateConfinedExclusiveFileRelativeToDir(destDirH, backupName, bakPath);
            try
            {
                byte[] copyBuf = new byte[65_536];
                long offset = 0;
                while (true)
                {
                    int n = RandomAccess.Read(existingH, copyBuf, offset);
                    if (n == 0) break;
                    RandomAccess.Write(bakH, copyBuf.AsSpan(0, n), offset);
                    offset += n;
                }
                RandomAccess.FlushToDisk(bakH);
            }
            catch
            {
                try
                {
                    byte del = 1;
                    _ = SetFileInformationByHandle(bakH, Win32FileDispositionInfoClass, &del, 1);
                }
                catch
                {
                    // Never mask the original copy/flush exception during partial backup cleanup.
                }
                throw;
            }
        }
        return true;
    }

    internal static unsafe void TryDeleteHandleConfined(string rootDir, string targetPath)
    {
        using SafeFileHandle h = CreateFileW(
            targetPath,
            DeleteAccess | Synchronize,
            FileShareAll,
            0,
            OpenExisting,
            FileFlagOpenReparsePoint,
            0);
        if (h.IsInvalid)
        {
            int err = Marshal.GetLastPInvokeError();
            if (err == ErrorFileNotFound || err == ErrorPathNotFound)
            {
                return;
            }
            throw new IOException($"Cannot open handle to delete '{targetPath}' (Win32 error {err}).");
        }
        VerifyHandleConfined(rootDir, h, targetPath);
        byte deleteFlag = 1;
        if (!SetFileInformationByHandle(h, Win32FileDispositionInfoClass, &deleteFlag, 1))
        {
            throw new IOException($"Handle-relative delete (FileDispositionInfo) failed for '{targetPath}' (Win32 error {Marshal.GetLastPInvokeError()}).");
        }
    }

    internal static void RestoreDestination(string rootDir, string destPath, string? backupName)
    {
        if (backupName is not null)
        {
            string bakPath = Path.Combine(Path.GetDirectoryName(Path.GetFullPath(destPath)) ?? rootDir, backupName);
            AtomicRenameConfined(rootDir, bakPath, destPath);
        }
        else
        {
            TryDeleteHandleConfined(rootDir, destPath);
        }
    }
}
