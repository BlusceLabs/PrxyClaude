// ─────────────────────────────────────────────────────────────────────────────
// PrxyClaude · Metrics
// ─────────────────────────────────────────────────────────────────────────────
const metrics = {
    totalRequests: 0,
    cachedRequests: 0,
    queuedRequests: 0,
    providers: {},
    startedAt: Date.now(),
};
const MAX_LATENCY_SAMPLES = 200;
function ensureProvider(id) {
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
export function recordRequest() {
    metrics.totalRequests++;
}
export function recordCacheHit(providerId) {
    metrics.cachedRequests++;
    if (providerId) {
        const p = ensureProvider(providerId);
        p.cachedHits++;
    }
}
export function recordQueued() {
    metrics.queuedRequests++;
}
export function recordProviderSuccess(providerId, latencyMs, tokensIn = 0, tokensOut = 0) {
    const p = ensureProvider(providerId);
    p.requests++;
    p.successes++;
    p.totalTokensIn += tokensIn;
    p.totalTokensOut += tokensOut;
    p.lastUsedAt = Date.now();
    p.latencies.push(latencyMs);
    if (p.latencies.length > MAX_LATENCY_SAMPLES)
        p.latencies.shift();
    p.avgLatencyMs =
        p.latencies.reduce((a, b) => a + b, 0) / p.latencies.length;
}
export function recordProviderFailure(providerId, errMsg) {
    const p = ensureProvider(providerId);
    p.requests++;
    p.failures++;
    p.lastErrorMsg = errMsg;
}
export function getMetrics() {
    return metrics;
}
export function getProviderMetrics(id) {
    return metrics.providers[id];
}
//# sourceMappingURL=metrics.js.map