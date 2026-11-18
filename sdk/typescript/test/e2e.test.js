const { test } = require('node:test');
const assert = require('node:assert');
const path = require('node:path');
const fs = require('node:fs');
const { ObscuraClient } = require('../dist/index.js');

function findBinary() {
  const root = path.resolve(__dirname, '..', '..', '..');
  const candidates = [
    path.join(root, 'target', 'debug', 'obscura-worker'),
    path.join(root, 'target', 'release', 'obscura-worker'),
    path.join(root, 'target', 'debug', 'obscura-runtime-worker'),
    path.join(root, 'target', 'release', 'obscura-runtime-worker'),
  ];
  for (const c of candidates) {
    if (fs.existsSync(c)) return c;
  }
  return null;
}

const BIN = findBinary();

test('TS SDK E2E', { skip: !BIN }, async () => {
  const client = new ObscuraClient(BIN);
  await client.start();
  const ping = await client.ping();
  assert.strictEqual(ping.ok, true);

  const health = await client.health();
  assert.strictEqual(health.ok, true);

  const sid = 'ts-test-session-1';
  const created = await client.createSession(sid);
  assert.strictEqual(created.ok, true);

  const nav = await client.navigate('data:text/html,<h1>Hello TS SDK</h1>', sid);
  assert.strictEqual(nav.success, true);

  const ev = await client.evaluate('1 + 2', sid);
  assert.strictEqual(ev.success, true);

  // Verify concurrent requests & request correlation against real worker process
  const concurrent = await Promise.all([
    client.ping(),
    client.health(),
    client.ping(),
    client.health(),
  ]);
  assert.strictEqual(concurrent.length, 4);
  for (const res of concurrent) {
    assert.strictEqual(res.ok, true);
  }

  const destroyed = await client.destroySession(sid);
  assert.strictEqual(destroyed.ok, true);

  await client.close();
});
