const { test } = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const fs = require('node:fs');
const { ObscuraClient } = require('../dist/index.js');

test('TypeScript SDK - handshake failure cleans up and kills subprocess', async () => {
  // Create a dummy script that does NOT output ready:true
  const dummyScript = path.join(__dirname, 'dummy_hang.sh');
  fs.writeFileSync(dummyScript, '#!/bin/sh\nexec sleep 30\n', { mode: 0o755 });

  const client = new ObscuraClient(dummyScript);
  let error;
  try {
    // 200ms handshake timeout
    await client.start(200);
  } catch (err) {
    error = err;
  } finally {
    fs.unlinkSync(dummyScript);
  }

  assert.ok(error, 'Expected start() to fail on non-ready handshake');
  assert.match(error.message, /handshake timed out/);
});

test('TypeScript SDK - premature worker exit during execution rejects pending commands', async () => {
  // Create a worker script that emits ready:true then exits on first command
  const dummyScript = path.join(__dirname, 'dummy_exit.sh');
  fs.writeFileSync(dummyScript, '#!/bin/sh\necho \'{"ready":true}\'\nread line\nexit 1\n', { mode: 0o755 });

  const client = new ObscuraClient(dummyScript);
  await client.start();

  let error;
  try {
    await client.ping();
  } catch (err) {
    error = err;
  } finally {
    try {
      await client.close();
    } catch {}
    fs.unlinkSync(dummyScript);
  }

  assert.ok(error, 'Expected command to reject when worker terminates');
  assert.match(error.message, /terminated unexpectedly/);
});
