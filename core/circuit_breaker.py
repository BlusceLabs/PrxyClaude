"""PrxyClaude · Circuit Breaker"""

from __future__ import annotations

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


_circuits: dict[str, Circuit] = {}

# Defaults
FAILURE_THRESHOLD = 5
HALF_OPEN_AFTER_MS = 30_000
SUCCESS_THRESHOLD = 2


def _get_circuit(provider_id: str) -> Circuit:
    if provider_id not in _circuits:
        _circuits[provider_id] = Circuit(provider_id=provider_id)
    return _circuits[provider_id]


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
        logger.info(f"[circuit] {provider_id} -> reset to closed")
