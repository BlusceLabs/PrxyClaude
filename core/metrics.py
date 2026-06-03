"""PrxyClaude · Metrics"""

from __future__ import annotations

from core.types import GlobalMetrics, ProviderMetrics

_metrics = GlobalMetrics()

MAX_LATENCY_SAMPLES = 200


def record_request() -> None:
    _metrics.total_requests += 1


def record_cache_hit(provider_id: str | None = None) -> None:
    _metrics.cached_requests += 1
    if provider_id:
        m = _metrics.providers.setdefault(provider_id, ProviderMetrics())
        m.cached_hits += 1


def record_queued() -> None:
    _metrics.queued_requests += 1


def record_provider_success(
    provider_id: str,
    latency_ms: float = 0,
    tokens_in: int | None = None,
    tokens_out: int | None = None,
) -> None:
    m = _metrics.providers.setdefault(provider_id, ProviderMetrics())
    m.requests += 1
    m.successes += 1
    if latency_ms > 0:
        m.latencies.append(latency_ms)
        if len(m.latencies) > MAX_LATENCY_SAMPLES:
            m.latencies = m.latencies[-MAX_LATENCY_SAMPLES:]
        m.avg_latency_ms = sum(m.latencies) / len(m.latencies)
    if tokens_in is not None:
        m.total_tokens_in += tokens_in
    if tokens_out is not None:
        m.total_tokens_out += tokens_out


def record_provider_failure(provider_id: str, error_msg: str = "") -> None:
    m = _metrics.providers.setdefault(provider_id, ProviderMetrics())
    m.requests += 1
    m.failures += 1
    if error_msg:
        m.last_error_msg = error_msg[:200]


def get_metrics() -> GlobalMetrics:
    return _metrics


def get_provider_metrics(provider_id: str) -> ProviderMetrics | None:
    return _metrics.providers.get(provider_id)
