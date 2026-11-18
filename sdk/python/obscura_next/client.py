"""
Obscura Next Python SDK — Async and sync client for the obscura-worker binary.

Communicates with the worker via framed JSON lines over stdin/stdout.
Protocol:
  - On launch the worker emits {"ready":true} on stdout.
  - Requests:  {"id":"…","version":"1.0","cmd":"…", …}
  - Responses: {"id":"…","version":"1.0","ok":true/false,"result":…,"error":…}
"""

import asyncio
import json
import os
import shutil
import time
import uuid
from typing import Any, Dict, Optional

from .models import ActionResult, SessionConfig


# ---------------------------------------------------------------------------
# Binary resolution
# ---------------------------------------------------------------------------

def _resolve_worker_binary(binary_path: Optional[str] = None) -> str:
    """Resolve the *obscura-worker* binary in priority order.

    1. Explicit *binary_path* argument.
    2. ``OBSCURA_WORKER_BIN`` environment variable.
    3. Well-known build directories relative to cwd.
    4. ``PATH`` lookup.
    5. Fallback (returns the argument or ``"obscura-worker"``).
    """
    # 1. Explicit argument
    if binary_path and os.path.isfile(binary_path) and os.access(binary_path, os.X_OK):
        return binary_path

    # 2. Environment variable
    env_bin = os.environ.get("OBSCURA_WORKER_BIN")
    if env_bin and os.path.isfile(env_bin) and os.access(env_bin, os.X_OK):
        return env_bin

    # 3. Build directories relative to cwd
    candidates = [
        "target/debug/obscura-worker",
        "target/release/obscura-worker",
        "../target/debug/obscura-worker",
        "../target/release/obscura-worker",
    ]
    for candidate in candidates:
        if os.path.isfile(candidate) and os.access(candidate, os.X_OK):
            return os.path.abspath(candidate)

    # 4. PATH lookup
    found = shutil.which("obscura-worker")
    if found:
        return found

    # 5. Fallback
    return binary_path or "obscura-worker"


# ---------------------------------------------------------------------------
# Async client
# ---------------------------------------------------------------------------

