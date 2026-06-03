"""PrxyClaude · Key Manager (round-robin with ban-on-429)"""

from __future__ import annotations

from dataclasses import dataclass


@dataclass
class KeySlot:
    key: str
    usage_count: int = 0
    error_count: int = 0
    last_used_at: float | None = None
    banned_until: float | None = None
    rate_limit_hits: int = 0


_pools: dict[str, list[KeySlot]] = {}


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
