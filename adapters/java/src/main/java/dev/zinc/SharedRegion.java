package dev.zinc;

import com.sun.jna.Pointer;
import com.sun.jna.ptr.PointerByReference;
import java.nio.ByteBuffer;

public final class SharedRegion implements AutoCloseable {
    private Pointer handle;

    private SharedRegion(Pointer h) {
        this.handle = h;
    }

    public static SharedRegion create(String name, long capacity) {
        var ref = new PointerByReference();
        int code = ZincLib.INSTANCE.zinc_create(name, capacity, ref);
        if (code != 0) {
            throw new RuntimeException("zinc_create failed: " + code);
        }
        return new SharedRegion(ref.getValue());
    }

    public static SharedRegion open(String name) {
        var ref = new PointerByReference();
        int code = ZincLib.INSTANCE.zinc_open(name, ref);
        if (code != 0) {
            throw new RuntimeException("zinc_open failed: " + code);
        }
        return new SharedRegion(ref.getValue());
    }

    /** Zero-copy ByteBuffer backed by shared memory. */
    public ByteBuffer buffer() {
        long cap = ZincLib.INSTANCE.zinc_capacity(handle);
        return ZincLib.INSTANCE.zinc_ptr(handle).getByteBuffer(0, cap);
    }

    public void notify() {
        ZincLib.INSTANCE.zinc_notify(handle);
    }

    public boolean wait(int timeoutMs) {
        return ZincLib.INSTANCE.zinc_wait(handle, timeoutMs) == 0;
    }

    @Override
    public void close() {
        if (handle != null) {
            ZincLib.INSTANCE.zinc_close(handle);
            handle = null;
        }
    }
}
