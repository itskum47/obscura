import asyncio
import os
import stat
import tempfile
import pytest
from obscura_next.client import AsyncObscuraClient

@pytest.mark.asyncio
async def test_python_sdk_handshake_timeout_and_no_leak():
    # Create a script that sleeps and doesn't emit ready
    with tempfile.NamedTemporaryFile("w", delete=False) as f:
        f.write("#!/bin/sh\nsleep 30\n")
        dummy_path = f.name
    os.chmod(dummy_path, stat.S_IRUSR | stat.S_IXUSR)

    client = AsyncObscuraClient(binary_path=dummy_path)
    try:
        with pytest.raises((RuntimeError, asyncio.TimeoutError)):
            await client.start(handshake_timeout=0.2)
        # Ensure client._proc was cleaned up
        assert client._proc is None
    finally:
        os.remove(dummy_path)

@pytest.mark.asyncio
async def test_python_sdk_premature_worker_exit():
    # Create a script that emits ready then exits immediately
    with tempfile.NamedTemporaryFile("w", delete=False) as f:
        f.write("#!/bin/sh\necho '{\"ready\":true}'\nread line\nexit 1\n")
        dummy_path = f.name
    os.chmod(dummy_path, stat.S_IRUSR | stat.S_IXUSR)

    client = AsyncObscuraClient(binary_path=dummy_path)
    try:
        await client.start()
        with pytest.raises(RuntimeError) as exc_info:
            await client.ping()
        assert "terminated unexpectedly" in str(exc_info.value)
    finally:
        await client.close()
        os.remove(dummy_path)