class AsyncObscuraClient:
    """Async client for the Obscura Next worker process.

    Usage::

        client = AsyncObscuraClient()
        await client.start()
        await client.create_session()
        result = await client.navigate("https://example.com")
        await client.close()
    """

    def __init__(
        self,
        binary_path: Optional[str] = None,
        default_session_id: Optional[str] = None,
    ):
        self.binary_path = _resolve_worker_binary(binary_path)
        self.default_session_id = default_session_id or f"py-session-{uuid.uuid4().hex[:12]}"
        self._proc: Optional[asyncio.subprocess.Process] = None
        self._lock = asyncio.Lock()
        self._req_counter = 0

    # -- helpers -------------------------------------------------------------

    def _next_id(self) -> str:
        self._req_counter += 1
        return f"py-{self._req_counter}"

    def _sid(self, session_id: Optional[str]) -> str:
        return session_id if session_id is not None else self.default_session_id

    # -- lifecycle -----------------------------------------------------------

    async def start(self, handshake_timeout: float = 10.0) -> None:
        """Launch the worker process and consume the ``{"ready":true}`` handshake."""
        self._proc = await asyncio.create_subprocess_exec(
            self.binary_path,
            stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )

        try:
            assert self._proc.stdout is not None
            handshake_line = await asyncio.wait_for(
                self._proc.stdout.readline(),
                timeout=handshake_timeout,
            )
            if not handshake_line:
                raise RuntimeError("Worker process exited before sending handshake")
            handshake = json.loads(handshake_line.decode("utf-8"))
            if not handshake.get("ready"):
                raise RuntimeError(f"Worker handshake failed: {handshake}")
        except Exception:
            if self._proc:
                try:
                    self._proc.kill()
                    await self._proc.wait()
                except Exception:
                    pass
                self._proc = None
            raise

    async def close(self) -> None:
        """Send ``shutdown`` command, wait for exit, force-kill if needed."""
        if not self._proc:
            return
        try:
            await self.send_command({"cmd": "shutdown"}, timeout=3.0)
        except Exception:
            pass

        if self._proc.stdin:
            try:
                self._proc.stdin.close()
            except Exception:
                pass

        try:
            await asyncio.wait_for(self._proc.wait(), timeout=5.0)
        except asyncio.TimeoutError:
            self._proc.kill()
            await self._proc.wait()
        self._proc = None

    # -- low-level protocol --------------------------------------------------

    async def send_command(
        self, cmd: Dict[str, Any], timeout: float = 30.0
    ) -> Dict[str, Any]:
        """Serialize *cmd* as a JSON line, send it, read the response.

        Automatically assigns ``id`` and ``version`` if not present.
        Verifies that the response ``id`` matches the request.
        Uses an ``asyncio.Lock`` so concurrent callers are serialized.
        """
        if not self._proc or not self._proc.stdin or not self._proc.stdout:
            raise RuntimeError("Client not connected to worker process")
        if getattr(self._proc, "returncode", None) is not None:
            raise RuntimeError(f"Worker process has terminated (code {self._proc.returncode})")

        if "id" not in cmd:
            cmd["id"] = self._next_id()
        if "version" not in cmd:
            cmd["version"] = "1.0"

        req_id = cmd["id"]

        async with self._lock:
            line = json.dumps(cmd) + "\n"
            self._proc.stdin.write(line.encode("utf-8"))
            await self._proc.stdin.drain()

            try:
                resp_line = await asyncio.wait_for(
                    self._proc.stdout.readline(),
                    timeout=timeout,
                )
            except asyncio.TimeoutError:
                raise TimeoutError(f"Command {cmd.get('cmd')} (id {req_id}) timed out after {timeout}s")

            if not resp_line:
                raise RuntimeError("Worker process terminated unexpectedly")

            try:
                resp = json.loads(resp_line.decode("utf-8"))
            except json.JSONDecodeError as e:
                raise RuntimeError(f"Malformed JSON response from worker: {resp_line!r}") from e

        if resp.get("id") != req_id:
            raise RuntimeError(
                f"Response id mismatch: expected {req_id!r}, got {resp.get('id')!r}"
            )

        return resp

    # -- session management --------------------------------------------------

    async def create_session(
        self,
        session_id: Optional[str] = None,
        config: Optional[SessionConfig] = None,
    ) -> Dict[str, Any]:
        """Create a new browser session."""
        sid = self._sid(session_id)
        cmd: Dict[str, Any] = {
            "cmd": "session_create",
            "session_id": sid,
        }
        if config:
            cfg: Dict[str, Any] = {
                "stealth": config.stealth,
                "timeout_ms": config.timeout_ms,
            }
            if config.proxy_url:
                cfg["proxy_url"] = config.proxy_url
            if config.user_agent:
                cfg["user_agent"] = config.user_agent
            cmd["config"] = cfg
        return await self.send_command(cmd)

    async def destroy_session(self, session_id: Optional[str] = None) -> Dict[str, Any]:
        """Destroy a browser session."""
        return await self.send_command({
            "cmd": "session_destroy",
            "session_id": self._sid(session_id),
        })

    # -- operations ----------------------------------------------------------

    async def ping(self) -> Dict[str, Any]:
        """Send a ``ping`` command (session-less)."""
        return await self.send_command({"cmd": "ping"})

    async def health(self) -> Dict[str, Any]:
        """Send a ``health`` check command (session-less)."""
        return await self.send_command({"cmd": "health"})

    async def navigate(
        self,
        url: str,
        session_id: Optional[str] = None,
        wait_until: str = "load",
        timeout_ms: Optional[int] = None,
    ) -> ActionResult:
        """Navigate the session's page to *url*."""
        sid = self._sid(session_id)
        t0 = time.time()
        cmd: Dict[str, Any] = {
            "cmd": "navigate",
            "session_id": sid,
            "url": url,
            "wait_until": wait_until,
        }
        if timeout_ms is not None:
            cmd["timeout_ms"] = timeout_ms
        resp = await self.send_command(cmd)
        elapsed = (time.time() - t0) * 1000.0
        return ActionResult(
            action="navigate",
            success=resp.get("ok", False),
            payload=resp.get("result"),
            elapsed_ms=elapsed,
            error=resp.get("error"),
        )

    async def evaluate(
        self,
        expression: str,
        session_id: Optional[str] = None,
        await_promise: bool = True,
        timeout_ms: Optional[int] = None,
    ) -> ActionResult:
        """Evaluate a JavaScript expression in the session."""
        sid = self._sid(session_id)
        t0 = time.time()
        cmd: Dict[str, Any] = {
            "cmd": "evaluate",
            "session_id": sid,
            "expression": expression,
            "await_promise": await_promise,
        }
        if timeout_ms is not None:
            cmd["timeout_ms"] = timeout_ms
        resp = await self.send_command(cmd)
        elapsed = (time.time() - t0) * 1000.0
        return ActionResult(
            action="evaluate",
            success=resp.get("ok", False),
            payload=resp.get("result"),
            elapsed_ms=elapsed,
            error=resp.get("error"),
        )

    async def snapshot(self, session_id: Optional[str] = None) -> Dict[str, Any]:
        """Get the accessibility snapshot of the current page."""
        return await self.send_command({
            "cmd": "snapshot",
            "session_id": self._sid(session_id),
        })

    async def click(self, selector: str, session_id: Optional[str] = None) -> ActionResult:
        """Click an element matching *selector*."""
        sid = self._sid(session_id)
        t0 = time.time()
        resp = await self.send_command({
            "cmd": "click",
            "session_id": sid,
            "selector": selector,
        })
        elapsed = (time.time() - t0) * 1000.0
        return ActionResult(
            action="click",
            success=resp.get("ok", False),
            payload=resp.get("result"),
            elapsed_ms=elapsed,
            error=resp.get("error"),
        )

    async def fill(
        self, selector: str, value: str, session_id: Optional[str] = None
    ) -> ActionResult:
        """Fill an input element matching *selector* with *value*."""
        sid = self._sid(session_id)
        t0 = time.time()
        resp = await self.send_command({
            "cmd": "fill",
            "session_id": sid,
            "selector": selector,
            "value": value,
        })
        elapsed = (time.time() - t0) * 1000.0
        return ActionResult(
            action="fill",
            success=resp.get("ok", False),
            payload=resp.get("result"),
            elapsed_ms=elapsed,
            error=resp.get("error"),
        )

    async def type_text(
        self, selector: str, text: str, session_id: Optional[str] = None
    ) -> ActionResult:
        """Type *text* into an element matching *selector*."""
        sid = self._sid(session_id)
        t0 = time.time()
        resp = await self.send_command({
            "cmd": "type",
            "session_id": sid,
            "selector": selector,
            "text": text,
        })
        elapsed = (time.time() - t0) * 1000.0
        return ActionResult(
            action="type",
            success=resp.get("ok", False),
            payload=resp.get("result"),
            elapsed_ms=elapsed,
            error=resp.get("error"),
        )

    async def screenshot(self, session_id: Optional[str] = None) -> Dict[str, Any]:
        """Take a screenshot of the current page."""
        return await self.send_command({
            "cmd": "screenshot",
            "session_id": self._sid(session_id),
        })

    async def title(self, session_id: Optional[str] = None) -> Dict[str, Any]:
        """Get the page title."""
        return await self.send_command({
            "cmd": "title",
            "session_id": self._sid(session_id),
        })

    async def dump(
        self, format: str = "html", session_id: Optional[str] = None
    ) -> Dict[str, Any]:
        """Dump the page content in the given *format*."""
        return await self.send_command({
            "cmd": "dump",
            "session_id": self._sid(session_id),
            "format": format,
        })


