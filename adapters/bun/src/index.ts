import { dlopen, FFIType, ptr, suffix, toBuffer } from "bun:ffi";
import { join } from "path";

const libPath = join(
  import.meta.dir,
  `../../../target/release/libzinc_core.${suffix}`,
);

const lib = dlopen(libPath, {
  zinc_create: {
    args: [FFIType.cstring, FFIType.usize, FFIType.pointer],
    returns: FFIType.i32,
  },
  zinc_open: {
    args: [FFIType.cstring, FFIType.pointer],
    returns: FFIType.i32,
  },
  zinc_ptr: {
    args: [FFIType.pointer],
    returns: FFIType.pointer,
  },
  zinc_capacity: {
    args: [FFIType.pointer],
    returns: FFIType.usize,
  },
  zinc_close: {
    args: [FFIType.pointer],
    returns: FFIType.void,
  },
  zinc_notify: {
    args: [FFIType.pointer],
    returns: FFIType.void,
  },
  zinc_try_wait: {
    args: [FFIType.pointer],
    returns: FFIType.i32,
  },
  zinc_wait: {
    args: [FFIType.pointer, FFIType.u32],
    returns: FFIType.i32,
  },
});

export class SharedRegion {
  #handle: number | null;

  private constructor(handle: number) {
    this.#handle = handle;
  }

  static create(name: string, capacity: number): SharedRegion {
    if (name.includes("\0")) throw new Error("name contains NUL");
    if (!Number.isSafeInteger(capacity) || capacity <= 0) {
      throw new Error("invalid capacity");
    }
    const out = new BigInt64Array(1);
    const code = lib.symbols.zinc_create(
      Buffer.from(name + "\0"),
      capacity,
      ptr(out),
    );
    if (code !== 0) throw new Error(`zinc_create failed: ${code}`);
    return new SharedRegion(Number(out[0]));
  }

  static open(name: string): SharedRegion {
    if (name.includes("\0")) throw new Error("name contains NUL");
    const out = new BigInt64Array(1);
    const code = lib.symbols.zinc_open(
      Buffer.from(name + "\0"),
      ptr(out),
    );
    if (code !== 0) throw new Error(`zinc_open failed: ${code}`);
    return new SharedRegion(Number(out[0]));
  }

  buffer(): Buffer {
    const dataPtr = lib.symbols.zinc_ptr(this.#liveHandle());
    const len = Number(lib.symbols.zinc_capacity(this.#liveHandle()));
    return toBuffer(dataPtr, 0, len);
  }

  notify(): void {
    lib.symbols.zinc_notify(this.#liveHandle());
  }

  wait(timeoutMs = 1000): boolean {
    if (!Number.isInteger(timeoutMs) || timeoutMs < 0 || timeoutMs > 0xFFFFFFFF) {
      throw new Error("invalid timeout");
    }
    const code = lib.symbols.zinc_wait(this.#liveHandle(), timeoutMs);
    if (code === -110) return false;
    if (code !== 0) throw new Error(`zinc_wait failed: ${code}`);
    return true;
  }

  tryWait(): boolean {
    const code = lib.symbols.zinc_try_wait(this.#liveHandle());
    if (code === -11) return false;
    if (code !== 0) throw new Error(`zinc_try_wait failed: ${code}`);
    return true;
  }

  #liveHandle() {
    if (this.#handle === null) throw new Error("region is closed");
    return this.#handle;
  }

  close(): void {
    if (this.#handle !== null) {
      lib.symbols.zinc_close(this.#handle);
      this.#handle = null;
    }
  }

  [Symbol.dispose](): void {
    this.close();
  }
}
