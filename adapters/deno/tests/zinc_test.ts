import { assert, assertEquals, assertThrows } from "@std/assert";
import { SharedRegion } from "../src/mod.ts";

const name = `deno_test_${Date.now().toString(36)}`;

Deno.test("invalid timeouts are rejected before FFI coercion", () => {
  const r = SharedRegion.create(name, 16384);
  try {
    for (const value of [-1, NaN, Infinity, 1.5, 2 ** 32]) {
      assertThrows(() => r.wait(value), Error, "timeout");
    }
  } finally {
    r.close();
  }
});

Deno.test("create and buffer", () => {
  const r = SharedRegion.create(name, 16384);
  const buf = r.buffer();
  assertEquals(buf.length, 16384);
  buf[0] = 0xAB;
  assertEquals(buf[0], 0xAB);
  r.close();
});

Deno.test("open and read", () => {
  const n = name + "_ro";
  const owner = SharedRegion.create(n, 16384);
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

Deno.test("notify and wait", async () => {
  const n = name + "_nw";
  const region = SharedRegion.create(n, 16384);

  const worker = new Worker(
    new URL("./notify-worker.ts", import.meta.url).href,
    { type: "module" },
  );
  worker.postMessage(n);

  const signaled = region.wait(5000);
  assert(signaled, "wait() should return true");
  assertEquals(region.buffer()[0], 99);
  worker.terminate();
  region.close();
});

Deno.test("open nonexistent region throws", () => {
  assertThrows(() => SharedRegion.open("__deno_nonexistent_xyz__"));
});

Deno.test("Symbol.dispose cleans up", () => {
  const r = SharedRegion.create(name + "_disp", 16384);
  r[Symbol.dispose]();
  // Should not crash — handle is nulled
});

Deno.test("close is idempotent and closed operations fail", () => {
  const r = SharedRegion.create(name + "_close", 16384);
  assertEquals(r.tryWait(), false);
  r.notify();
  assertEquals(r.tryWait(), true);
  assertEquals(r.wait(0), false);
  r.close();
  r.close();
  assertThrows(() => r.buffer(), Error, "closed");
  assertThrows(() => r.notify(), Error, "closed");
  assertThrows(() => r.wait(), Error, "closed");
  assertThrows(() => r.tryWait(), Error, "closed");
});
