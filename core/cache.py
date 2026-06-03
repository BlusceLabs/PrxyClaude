"""PrxyClaude · LRU Response Cache"""

from __future__ import annotations

import hashlib
import json
import time
from collections import OrderedDict
from typing import Any

from loguru import logger

DEFAULT_MAX_ENTRIES = 500
DEFAULT_TTL_MS = 5 * 60 * 1000  # 5 minutes


class ResponseCache:
    def __init__(
        self, max_entries: int = DEFAULT_MAX_ENTRIES, ttl_ms: int = DEFAULT_TTL_MS
    ):
        self._cache: OrderedDict[str, tuple[Any, float]] = OrderedDict()
        self._max_entries = max_entries
        self._ttl_ms = ttl_ms

    def _make_key(self, request: dict) -> str:
        data = json.dumps(request, sort_keys=True, default=str)
        return hashlib.sha256(data.encode()).hexdigest()

    def get(self, request: dict) -> Any | None:
        key = self._make_key(request)
        if key not in self._cache:
            return None
        value, ts = self._cache[key]
        if (time.time() * 1000 - ts) > self._ttl_ms:
            del self._cache[key]
            return None
        self._cache.move_to_end(key)
        return value

    def set(self, request: dict, response: Any) -> None:
        key = self._make_key(request)
        self._cache[key] = (response, time.time() * 1000)
        self._cache.move_to_end(key)
        while len(self._cache) > self._max_entries:
            self._cache.popitem(last=False)

    def clear(self) -> None:
        self._cache.clear()
        logger.info("[cache] cleared")

    def stats(self) -> dict:
        return {
            "size": len(self._cache),
            "maxEntries": self._max_entries,
            "ttlMs": self._ttl_ms,
        }


_cache: ResponseCache | None = None


def get_cache() -> ResponseCache:
    global _cache
    if _cache is None:
        _cache = ResponseCache()
    return _cache


def cache_get(request: dict) -> Any | None:
    return get_cache().get(request)


def cache_set(request: dict, response: Any) -> None:
    get_cache().set(request, response)


def cache_clear() -> None:
    get_cache().clear()


def cache_stats() -> dict:
    return get_cache().stats()
