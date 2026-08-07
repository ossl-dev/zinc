using Xunit;

namespace Zinc.Tests;

public class SharedRegionTests
{
    private static bool Skip = !TryCreate();

    private static bool TryCreate()
    {
        try
        {
            using var r = SharedRegion.Create("__cs_skip_test__", 4096);
            return true;
        }
        catch
        {
            return false;
        }
    }

    [Fact]
    public void CreateAndBuffer()
    {
        if (Skip) return;

        using var r = SharedRegion.Create($"cs_test_{DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()}", 4096);
        var span = r.Bytes();
        Assert.Equal(4096, span.Length);
        span[0] = 0xAB;
        Assert.Equal(0xAB, span[0]);
    }

    [Fact]
    public void OpenAndRead()
    {
        if (Skip) return;

        var name = $"cs_test_ro_{DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()}";
        using var owner = SharedRegion.Create(name, 4096);
        owner.Bytes()[0] = 0x42;
        owner.Bytes()[1] = 0x58;

        using var reader = SharedRegion.Open(name);
        Assert.Equal(0x42, reader.Bytes()[0]);
        Assert.Equal(0x58, reader.Bytes()[1]);
    }

    [Fact]
    public void NotifyAndWait()
    {
        if (Skip) return;

        var name = $"cs_test_nw_{DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()}";
        using var region = SharedRegion.Create(name, 4096);

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
        if (Skip) return;
        Assert.Throws<InvalidOperationException>(() => SharedRegion.Open("__cs_nonexistent_xyz__"));
    }

    [Fact]
    public void DisposeIdempotent()
    {
        if (Skip) return;

        var r = SharedRegion.Create($"cs_test_disp_{DateTimeOffset.UtcNow.ToUnixTimeMilliseconds()}", 4096);
        r.Dispose();
        r.Dispose(); // should not crash
    }
}
