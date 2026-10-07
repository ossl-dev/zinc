using Xunit;

namespace Zinc.Tests;

public class SharedRegionTests
{
    [Fact]
    public void CreateAndBuffer()
    {
        using var r = SharedRegion.Create("cs_" + Guid.NewGuid().ToString("N")[..16], 16384);
        var span = r.Bytes();
        Assert.Equal(16384, span.Length);
        span[0] = 0xAB;
        Assert.Equal(0xAB, span[0]);
    }

    [Fact]
    public void OpenAndRead()
    {
        var name = "cs_" + Guid.NewGuid().ToString("N")[..16];
        using var owner = SharedRegion.Create(name, 16384);
        owner.Bytes()[0] = 0x42;
        owner.Bytes()[1] = 0x58;

        using var reader = SharedRegion.Open(name);
        Assert.Equal(0x42, reader.Bytes()[0]);
        Assert.Equal(0x58, reader.Bytes()[1]);
    }

    [Fact]
    public void NotifyAndWait()
    {
        var name = "cs_" + Guid.NewGuid().ToString("N")[..16];
        using var region = SharedRegion.Create(name, 16384);

        var t = new Thread(() =>
        {
            using var r2 = SharedRegion.Open(name);
            r2.Bytes()[0] = 99;
            r2.Notify();
        });
        t.Start();

        var signaled = region.Wait(5000);
        Assert.True(signaled);
        Assert.Equal(99, region.Bytes()[0]);
        t.Join();
    }

    [Fact]
    public void OpenNonexistent()
    {
        Assert.Throws<InvalidOperationException>(() => SharedRegion.Open("__cs_nonexistent_xyz__"));
    }

    [Fact]
    public void DisposeIdempotent()
    {
        var r = SharedRegion.Create("cs_" + Guid.NewGuid().ToString("N")[..16], 16384);
        r.Dispose();
        r.Dispose(); // should not crash
    }

    [Fact]
    public void PendingAndClosedHandle()
    {
        var region = SharedRegion.Create("cs_" + Guid.NewGuid().ToString("N")[..16], 16384);
        Assert.False(region.TryWait());
        region.Notify();
        Assert.True(region.TryWait());
        Assert.False(region.Wait(0));
        region.Dispose();
        Assert.Throws<ObjectDisposedException>(() => region.Notify());
        Assert.Throws<ObjectDisposedException>(() => region.TryWait());
    }
}
