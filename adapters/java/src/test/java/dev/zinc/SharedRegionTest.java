package dev.zinc;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.BeforeAll;
import static org.junit.jupiter.api.Assertions.*;
import java.nio.ByteBuffer;

class SharedRegionTest {

    private static boolean SKIP = false;

    @BeforeAll
    static void checkLibrary() {
        try {
            var r = SharedRegion.create("__java_skip_test__", 4096);
            r.close();
        } catch (Exception e) {
            SKIP = true;
        }
    }

    @Test
    void testCreateAndBuffer() {
        if (SKIP) return;
        var r = SharedRegion.create("java_test_" + System.currentTimeMillis(), 4096);
        ByteBuffer buf = r.buffer();
        assertEquals(4096, buf.capacity());
        buf.put(0, (byte) 0xAB);
        assertEquals((byte) 0xAB, buf.get(0));
        r.close();
    }

    @Test
    void testOpenAndRead() {
        if (SKIP) return;
        String name = "java_test_ro_" + System.currentTimeMillis();
        var owner = SharedRegion.create(name, 4096);
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
        if (SKIP) return;
        String name = "java_test_nw_" + System.currentTimeMillis();
        var region = SharedRegion.create(name, 4096);

        Thread writer = new Thread(() -> {
            var r2 = SharedRegion.open(name);
            r2.buffer().put(0, (byte) 99);
            r2.notify();
            r2.close();
        });
        writer.start();

        boolean signaled = region.wait(5000);
        assertTrue(signaled, "wait() should return true");
        assertEquals((byte) 99, region.buffer().get(0));
        writer.join();
        region.close();
    }

    @Test
    void testOpenNonexistent() {
        if (SKIP) return;
        assertThrows(RuntimeException.class, () -> SharedRegion.open("__java_nonexistent_xyz__"));
    }

    @Test
    void testAutoCloseable() {
        if (SKIP) return;
        var r = SharedRegion.create("java_test_ac_" + System.currentTimeMillis(), 4096);
        r.close();
        // Second close should not crash (handle already null)
        r.close();
    }
}
