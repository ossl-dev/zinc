import { describe, expect, test } from "bun:test";
import { SharedRegion } from "../src/index.ts";

describe("SharedRegion (Bun)", () => {
  const name = `bun_test_${Date.now().toString(36)}`;

  test("create and buffer", () => {
    const r = SharedRegion.create(name, 16384);
    const buf = r.buffer();
    expect(buf.length).toBe(16384);
    buf[0] = 0xAB;
    expect(buf[0]).toBe(0xAB);
    r.close();
  });

  test("open and read", () => {
    const owner = SharedRegion.create(name + "_ro", 16384);
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
    const n = name + "_nw";
    const region = SharedRegion.create(n, 16384);

    const worker = new Worker(
      new URL("./notify-worker.ts", import.meta.url).href,
      { type: "module" },
    );
    worker.postMessage(n);

    const signaled = region.wait(5000);
    expect(signaled).toBe(true);
    expect(region.buffer()[0]).toBe(99);
    worker.terminate();
    region.close();
  });

  test("open nonexistent region throws", () => {
    expect(() => SharedRegion.open("__bun_nonexistent_xyz__")).toThrow();
  });

  test("Symbol.dispose cleans up", () => {
    const r = SharedRegion.create(name + "_disp", 16384);
    r[Symbol.dispose]();
    // Should not crash — handle is nulled
  });

  test("close is idempotent and closed operations fail", () => {
    const r = SharedRegion.create(name + "_close", 16384);
    expect(r.tryWait()).toBe(false);
    r.notify();
    expect(r.tryWait()).toBe(true);
    expect(r.wait(0)).toBe(false);
    r.close();
    r.close();
    expect(() => r.buffer()).toThrow("closed");
    expect(() => r.notify()).toThrow("closed");
    expect(() => r.wait()).toThrow("closed");
    expect(() => r.tryWait()).toThrow("closed");
  });
});
