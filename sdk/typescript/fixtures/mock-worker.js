#!/usr/bin/env node
const readline = require('readline');

// 1. Emit handshake
console.log(JSON.stringify({ ready: true }));

const rl = readline.createInterface({ input: process.stdin });

rl.on('line', (line) => {
  try {
    const req = JSON.parse(line);
    const id = req.id || null;
    const cmd = req.cmd;

    if (cmd === 'shutdown') {
      console.log(JSON.stringify({ id, version: '1.0', ok: true, result: 'shutting down' }));
      process.exit(0);
    } else if (cmd === 'ping') {
      console.log(JSON.stringify({ id, version: '1.0', ok: true, result: 'pong' }));
    } else if (cmd === 'health') {
      console.log(JSON.stringify({ id, version: '1.0', ok: true, result: { status: 'healthy', uptime_secs: 1 } }));
    } else if (cmd === 'session_create') {
      console.log(JSON.stringify({ id, version: '1.0', ok: true, result: { session_id: req.session_id } }));
    } else if (cmd === 'navigate') {
      console.log(JSON.stringify({
        id,
        version: '1.0',
        ok: true,
        result: { url: req.url, title: 'Mock Page', status: 200 }
      }));
    } else if (cmd === 'evaluate') {
      console.log(JSON.stringify({ id, version: '1.0', ok: true, result: 42 }));
    } else {
      console.log(JSON.stringify({ id, version: '1.0', ok: true, result: 'ok' }));
    }
  } catch (err) {
    console.log(JSON.stringify({ ok: false, error: { code: 'INVALID', message: String(err) } }));
  }
});
