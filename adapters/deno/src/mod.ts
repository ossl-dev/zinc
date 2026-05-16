import { join } from "@std/path";

const suffix = {
  darwin: "dylib",
  linux: "so",
  windows: "dll",
}[Deno.build.os];

const libPath = join(
  import.meta.dirname!,
  `../../core/target/release/libzinc_core.${suffix}`,
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
    const dataPtr = lib.symbols.zinc_ptr(this.#handle);
    const len = lib.symbols.zinc_capacity(this.#handle);
    if (dataPtr === null || len === 0) {
      throw new Error("Invalid region pointer");
    }
    return new Uint8Array(
      Deno.UnsafePointerView.getArrayBuffer(dataPtr!, len),
    );
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
