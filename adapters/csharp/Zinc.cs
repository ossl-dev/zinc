using System;
using System.Runtime.InteropServices;

namespace Zinc;

internal static class Native
{
    const string Lib = "zinc_core";

    [DllImport(Lib)]
    internal static extern int zinc_create(string name, nuint capacity, out nint handle);

    [DllImport(Lib)]
    internal static extern int zinc_open(string name, out nint handle);

    [DllImport(Lib)]
    internal static extern nint zinc_ptr(nint handle);

    [DllImport(Lib)]
    internal static extern nuint zinc_capacity(nint handle);

    [DllImport(Lib)]
    internal static extern void zinc_close(nint handle);

    [DllImport(Lib)]
    internal static extern void zinc_notify(nint handle);

    [DllImport(Lib)]
    internal static extern int zinc_wait(nint handle, uint timeoutMs);
}

public sealed unsafe class SharedRegion : IDisposable
{
    nint _handle;

    public static SharedRegion Create(string name, nuint capacity)
    {
        if (Native.zinc_create(name, capacity, out var h) is not 0 and var e)
            throw new InvalidOperationException($"zinc_create failed: {e}");
        return new() { _handle = h };
    }

    public static SharedRegion Open(string name)
    {
        if (Native.zinc_open(name, out var h) is not 0 and var e)
            throw new InvalidOperationException($"zinc_open failed: {e}");
        return new() { _handle = h };
    }

    /// <summary>Zero-copy Span over shared memory.</summary>
    public Span<byte> Bytes() =>
        new((void*)Native.zinc_ptr(_handle), (int)Native.zinc_capacity(_handle));

    public void Notify() => Native.zinc_notify(_handle);

    public bool Wait(uint timeoutMs = 1000) => Native.zinc_wait(_handle, timeoutMs) == 0;

    public void Dispose()
    {
        if (_handle != 0)
        {
            Native.zinc_close(_handle);
            _handle = 0;
        }
    }
}
