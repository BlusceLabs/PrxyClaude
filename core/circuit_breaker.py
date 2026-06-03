"""PrxyClaude · Circuit Breaker"""

from __future__ import annotations

import time
from dataclasses import dataclass

from loguru import logger

from core.types import CircuitState


@dataclass
class Circuit:
    provider_id: str
    state: CircuitState = CircuitState.closed
    failures: int = 0
    last_failure_at: float | None = None
    last_success_at: float | None = None
    retry_after: float | None = None
    total_requests: int = 0
    total_failures: int = 0
    consecutive_successes: int = 0


_circuits: dict[str, Circuit] = {}

FAILURE_THRESHOLD = 5
HALF_OPEN_AFTER_MS = 30_000
SUCCESS_THRESHOLD = 2


def _get_circuit(provider_id: str) -> Circuit:
    if provider_id not in _circuits:
        _circuits[provider_id] = Circuit(provider_id=provider_id)
    return _circuits[provider_id]


def can_proceed(provider_id: str) -> bool:
    c = _get_circuit(provider_id)
    c.total_requests += 1

    if c.state == CircuitState.closed:
        return True

    if c.state == CircuitState.open:
        now = time.time() * 1000
        if (
            c.last_failure_at is not None
            and (now - c.last_failure_at) >= HALF_OPEN_AFTER_MS
        ):
            c.state = CircuitState.half_open
            logger.info(f"[circuit] {provider_id} -> half-open (timeout elapsed)")
            return True
        return False

    return c.state == CircuitState.half_open


def record_success(provider_id: str) -> None:
    c = _get_circuit(provider_id)
    c.last_success_at = time.time() * 1000
    c.consecutive_successes += 1

    if c.state == CircuitState.half_open:
        if c.consecutive_successes >= SUCCESS_THRESHOLD:
            c.state = CircuitState.closed
            c.failures = 0
            c.retry_after = None
            c.consecutive_successes = 0
            logger.info(f"[circuit] {provider_id} -> closed (success threshold met)")
    elif c.state == CircuitState.closed:
        c.failures = 0


def record_failure(provider_id: str) -> None:
    c = _get_circuit(provider_id)
    c.last_failure_at = time.time() * 1000
    c.total_failures += 1
    c.consecutive_successes = 0

    if c.state == CircuitState.closed:
        c.failures += 1
        if c.failures >= FAILURE_THRESHOLD:
            c.state = CircuitState.open
            c.retry_after = c.last_failure_at + HALF_OPEN_AFTER_MS
            logger.warning(
                f"[circuit] {provider_id} -> open "
                f"({c.failures} failures, retry after {HALF_OPEN_AFTER_MS}ms)"
            )
    elif c.state == CircuitState.half_open:
        c.state = CircuitState.open
        c.retry_after = c.last_failure_at + HALF_OPEN_AFTER_MS
        logger.warning(f"[circuit] {provider_id} -> open (failed in half-open)")


def get_circuit_states() -> list[dict]:
    return [
        {
            "providerId": c.provider_id,
            "state": c.state.value,
            "failures": c.failures,
            "lastFailureAt": c.last_failure_at,
            "lastSuccessAt": c.last_success_at,
            "retryAfter": c.retry_after,
            "totalRequests": c.total_requests,
            "totalFailures": c.total_failures,
        }
        for c in _circuits.values()
    ]


def reset_circuit(provider_id: str) -> None:
    if provider_id in _circuits:
        c = _circuits[provider_id]
        c.state = CircuitState.closed
        c.failures = 0
        c.retry_after = None
        c.consecutive_successes = 0
        logger.info(f"[circuit] {provider_id} -> reset to closed")
