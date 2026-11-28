using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace Imageflow.Media;

/// <summary>
/// Word-split 64-bit unsigned integer matching <c>IfxU64V1</c> in <c>imageflow_stream_v1.h</c>
/// (alignment 4 on every 32-bit and 64-bit target, endian-independent).
/// </summary>
[StructLayout(LayoutKind.Sequential)]
internal struct U64Words
{
    internal uint Lo;
    internal uint Hi;

    internal readonly ulong Value => ((ulong)Hi << 32) | Lo;

    internal static U64Words FromU64(ulong v) => new()
    {
        Lo = unchecked((uint)v),
        Hi = unchecked((uint)(v >> 32)),
    };
}

[StructLayout(LayoutKind.Sequential)]
internal struct BindingV1
{
    internal uint Size;
    internal int IoId;
    internal uint Flags;
    internal uint Reserved;
}

[StructLayout(LayoutKind.Sequential)]
internal struct EventV1
{
    internal uint Size;
    internal uint Kind;
    internal int IoId;
    internal uint Flags;
    internal U64Words Ticket;
    internal U64Words Offset;
    internal U64Words Length;
    internal uint TransferMax;
    internal uint Status;
    internal uint Reason;
    internal uint Reserved;
}

[StructLayout(LayoutKind.Sequential)]
internal struct ReplyV1
{
    internal uint Size;
    internal uint Code;
    internal uint Flags;
    internal uint Reserved;
    internal U64Words Ticket;
    internal U64Words Value;
}

[StructLayout(LayoutKind.Sequential)]
internal struct ProgressV1
{
    internal uint Size;
    internal uint Status;
    internal uint Flags;
    internal uint Reserved;
    internal double Fraction;
    internal double UnresolvedFraction;
    internal U64Words Generation;
}

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct HooksV1
{
    internal uint Size;
    internal uint Flags;
    internal nint User;
    internal delegate* unmanaged[Cdecl]<nint, uint, void> Notify;
    internal delegate* unmanaged[Cdecl]<nint, void> ReleaseUser;
}

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct ReadAtV1
{
    internal uint Size;
    internal uint Flags;
    internal nint User;
    internal delegate* unmanaged[Cdecl]<nint, U64Words*, byte*, nuint, nuint*, int> ReadAt;
    internal delegate* unmanaged[Cdecl]<nint, U64Words*, int> Length;
    internal delegate* unmanaged[Cdecl]<nint, void> ReleaseUser;
}

internal static class WireConstants
{
    internal const uint StreamAbiVersionV1 = 0x0001_0000u;
    internal const uint FeatManualV1 = 1u << 0;
    internal const uint FeatAsyncV1 = 1u << 1;
    internal const uint FeatProgressV1 = 1u << 2;

    internal const uint HookProgressPull = 1u << 0;
    internal const uint HookProgressNotify = 1u << 1;

    internal const uint ProgressFlagHasFraction = 1u << 0;
    internal const uint ProgressFlagTerminal = 1u << 1;

    internal const uint OpExecuteV1 = 1u;
    internal const uint OpInspectV1 = 2u;
    internal const uint OpEstimateV1 = 3u;

    internal const int Ok = 0;
    internal const int WouldBlock = 1;
    internal const int BufferTooSmall = 2;
    internal const int StaleTicket = 3;
    internal const int Closed = 4;
    internal const int WrongThread = 5;
    internal const int Busy = 6;
    internal const int End = 7;
    internal const int LengthUnknown = 8;

    internal const uint BindSource = 1u << 0;
    internal const uint BindSink = 1u << 1;
    internal const uint BindSourceRange = 1u << 2;
    internal const uint BindSourceKnownLen = 1u << 3;
    internal const uint BindSinkPatch = 1u << 2;
    internal const uint BindSinkIncremental = 1u << 3;

