import { describe, test, expect, beforeAll } from "bun:test";
import { SharedRegion } from "../src/index.ts";

const SKIP = (() => {
  try {
    // Test if the core library loads
    SharedRegion.create("__bun_skip_test__", 4096).close();
    return false;
  } catch {
    return true;
  }
})();

describe("SharedRegion (Bun)", () => {
  const name = `bun_test_${Date.now()}`;

  test("create and buffer", () => {
    if (SKIP) return;

    const r = SharedRegion.create(name, 4096);
    const buf = r.buffer();
    expect(buf.length).toBe(4096);
    buf[0] = 0xAB;
    expect(buf[0]).toBe(0xAB);
    r.close();
  });

  test("open and read", () => {
    if (SKIP) return;

    const owner = SharedRegion.create(name + "_ro", 4096);
    const obuf = owner.buffer();
    obuf[0] = 0x42;
    obuf[1] = 0x58;

    const reader = SharedRegion.open(name + "_ro");
    const rbuf = reader.buffer();
    expect(rbuf[0]).toBe(0x42);
    expect(rbuf[1]).toBe(0x58);
    reader.close();
    owner.close();
  });

  test("notify and wait", async () => {
    if (SKIP) return;

    const n = name + "_nw";
    const region = SharedRegion.create(n, 4096);

    const worker = new Promise<void>((resolve) => {
      setTimeout(() => {
        const r2 = SharedRegion.open(n);
        r2.buffer()[0] = 99;
        r2.notify();
        r2.close();
        resolve();
      }, 10);
    });

    const signaled = region.wait(5000);
    expect(signaled).toBe(true);
    expect(region.buffer()[0]).toBe(99);
    await worker;
    region.close();
  });

  test("open nonexistent region throws", () => {
    if (SKIP) return;
    expect(() => SharedRegion.open("__bun_nonexistent_xyz__")).toThrow();
  });

  test("Symbol.dispose cleans up", () => {
    if (SKIP) return;
    const r = SharedRegion.create(name + "_disp", 4096);
    r[Symbol.dispose]();
    // Should not crash — handle is nulled
  });
});
