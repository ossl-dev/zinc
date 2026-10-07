package dev.zinc;

import com.sun.jna.Pointer;
import com.sun.jna.ptr.PointerByReference;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;

public final class SharedRegion implements AutoCloseable {
    private Pointer handle;

    private SharedRegion(Pointer h) {
        this.handle = h;
    }

    public static SharedRegion create(String name, long capacity) {
        if (capacity <= 0) throw new IllegalArgumentException("capacity must be positive");
        if (name.indexOf('\0') >= 0) throw new IllegalArgumentException("name contains NUL");
        var ref = new PointerByReference();
        int code = ZincLib.INSTANCE.zinc_create(name, capacity, ref);
        if (code != 0) {
            throw new RuntimeException("zinc_create failed: " + code);
        }
        return new SharedRegion(ref.getValue());
    }

    public static SharedRegion open(String name) {
        if (name.indexOf('\0') >= 0) throw new IllegalArgumentException("name contains NUL");
        var ref = new PointerByReference();
        int code = ZincLib.INSTANCE.zinc_open(name, ref);
        if (code != 0) {
            throw new RuntimeException("zinc_open failed: " + code);
        }
        return new SharedRegion(ref.getValue());
    }

    /** Zero-copy ByteBuffer backed by shared memory. */
    public ByteBuffer buffer() {
        long cap = ZincLib.INSTANCE.zinc_capacity(liveHandle());
        return ZincLib.INSTANCE.zinc_ptr(liveHandle()).getByteBuffer(0, cap).order(ByteOrder.nativeOrder());
    }

    public void signal() {
        ZincLib.INSTANCE.zinc_notify(liveHandle());
    }

    public boolean waitForNotification(int timeoutMs) {
        if (timeoutMs < 0) throw new IllegalArgumentException("timeout must be nonnegative");
        int code = ZincLib.INSTANCE.zinc_wait(liveHandle(), timeoutMs);
        if (code == -110) return false;
        if (code != 0) throw new IllegalStateException("zinc_wait failed: " + code);
        return true;
    }

    public boolean tryWait() {
        int code = ZincLib.INSTANCE.zinc_try_wait(liveHandle());
        if (code == -11) return false;
        if (code != 0) throw new IllegalStateException("zinc_try_wait failed: " + code);
        return true;
    }

    private Pointer liveHandle() {
        if (handle == null) throw new IllegalStateException("region is closed");
        return handle;
    }

    @Override
    public void close() {
        if (handle != null) {
            ZincLib.INSTANCE.zinc_close(handle);
            handle = null;
        }
    }
}
