const assert = require('node:assert/strict');
const { test } = require('node:test');
const { ZincRegion } = require(process.env.ZINC_NODE_ADDON || '../index.js');

const capacity = 16384;
let id = 0;
const name = () => `node_${process.pid}_${++id}`;

test('invalid numbers are rejected before native coercion', () => {
  const region = ZincRegion.create(name(), capacity);
  for (const value of [-1, NaN, Infinity, 1.5, 2 ** 32]) {
    assert.throws(() => ZincRegion.create(name(), value), /capacity/);
    assert.throws(() => region.wait(value), /timeout/);
  }
});

async function collectUntil(predicate) {
  for (let i = 0; i < 100; i++) {
    global.gc();
    await new Promise(setImmediate);
    if (predicate()) return;
  }
  assert.fail('garbage collection did not finish');
}

test('shared bytes and pending notifications', () => {
  const n = name();
  const owner = ZincRegion.create(n, capacity);
  const reader = ZincRegion.open(n);
  const buffer = owner.asBuffer();
  buffer[0] = 42;
  assert.equal(reader.asBuffer()[0], 42);
  assert.equal(reader.tryWait(), false);
  owner.notify();
  assert.equal(reader.tryWait(), true);
  assert.equal(reader.wait(0), false);
  assert.throws(() => ZincRegion.create(n, capacity));
});

test('buffer keeps its mapping after the region is collected', async () => {
  const n = name();
  let collected = false;
  const registry = new FinalizationRegistry(() => { collected = true; });
  let buffer = (() => {
    const region = ZincRegion.create(n, capacity);
    registry.register(region, 0);
    return region.asBuffer();
  })();
  await collectUntil(() => collected);
  buffer[0] = 99;
  assert.equal(buffer[0], 99);
  assert.doesNotThrow(() => ZincRegion.open(n));
  buffer = null;
  await collectUntil(() => {
    try {
      ZincRegion.open(n);
      return false;
    } catch {
      return true;
    }
  });
});