# ---------------------------------------------------------------------------
# Sync wrapper
# ---------------------------------------------------------------------------

class ObscuraClient:
    """Synchronous convenience wrapper around :class:`AsyncObscuraClient`.

    Usage::

        with ObscuraClient() as client:
            client.create_session()
            result = client.navigate("https://example.com")
    """

    def __init__(
        self,
        binary_path: Optional[str] = None,
        session_id: Optional[str] = None,
    ):
        self._async_client = AsyncObscuraClient(binary_path, session_id)

    def __enter__(self):
        asyncio.run(self._async_client.start())
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        asyncio.run(self._async_client.close())

    # -- forwarded methods ---------------------------------------------------

    def ping(self) -> Dict[str, Any]:
        return asyncio.run(self._async_client.ping())

    def health(self) -> Dict[str, Any]:
        return asyncio.run(self._async_client.health())

    def create_session(
        self, session_id: Optional[str] = None, config: Optional[SessionConfig] = None
    ) -> Dict[str, Any]:
        return asyncio.run(self._async_client.create_session(session_id, config))

    def destroy_session(self, session_id: Optional[str] = None) -> Dict[str, Any]:
        return asyncio.run(self._async_client.destroy_session(session_id))

    def navigate(
        self, url: str, session_id: Optional[str] = None, wait_until: str = "load", timeout_ms: Optional[int] = None
    ) -> ActionResult:
        return asyncio.run(self._async_client.navigate(url, session_id, wait_until, timeout_ms))

    def evaluate(
        self, expression: str, session_id: Optional[str] = None, await_promise: bool = True, timeout_ms: Optional[int] = None
    ) -> ActionResult:
        return asyncio.run(self._async_client.evaluate(expression, session_id, await_promise, timeout_ms))

    def snapshot(self, session_id: Optional[str] = None) -> Dict[str, Any]:
        return asyncio.run(self._async_client.snapshot(session_id))

    def click(self, selector: str, session_id: Optional[str] = None) -> ActionResult:
        return asyncio.run(self._async_client.click(selector, session_id))

    def fill(self, selector: str, value: str, session_id: Optional[str] = None) -> ActionResult:
        return asyncio.run(self._async_client.fill(selector, value, session_id))

    def type_text(self, selector: str, text: str, session_id: Optional[str] = None) -> ActionResult:
        return asyncio.run(self._async_client.type_text(selector, text, session_id))

    def screenshot(self, session_id: Optional[str] = None) -> Dict[str, Any]:
        return asyncio.run(self._async_client.screenshot(session_id))

    def title(self, session_id: Optional[str] = None) -> Dict[str, Any]:
        return asyncio.run(self._async_client.title(session_id))

    def dump(self, format: str = "html", session_id: Optional[str] = None) -> Dict[str, Any]:
        return asyncio.run(self._async_client.dump(format, session_id))
