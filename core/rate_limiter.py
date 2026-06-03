"""PrxyClaude · Rate Limiter (rolling window + 429 backoff)"""

from __future__ import annotations

import time
from collections import deque

from loguru import logger


class RateLimiter:
    """Rolling-window rate limiter with per-provider tracking."""

    def __init__(self, max_requests: int = 40, window_seconds: float = 60):
        self._max_requests = max_requests
        self._window_seconds = window_seconds
        self._windows: dict[str, deque[float]] = {}
        self._backoff_until: dict[str, float] = {}

    def _get_window(self, provider_id: str) -> deque[float]:
        if provider_id not in self._windows:
            self._windows[provider_id] = deque()
        return self._windows[provider_id]

    def can_proceed(self, provider_id: str) -> bool:
        """Check if a request can proceed for this provider."""
        now = time.time()

        # Check if in backoff period
        backoff = self._backoff_until.get(provider_id, 0)
        if now < backoff:
            remaining = backoff - now
            logger.debug(f"[ratelimit] {provider_id} in backoff for {remaining:.1f}s")
            return False

        window = self._get_window(provider_id)

        # Remove expired entries
        cutoff = now - self._window_seconds
        while window and window[0] < cutoff:
            window.popleft()

        if len(window) >= self._max_requests:
            oldest = window[0]
            wait_time = self._window_seconds - (now - oldest)
            logger.debug(f"[ratelimit] {provider_id} at limit, wait {wait_time:.1f}s")
            return False

        return True

    def record_request(self, provider_id: str) -> None:
        """Record a request timestamp."""
        window = self._get_window(provider_id)
        window.append(time.time())

    def record_429(self, provider_id: str, retry_after: float = 60) -> None:
        """Record a 429 rate limit hit with backoff."""
        now = time.time()
        self._backoff_until[provider_id] = now + retry_after
        logger.warning(f"[ratelimit] {provider_id} 429 backoff for {retry_after:.0f}s")

    def get_status(self, provider_id: str) -> dict:
        """Get rate limit status for a provider."""
        now = time.time()
        window = self._get_window(provider_id)

        cutoff = now - self._window_seconds
        while window and window[0] < cutoff:
            window.popleft()

        backoff = self._backoff_until.get(provider_id, 0)
        in_backoff = now < backoff

        return {
            "requests_in_window": len(window),
            "max_requests": self._max_requests,
            "window_seconds": self._window_seconds,
            "in_backoff": in_backoff,
            "backoff_remaining": max(0, backoff - now) if in_backoff else 0,
        }


# Singleton
_limiter: RateLimiter | None = None


def get_rate_limiter() -> RateLimiter:
    global _limiter
    if _limiter is None:
        _limiter = RateLimiter()
    return _limiter


def configure_rate_limiter(max_requests: int, window_seconds: float) -> RateLimiter:
    global _limiter
    _limiter = RateLimiter(max_requests, window_seconds)
    return _limiter
