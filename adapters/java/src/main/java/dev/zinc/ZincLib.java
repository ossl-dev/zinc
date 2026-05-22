package dev.zinc;

import com.sun.jna.Library;
import com.sun.jna.Native;
import com.sun.jna.Pointer;
import com.sun.jna.ptr.PointerByReference;

interface ZincLib extends Library {
    ZincLib INSTANCE = Native.load("zinc_core", ZincLib.class);

    int zinc_create(String name, long capacity, PointerByReference out);
    int zinc_open(String name, PointerByReference out);
    Pointer zinc_ptr(Pointer handle);
    long zinc_capacity(Pointer handle);
    void zinc_close(Pointer handle);
    void zinc_notify(Pointer handle);
    int zinc_wait(Pointer handle, int timeoutMs);
    int zinc_version();
}
