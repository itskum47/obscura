"""Unit tests for the Obscura Next Python SDK."""

import asyncio
import json
import os
import sys
import unittest
from unittest.mock import AsyncMock, MagicMock, patch

# Ensure sdk/python is on sys.path
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

import obscura_next
from obscura_next.client import AsyncObscuraClient, ObscuraClient, _resolve_worker_binary
from obscura_next.models import ActionResult, AgentAction, SemanticNode, SessionConfig


class TestPackageImports(unittest.TestCase):
    """Verify package importability and metadata."""

    def test_import_obscura_next(self):
        self.assertIsNotNone(obscura_next.__file__)
        self.assertTrue(hasattr(obscura_next, "AsyncObscuraClient"))
        self.assertTrue(hasattr(obscura_next, "ObscuraClient"))
        self.assertTrue(hasattr(obscura_next, "SessionConfig"))
        self.assertTrue(hasattr(obscura_next, "ActionResult"))

    def test_models_instantiation(self):
        cfg = SessionConfig(stealth=True, timeout_ms=10000)
        self.assertTrue(cfg.stealth)
        self.assertEqual(cfg.timeout_ms, 10000)

        act = AgentAction(action="click", selector="#submit")
        self.assertEqual(act.action, "click")
        self.assertEqual(act.selector, "#submit")

        res = ActionResult(action="click", success=True, payload={"clicked": True}, elapsed_ms=12.4)
        self.assertTrue(res.success)
        self.assertEqual(res.elapsed_ms, 12.4)

        node = SemanticNode(id="@e1", tag="button", text="Submit", selector="#submit", role="button", is_interactive=True)
        self.assertTrue(node.is_interactive)
        self.assertEqual(node.tag, "button")

    def test_binary_resolution(self):
        resolved = _resolve_worker_binary()
        self.assertIsInstance(resolved, str)
        self.assertTrue(len(resolved) > 0)


class MockWorkerProcess:
    """Simulates the obscura-worker stdout/stdin pipe for unit tests."""

    def __init__(self, handshake=b'{"ready":true}\n'):
        self.stdin = MagicMock()
        self.stdin.write = MagicMock()
        self.stdin.drain = AsyncMock()
        self.stdin.close = MagicMock()

        self.stdout = MagicMock()
        self.response_queue = asyncio.Queue()
        if handshake:
            self.response_queue.put_nowait(handshake)

        async def readline():
            return await self.response_queue.get()

        self.stdout.readline = readline

        self.wait = AsyncMock(return_value=0)
        self.kill = MagicMock()

    def queue_response(self, resp_dict: dict):
        line = json.dumps(resp_dict).encode("utf-8") + b"\n"
        self.response_queue.put_nowait(line)

    def queue_raw(self, raw_bytes: bytes):
        self.response_queue.put_nowait(raw_bytes)


class TestAsyncObscuraClient(unittest.IsolatedAsyncioTestCase):
    """Protocol unit tests using simulated worker IO."""

    async def test_worker_startup_and_handshake(self):
        mock_proc = MockWorkerProcess()
        with patch("asyncio.create_subprocess_exec", AsyncMock(return_value=mock_proc)) as mock_exec, \
             patch("os.path.isfile", return_value=True), \
             patch("os.access", return_value=True):
            client = AsyncObscuraClient(binary_path="/dummy/obscura-worker")
            await client.start()
            mock_exec.assert_called_once_with(
                "/dummy/obscura-worker",
                stdin=asyncio.subprocess.PIPE,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
            )
            self.assertIsNotNone(client._proc)

    async def test_startup_handshake_failure(self):
        mock_proc = MockWorkerProcess(handshake=b'{"ready":false}\n')
        with patch("asyncio.create_subprocess_exec", AsyncMock(return_value=mock_proc)):
            client = AsyncObscuraClient(binary_path="/dummy/obscura-worker")
            with self.assertRaises(RuntimeError) as ctx:
                await client.start()
            self.assertIn("handshake failed", str(ctx.exception))

    async def test_navigation(self):
        mock_proc = MockWorkerProcess()
        with patch("asyncio.create_subprocess_exec", AsyncMock(return_value=mock_proc)):
            client = AsyncObscuraClient(binary_path="/dummy/obscura-worker", default_session_id="s1")
            await client.start()

            # Queue response with matching ID
            mock_proc.queue_response({
                "id": "py-1",
                "version": "1.0",
                "ok": True,
                "result": {"url": "https://example.com", "title": "Example", "status": 200},
            })

            res = await client.navigate("https://example.com")
            self.assertTrue(res.success)
            self.assertEqual(res.action, "navigate")
            self.assertEqual(res.payload["status"], 200)

    async def test_evaluation(self):
        mock_proc = MockWorkerProcess()
        with patch("asyncio.create_subprocess_exec", AsyncMock(return_value=mock_proc)):
            client = AsyncObscuraClient(binary_path="/dummy/obscura-worker", default_session_id="s1")
            await client.start()

            mock_proc.queue_response({
                "id": "py-1",
                "version": "1.0",
                "ok": True,
                "result": 42,
            })

            res = await client.evaluate("6 * 7")
            self.assertTrue(res.success)
            self.assertEqual(res.action, "evaluate")
            self.assertEqual(res.payload, 42)

    async def test_invalid_response_id_mismatch(self):
        mock_proc = MockWorkerProcess()
        with patch("asyncio.create_subprocess_exec", AsyncMock(return_value=mock_proc)):
            client = AsyncObscuraClient(binary_path="/dummy/obscura-worker")
            await client.start()

            mock_proc.queue_response({
                "id": "wrong-id",
                "version": "1.0",
                "ok": True,
                "result": "pong",
            })

            with self.assertRaises(RuntimeError) as ctx:
                await client.ping()
            self.assertIn("Response id mismatch", str(ctx.exception))

    async def test_worker_termination(self):
        mock_proc = MockWorkerProcess()
        with patch("asyncio.create_subprocess_exec", AsyncMock(return_value=mock_proc)):
            client = AsyncObscuraClient(binary_path="/dummy/obscura-worker")
            await client.start()

            mock_proc.queue_response({
                "id": "py-1",
                "version": "1.0",
                "ok": True,
                "result": "shutting down",
            })

            await client.close()
            mock_proc.wait.assert_awaited()
            self.assertIsNone(client._proc)

    async def test_concurrent_calls_serialization(self):
        mock_proc = MockWorkerProcess()
        with patch("asyncio.create_subprocess_exec", AsyncMock(return_value=mock_proc)):
            client = AsyncObscuraClient(binary_path="/dummy/obscura-worker")
            await client.start()

            # Set up responses for sequential retrieval
            mock_proc.queue_response({"id": "py-1", "version": "1.0", "ok": True, "result": "pong1"})
            mock_proc.queue_response({"id": "py-2", "version": "1.0", "ok": True, "result": "pong2"})

            # Issue concurrent ping calls
            r1, r2 = await asyncio.gather(client.ping(), client.ping())
            self.assertEqual(r1["result"], "pong1")
            self.assertEqual(r2["result"], "pong2")


if __name__ == "__main__":
    unittest.main()