    internal const uint EventNeedAtLeast = 1u;
    internal const uint EventNeedRange = 2u;
    internal const uint EventNeedLen = 3u;
    internal const uint EventOutput = 4u;
    internal const uint EventPatch = 5u;
    internal const uint EventFlush = 6u;
    internal const uint EventCommit = 7u;
    internal const uint EventYield = 8u;
    internal const uint EventComplete = 9u;
    internal const uint EventFailed = 10u;

    internal const uint ReplyBytes = 1u;
    internal const uint ReplyEnd = 2u;
    internal const uint ReplyLength = 3u;
    internal const uint ReplyUnknownLength = 4u;
    internal const uint ReplyAck = 5u;
    internal const uint ReplyAborted = 6u;
    internal const uint ReplyIoError = 7u;
    internal const uint ReplyFlagHasValue = 1u << 0;

    internal const uint NotifyEvent = 1u << 0;
    internal const uint NotifyStop = 1u << 1;
    internal const uint NotifyProgress = 1u << 2;

    /// <summary>
    /// Verifies exact managed struct sizes and field offsets match <c>imageflow_stream_v1.h</c>.
    /// </summary>
    internal static void AssertLayouts()
    {
        if (Unsafe.SizeOf<U64Words>() != 8) throw new InvalidOperationException("U64Words size mismatch");
        if (Unsafe.SizeOf<BindingV1>() != 16) throw new InvalidOperationException("BindingV1 size mismatch");
        if (Unsafe.SizeOf<EventV1>() != 56) throw new InvalidOperationException("EventV1 size mismatch");
        if (Unsafe.SizeOf<ReplyV1>() != 32) throw new InvalidOperationException("ReplyV1 size mismatch");
        if (Unsafe.SizeOf<ProgressV1>() != 40) throw new InvalidOperationException("ProgressV1 size mismatch");
        int expectedHooks = IntPtr.Size == 8 ? 32 : 20;
        int expectedReadAt = IntPtr.Size == 8 ? 40 : 24;
        if (Unsafe.SizeOf<HooksV1>() != expectedHooks) throw new InvalidOperationException("HooksV1 size mismatch");
        if (Unsafe.SizeOf<ReadAtV1>() != expectedReadAt) throw new InvalidOperationException("ReadAtV1 size mismatch");
    }
}

/// <summary>
/// SafeHandle owning one <c>IfxJobV1*</c>. Reference-counted by the CLR across every concurrent
/// P/Invoke call (including cancellation callbacks), preventing handle reuse or use-after-free.
/// </summary>
internal sealed class JobHandle : SafeHandle
{
    public JobHandle() : base(nint.Zero, ownsHandle: true) { }

    internal JobHandle(nint existing) : base(nint.Zero, ownsHandle: true)
    {
        SetHandle(existing);
    }

    public override bool IsInvalid => handle == nint.Zero;

    protected override bool ReleaseHandle() => Native.Release(handle) == WireConstants.Ok;
}

internal static unsafe partial class Native
{
    private const string Lib = "imageflow";

    [LibraryImport(Lib, EntryPoint = "imageflow_stream_abi_version_v1")]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial uint AbiVersion();

    [LibraryImport(Lib, EntryPoint = "imageflow_stream_abi_features_v1")]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial uint AbiFeatures();

    [LibraryImport(Lib, EntryPoint = "imageflow_stream_job_create_v1")]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial int Create(
        nint context,
        uint operation,
        byte* json,
        nuint jsonLen,
        BindingV1* bindings,
        nuint count,
        out nint outJob);

    [LibraryImport(Lib, EntryPoint = "imageflow_stream_job_start_async_v1")]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial int StartAsync(JobHandle job, HooksV1* hooks);

    [LibraryImport(Lib, EntryPoint = "imageflow_stream_job_next_v1")]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial int Next(JobHandle job, uint work, EventV1* outEvent);

    [LibraryImport(Lib, EntryPoint = "imageflow_stream_job_copy_pending_v1")]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial int CopyPending(
        JobHandle job,
        U64Words* ticket,
        byte* dst,
        nuint cap,
        nuint* written);

