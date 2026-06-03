"""PrxyClaude · Key Manager (round-robin with ban-on-429)"""

from __future__ import annotations

import time
from dataclasses import dataclass

from loguru import logger

BAN_DURATION_429 = 300.0
MAX_ERROR_RATE = 0.5


@dataclass
class KeySlot:
    key: str
    usage_count: int = 0
    error_count: int = 0
    last_used_at: float | None = None
    banned_until: float | None = None
    rate_limit_hits: int = 0
    consecutive_failures: int = 0


_pools: dict[str, list[KeySlot]] = {}
_round_robin_index: dict[str, int] = {}


def add_key(provider_id: str, key: str) -> None:
    if provider_id not in _pools:
        _pools[provider_id] = []
        _round_robin_index[provider_id] = 0
    if not any(s.key == key for s in _pools[provider_id]):
        _pools[provider_id].append(KeySlot(key=key))
        logger.info(f"[keys] Added key for {provider_id}")


def get_next_key(provider_id: str) -> str | None:
    slots = _pools.get(provider_id)
    if not slots:
        return None

    idx = _round_robin_index.get(provider_id, 0)

    for offset in range(len(slots)):
        slot = slots[(idx + offset) % len(slots)]
        now = time.time()
        if slot.banned_until is not None and now < slot.banned_until:
            continue
        next_idx = (idx + offset + 1) % len(slots)
        _round_robin_index[provider_id] = next_idx
        return slot.key

    return None


def record_success(provider_id: str, key: str) -> None:
    slot = _find_slot(provider_id, key)
    if slot is None:
        return
    slot.usage_count += 1
    slot.last_used_at = time.time()
    slot.consecutive_failures = 0


def record_failure(provider_id: str, key: str) -> None:
    slot = _find_slot(provider_id, key)
    if slot is None:
        return
    slot.error_count += 1
    slot.last_used_at = time.time()
    slot.consecutive_failures += 1

    total = slot.usage_count + slot.error_count
    if total > 0 and slot.error_count / total >= MAX_ERROR_RATE:
        slot.banned_until = time.time() + BAN_DURATION_429
        logger.warning(
            f"[keys] Banned key for {provider_id} "
            f"({slot.consecutive_failures} consecutive failures, "
            f"error rate {slot.error_count / total:.0%})"
        )


def record_rate_limit(provider_id: str, key: str) -> None:
    slot = _find_slot(provider_id, key)
    if slot is None:
        return
    slot.rate_limit_hits += 1
    slot.banned_until = time.time() + BAN_DURATION_429
    logger.warning(f"[keys] Rate limited key for {provider_id}, banned 5min")


def get_key_stats(provider_id: str) -> list[dict]:
    slots = _pools.get(provider_id, [])
    return [
        {
            "usage_count": s.usage_count,
            "error_count": s.error_count,
            "last_used_at": s.last_used_at,
            "banned_until": s.banned_until,
            "rate_limit_hits": s.rate_limit_hits,
        }
        for s in slots
    ]


def all_key_pool_stats() -> dict[str, list[dict]]:
    return {pid: get_key_stats(pid) for pid in _pools}


def _find_slot(provider_id: str, key: str) -> KeySlot | None:
    slots = _pools.get(provider_id)
    if slots is None:
        return None
    for slot in slots:
        if slot.key == key:
            return slot
    return None
