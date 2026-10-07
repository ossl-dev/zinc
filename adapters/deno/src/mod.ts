import { join } from "@std/path";

if (Deno.build.os !== "darwin" && Deno.build.os !== "linux") {
  throw new Error("Zinc supports Linux and macOS only");
}
const suffix = Deno.build.os === "darwin" ? "dylib" : "so";

const libPath = join(
  import.meta.dirname!,
  `../../../target/release/libzinc_core.${suffix}`,
);

const lib = Deno.dlopen(libPath, {
  zinc_create: {
    parameters: ["buffer", "usize", "pointer"],
    result: "i32",
  },
  zinc_open: {
    parameters: ["buffer", "pointer"],
    result: "i32",
  },
  zinc_ptr: {
    parameters: ["pointer"],
    result: "pointer",
  },
  zinc_capacity: {
    parameters: ["pointer"],
    result: "usize",
  },
  zinc_close: {
    parameters: ["pointer"],
    result: "void",
  },
  zinc_notify: {
    parameters: ["pointer"],
    result: "void",
  },
  zinc_try_wait: {
    parameters: ["pointer"],
    result: "i32",
  },
  zinc_wait: {
    parameters: ["pointer", "u32"],
    result: "i32",
  },
});

export class SharedRegion {
  #handle: Deno.PointerValue;

  private constructor(handle: Deno.PointerValue) {
    this.#handle = handle;
  }

  static create(name: string, capacity: number): SharedRegion {
    if (name.includes("\0")) throw new Error("name contains NUL");
    if (!Number.isSafeInteger(capacity) || capacity <= 0) {
      throw new Error("invalid capacity");
    }
    const buf = new BigUint64Array(1);
    const nameBuf = new TextEncoder().encode(name + "\0");
    const code = lib.symbols.zinc_create(
      nameBuf,
      capacity,
      Deno.UnsafePointer.of(buf),
    );
    if (code !== 0) throw new Error(`zinc_create failed: ${code}`);
    return new SharedRegion(Deno.UnsafePointer.create(buf[0])!);
  }

  static open(name: string): SharedRegion {
    if (name.includes("\0")) throw new Error("name contains NUL");
    const buf = new BigUint64Array(1);
    const nameBuf = new TextEncoder().encode(name + "\0");
    const code = lib.symbols.zinc_open(
      nameBuf,
      Deno.UnsafePointer.of(buf),
    );
    if (code !== 0) throw new Error(`zinc_open failed: ${code}`);
    return new SharedRegion(Deno.UnsafePointer.create(buf[0])!);
  }

  buffer(): Uint8Array {
    const dataPtr = lib.symbols.zinc_ptr(this.#liveHandle());
    const len = Number(lib.symbols.zinc_capacity(this.#liveHandle()));
    if (dataPtr === null || len === 0) {
      throw new Error("Invalid region pointer");
    }
    return new Uint8Array(
      Deno.UnsafePointerView.getArrayBuffer(dataPtr!, len),
    );
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
