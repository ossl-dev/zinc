package dev.zinc;

import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;
import java.nio.ByteBuffer;

class SharedRegionTest {

    @Test
    void testCreateAndBuffer() {
        var r = SharedRegion.create("java_" + Long.toHexString(System.nanoTime()), 16384);
        ByteBuffer buf = r.buffer();
        assertEquals(16384, buf.capacity());
        buf.put(0, (byte) 0xAB);
        assertEquals((byte) 0xAB, buf.get(0));
        r.close();
    }

    @Test
    void testOpenAndRead() {
        String name = "java_" + Long.toHexString(System.nanoTime());
        var owner = SharedRegion.create(name, 16384);
        ByteBuffer obuf = owner.buffer();
        obuf.put(0, (byte) 0x42);
        obuf.put(1, (byte) 0x58);

        var reader = SharedRegion.open(name);
        ByteBuffer rbuf = reader.buffer();
        assertEquals((byte) 0x42, rbuf.get(0));
        assertEquals((byte) 0x58, rbuf.get(1));
        reader.close();
        owner.close();
    }

    @Test
    void testNotifyWait() throws Exception {
        String name = "java_" + Long.toHexString(System.nanoTime());
        var region = SharedRegion.create(name, 16384);

        Thread writer = new Thread(() -> {
            var r2 = SharedRegion.open(name);
            r2.buffer().put(0, (byte) 99);
            r2.signal();
            r2.close();
        });
        writer.start();

        boolean signaled = region.waitForNotification(5000);
        assertTrue(signaled, "wait() should return true");
        assertEquals((byte) 99, region.buffer().get(0));
        writer.join();
        region.close();
    }

    @Test
    void testOpenNonexistent() {
        assertThrows(RuntimeException.class, () -> SharedRegion.open("__java_nonexistent_xyz__"));
    }

    @Test
    void testAutoCloseable() {
        var r = SharedRegion.create("java_" + Long.toHexString(System.nanoTime()), 16384);
        r.close();
        // Second close should not crash (handle already null)
        r.close();
    }

    @Test
    void testPendingAndClosedHandle() {
        var region = SharedRegion.create("java_" + Long.toHexString(System.nanoTime()), 16384);
        assertFalse(region.tryWait());
        region.signal();
        assertTrue(region.tryWait());
        assertFalse(region.waitForNotification(0));
        region.close();
        assertThrows(IllegalStateException.class, region::buffer);
        assertThrows(IllegalStateException.class, region::signal);
        assertThrows(IllegalStateException.class, region::tryWait);
    }
}