    [LibraryImport(Lib, EntryPoint = "imageflow_stream_job_reply_v1")]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial int Reply(JobHandle job, ReplyV1* reply, byte* bytes, nuint len);

    [LibraryImport(Lib, EntryPoint = "imageflow_stream_job_result_json_v1")]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial int ResultJson(JobHandle job, byte* dst, nuint cap, nuint* needed);

    [LibraryImport(Lib, EntryPoint = "imageflow_stream_job_cancel_v1")]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial int Cancel(JobHandle job);

    [LibraryImport(Lib, EntryPoint = "imageflow_stream_job_release_v1")]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial int Release(nint job);

    [LibraryImport(Lib, EntryPoint = "imageflow_stream_job_enable_progress_v1")]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial int EnableProgress(JobHandle job);

    [LibraryImport(Lib, EntryPoint = "imageflow_stream_job_progress_v1")]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial int Progress(JobHandle job, ProgressV1* outProgress);

    [LibraryImport(Lib, EntryPoint = "imageflow_stream_job_progress_json_v1")]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial int ProgressJson(JobHandle job, byte* dst, nuint cap, nuint* needed);

    [LibraryImport(Lib, EntryPoint = "imageflow_context_has_error")]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    [return: MarshalAs(UnmanagedType.I1)]
    internal static partial bool ContextHasError(nint context);

    [LibraryImport(Lib, EntryPoint = "imageflow_context_send_json")]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    internal static partial nint ContextSendJson(
        nint context,
        byte* methodUtf8NullTerm,
        byte* json,
        nuint jsonLen);

    [LibraryImport(Lib, EntryPoint = "imageflow_json_response_read")]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    [return: MarshalAs(UnmanagedType.I1)]
    internal static partial bool JsonResponseRead(
        nint context,
        nint response,
        out long httpCode,
        out byte* utf8Ptr,
        out nuint utf8Len);

    [LibraryImport(Lib, EntryPoint = "imageflow_json_response_destroy")]
    [UnmanagedCallConv(CallConvs = new[] { typeof(CallConvCdecl) })]
    [return: MarshalAs(UnmanagedType.I1)]
    internal static partial bool JsonResponseDestroy(nint context, nint response);

    internal static (long HttpCode, string Json, bool HasError) SendContextJson(
        SafeHandle context,
        string method,
        string payload)
    {
        ArgumentNullException.ThrowIfNull(context);
        byte[] methodBytes = System.Text.Encoding.UTF8.GetBytes(method + "\0");
        byte[] payloadBytes = System.Text.Encoding.UTF8.GetBytes(payload);
        bool addedRef = false;
        try
        {
            context.DangerousAddRef(ref addedRef);
            nint raw = context.DangerousGetHandle();
            fixed (byte* mPtr = methodBytes)
            fixed (byte* pPtr = payloadBytes)
            {
                nint resp = ContextSendJson(raw, mPtr, pPtr, (nuint)payloadBytes.Length);
                if (resp == nint.Zero)
                {
                    throw new InvalidOperationException($"imageflow_context_send_json('{method}') returned null response.");
                }
                try
                {
                    if (!JsonResponseRead(raw, resp, out long code, out byte* outPtr, out nuint outLen))
                    {
                        throw new InvalidOperationException($"imageflow_json_response_read('{method}') failed.");
                    }
                    string json = outLen > 0
                        ? System.Text.Encoding.UTF8.GetString(new ReadOnlySpan<byte>(outPtr, checked((int)outLen)))
                        : string.Empty;
                    bool hasError = ContextHasError(raw);
                    return (code, json, hasError);
                }
                finally
                {
                    JsonResponseDestroy(raw, resp);
                }
            }
        }
        finally
        {
            if (addedRef)
            {
                context.DangerousRelease();
            }
        }
    }
}
