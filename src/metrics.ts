// ─────────────────────────────────────────────────────────────────────────────
// PrxyClaude · Metrics
// ─────────────────────────────────────────────────────────────────────────────

import type { GlobalMetrics, ProviderMetrics } from "./types.js";

const metrics: GlobalMetrics = {
  totalRequests: 0,
  cachedRequests: 0,
  queuedRequests: 0,
  providers: {},
  startedAt: Date.now(),
};

const MAX_LATENCY_SAMPLES = 200;

function ensureProvider(id: string): ProviderMetrics {
  if (!metrics.providers[id]) {
    metrics.providers[id] = {
      providerId: id,
      requests: 0,
      successes: 0,
      failures: 0,
      cachedHits: 0,
      totalTokensIn: 0,
      totalTokensOut: 0,
      avgLatencyMs: 0,
      latencies: [],
    };
  }
  return metrics.providers[id];
}

export function recordRequest(): void {
  metrics.totalRequests++;
}

export function recordCacheHit(providerId?: string): void {
  metrics.cachedRequests++;
  if (providerId) {
    const p = ensureProvider(providerId);
    p.cachedHits++;
  }
}

export function recordQueued(): void {
  metrics.queuedRequests++;
}

export function recordProviderSuccess(
  providerId: string,
  latencyMs: number,
  tokensIn = 0,
  tokensOut = 0
): void {
  const p = ensureProvider(providerId);
  p.requests++;
  p.successes++;
  p.totalTokensIn += tokensIn;
  p.totalTokensOut += tokensOut;
  p.lastUsedAt = Date.now();

  p.latencies.push(latencyMs);
  if (p.latencies.length > MAX_LATENCY_SAMPLES) p.latencies.shift();
  p.avgLatencyMs =
    p.latencies.reduce((a, b) => a + b, 0) / p.latencies.length;
}

export function recordProviderFailure(providerId: string, errMsg?: string): void {
  const p = ensureProvider(providerId);
  p.requests++;
  p.failures++;
  p.lastErrorMsg = errMsg;
}

export function getMetrics(): GlobalMetrics {
  return metrics;
}

export function getProviderMetrics(id: string): ProviderMetrics | undefined {
  return metrics.providers[id];
}
