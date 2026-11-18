const { test } = require('node:test');
const assert = require('node:assert');
const path = require('node:path');
const { ObscuraClient, resolveWorkerBinary } = require('../dist/index.js');

const MOCK_WORKER = path.resolve(__dirname, '..', 'fixtures', 'mock-worker.js');

test('TypeScript SDK - binary resolution fallback', () => {
  const resolved = resolveWorkerBinary();
  assert.ok(typeof resolved === 'string');
});

test('TypeScript SDK - client lifecycle with mock worker', async () => {
  const client = new ObscuraClient(MOCK_WORKER);
  await client.start();

  // Test ping
  const ping = await client.ping();
  assert.strictEqual(ping.ok, true);
  assert.strictEqual(ping.result, 'pong');

  // Test health
  const health = await client.health();
  assert.strictEqual(health.ok, true);

  // Test session create
  const session = await client.createSession('test-s1');
  assert.strictEqual(session.ok, true);

  // Test navigate
  const nav = await client.navigate('https://example.com', 'test-s1');
  assert.strictEqual(nav.success, true);
  assert.strictEqual(nav.action, 'navigate');
  assert.strictEqual(nav.payload.status, 200);

  // Test evaluate
  const ev = await client.evaluate('6 * 7', 'test-s1');
  assert.strictEqual(ev.success, true);
  assert.strictEqual(ev.action, 'evaluate');
  assert.strictEqual(ev.payload, 42);

  // Test concurrent requests and ID correlation
  const [p1, p2, p3] = await Promise.all([
    client.ping(),
    client.ping(),
    client.ping(),
  ]);
  assert.strictEqual(p1.result, 'pong');
  assert.strictEqual(p2.result, 'pong');
  assert.strictEqual(p3.result, 'pong');

  // Test close
  await client.close();
});
