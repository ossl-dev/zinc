import { dlopen, FFIType, suffix, ptr, toBuffer } from "bun:ffi";
import { join } from "path";

const libPath = join(
  import.meta.dir,
  `../../core/target/release/libzinc_core.${suffix}`,
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
  zinc_wait: {
    args: [FFIType.pointer, FFIType.u32],
    returns: FFIType.i32,
  },
});

export class SharedRegion {
  #handle: bigint;

  private constructor(handle: bigint) {
    this.#handle = handle;
  }

  static create(name: string, capacity: number): SharedRegion {
    const out = new BigInt64Array(1);
    const code = lib.symbols.zinc_create(
      Buffer.from(name + "\0"),
      capacity,
      ptr(out),
    );
    if (code !== 0) throw new Error(`zinc_create failed: ${code}`);
    return new SharedRegion(out[0]);
  }

  static open(name: string): SharedRegion {
    const out = new BigInt64Array(1);
    const code = lib.symbols.zinc_open(
      Buffer.from(name + "\0"),
      ptr(out),
    );
    if (code !== 0) throw new Error(`zinc_open failed: ${code}`);
    return new SharedRegion(out[0]);
  }

  buffer(): Buffer {
    const dataPtr = lib.symbols.zinc_ptr(this.#handle);
    const len = Number(lib.symbols.zinc_capacity(this.#handle));
    return toBuffer(dataPtr, len);
  }

  notify(): void {
    lib.symbols.zinc_notify(this.#handle);
  }

  wait(timeoutMs = 1000): boolean {
    return lib.symbols.zinc_wait(this.#handle, timeoutMs) === 0;
  }

  close(): void {
    lib.symbols.zinc_close(this.#handle);
  }

  [Symbol.dispose](): void {
    this.close();
  }
}
