"""PrxyClaude · Process Registry (subprocess management)"""

from __future__ import annotations

import contextlib
import os
import signal

from loguru import logger

# Track spawned subprocesses for cleanup on shutdown
_spawned_pids: set[int] = set()


def register(pid: int) -> None:
    """Register a subprocess PID for cleanup."""
    _spawned_pids.add(pid)
    logger.debug(f"[process_registry] registered pid {pid}")


def register_pid(pid: int) -> None:
    """Register a subprocess PID for cleanup (alias for register)."""
    register(pid)


def unregister(pid: int) -> None:
    """Unregister a subprocess PID (e.g., when it exits normally)."""
    _spawned_pids.discard(pid)


def unregister_pid(pid: int) -> None:
    """Unregister a subprocess PID (alias for unregister)."""
    unregister(pid)


def kill_all_best_effort() -> None:
    """Best-effort cleanup: send SIGTERM to all tracked processes."""
    if not _spawned_pids:
        return
    logger.info(f"[process_registry] cleaning up {len(_spawned_pids)} subprocess(es)")
    for pid in list(_spawned_pids):
        with contextlib.suppress(ProcessLookupError, PermissionError):
            os.kill(pid, signal.SIGTERM)
    _spawned_pids.clear()
