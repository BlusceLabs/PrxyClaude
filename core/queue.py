"""PrxyClaude · Priority Queue with Concurrency Control"""

from __future__ import annotations

import asyncio
import time
import uuid
from collections.abc import AsyncIterator, Callable, Coroutine
from dataclasses import dataclass, field
from typing import Any

from core.metrics import record_queued


@dataclass(order=True)
class QueueItem:
    priority: int
    created_at: float
    id: str = field(compare=False)
    tier: str = field(compare=False)
    timeout_at: float = field(compare=False)
    future: asyncio.Future = field(compare=False)
    execute: Callable[[], Coroutine[Any, Any, Any]] = field(compare=False)


class RequestQueue:
    def __init__(
        self, max_size: int = 200, timeout_ms: float = 120_000, max_concurrent: int = 10
    ):
        self._queue: asyncio.PriorityQueue[QueueItem] = asyncio.PriorityQueue(
            maxsize=max_size
        )
        self._timeout_ms = timeout_ms
        self._max_concurrent = max_concurrent
        self._semaphore = asyncio.Semaphore(max_concurrent)
        self._active = 0

    @property
    def depth(self) -> int:
        return self._queue.qsize()

    @property
    def active(self) -> int:
        return self._active

    @property
    def max_concurrent(self) -> int:
        return self._max_concurrent

    def stats(self) -> dict:
        return {
            "depth": self.depth,
            "maxSize": self._queue.maxsize,
            "active": self._active,
            "maxConcurrent": self._max_concurrent,
        }

    async def enqueue(
        self, execute: Callable[[], Coroutine[Any, Any, Any]], tier: str
    ) -> Any:
        TIER_PRIORITY = {"opus": 0, "sonnet": 1, "haiku": 2}
        priority = TIER_PRIORITY.get(tier, 1)
        now = time.time() * 1000

        record_queued()

        future = asyncio.get_event_loop().create_future()
        item = QueueItem(
            priority=priority,
            created_at=now,
            id=f"q_{uuid.uuid4().hex[:12]}",
            tier=tier,
            timeout_at=now + self._timeout_ms,
            future=future,
            execute=execute,
        )

        try:
            self._queue.put_nowait(item)
        except asyncio.QueueFull as err:
            raise RuntimeError("Request queue is full") from err

        async with self._semaphore:
            self._active += 1
            try:
                # Check if we timed out while waiting
                if time.time() * 1000 > item.timeout_at:
                    if not future.done():
                        future.set_exception(TimeoutError("Request timed out in queue"))
                    raise TimeoutError("Request timed out in queue")
                result = await execute()
                if not future.done():
                    future.set_result(result)
                return result
            finally:
                self._active -= 1

    async def enqueue_stream(
        self, execute: Callable[[], AsyncIterator[str]], tier: str
    ) -> AsyncIterator[str]:
        """Enqueue an async generator for streaming execution with concurrency control."""
        now = time.time() * 1000

        record_queued()

        # Wait for concurrency slot
        async with self._semaphore:
            self._active += 1
            try:
                timeout_at = now + self._timeout_ms
                if time.time() * 1000 > timeout_at:
                    raise TimeoutError("Request timed out in queue")
                async for chunk in execute():
                    yield chunk
            finally:
                self._active -= 1


_queue: RequestQueue | None = None


def get_queue() -> RequestQueue:
    global _queue
    if _queue is None:
        _queue = RequestQueue()
    return _queue


def queue_stats() -> dict:
    return get_queue().stats()
