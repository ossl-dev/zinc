import { assertEquals, assert, assertThrows } from "@std/assert";
import { SharedRegion } from "../src/mod.ts";

// Skip if core library not built
let skip = false;
try {
  const r = SharedRegion.create("__deno_skip_test__", 4096);
  r.close();
} catch {
  skip = true;
}

const name = `deno_test_${Date.now()}`;

Deno.test("create and buffer", { ignore: skip }, () => {
  const r = SharedRegion.create(name, 4096);
  const buf = r.buffer();
  assertEquals(buf.length, 4096);
  buf[0] = 0xAB;
  assertEquals(buf[0], 0xAB);
  r.close();
});

Deno.test("open and read", { ignore: skip }, () => {
  const n = name + "_ro";
  const owner = SharedRegion.create(n, 4096);
  const obuf = owner.buffer();
  obuf[0] = 0x42;
  obuf[1] = 0x58;

  const reader = SharedRegion.open(n);
  const rbuf = reader.buffer();
  assertEquals(rbuf[0], 0x42);
  assertEquals(rbuf[1], 0x58);
  reader.close();
  owner.close();
});

Deno.test("notify and wait", { ignore: skip }, async () => {
  const n = name + "_nw";
  const region = SharedRegion.create(n, 4096);

  const signal = new Promise<void>((resolve) => {
    setTimeout(() => {
      const r2 = SharedRegion.open(n);
      r2.buffer()[0] = 99;
      r2.notify();
      r2.close();
      resolve();
    }, 10);
  });

  const signaled = region.wait(5000);
  assert(signaled, "wait() should return true");
  assertEquals(region.buffer()[0], 99);
  await signal;
  region.close();
});

Deno.test("open nonexistent region throws", { ignore: skip }, () => {
  assertThrows(() => SharedRegion.open("__deno_nonexistent_xyz__"));
});

Deno.test("Symbol.dispose cleans up", { ignore: skip }, () => {
  const r = SharedRegion.create(name + "_disp", 4096);
  r[Symbol.dispose]();
  // Should not crash — handle is nulled
});
