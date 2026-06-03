// ─────────────────────────────────────────────────────────────────────────────
// PrxyClaude · Circuit Breaker
// ─────────────────────────────────────────────────────────────────────────────
import { getConfig } from "./config.js";
import { log } from "./logger.js";
const circuits = new Map();
function getCircuit(providerId) {
    if (!circuits.has(providerId)) {
        circuits.set(providerId, {
            providerId,
            state: "closed",
            failures: 0,
            lastFailureAt: null,
            lastSuccessAt: null,
            retryAfter: null,
            totalRequests: 0,
            totalFailures: 0,
        });
    }
    return circuits.get(providerId);
}
export function isCircuitOpen(providerId) {
    const cb = getCircuit(providerId);
    const cfg = getConfig().circuitBreaker;
    if (cb.state === "closed")
        return false;
    if (cb.state === "open") {
        if (cb.retryAfter && Date.now() >= cb.retryAfter) {
            // Transition to half-open: allow one probe request through
            cb.state = "half-open";
            cb.failures = 0;
            log("info", `[circuit] ${providerId} → half-open (probing)`);
            return false;
        }
        return true;
    }
    // half-open: allow through
    return false;
}
export function recordSuccess(providerId) {
    const cb = getCircuit(providerId);
    const cfg = getConfig().circuitBreaker;
    cb.totalRequests++;
    cb.lastSuccessAt = Date.now();
    if (cb.state === "half-open") {
        cb.failures = 0;
        if (cb.failures === 0) {
            cb.state = "closed";
            cb.retryAfter = null;
            log("info", `[circuit] ${providerId} → closed (recovered)`);
        }
    }
    else if (cb.state === "closed") {
        cb.failures = Math.max(0, cb.failures - 1);
    }
}
export function recordFailure(providerId, reason) {
    const cb = getCircuit(providerId);
    const cfg = getConfig().circuitBreaker;
    cb.totalRequests++;
    cb.totalFailures++;
    cb.failures++;
    cb.lastFailureAt = Date.now();
    log("warn", `[circuit] ${providerId} failure #${cb.failures}${reason ? ": " + reason : ""}`);
    if (cb.state === "half-open" || cb.failures >= cfg.failureThreshold) {
        cb.state = "open";
        cb.retryAfter = Date.now() + cfg.halfOpenAfterMs;
        log("error", `[circuit] ${providerId} → OPEN (retry in ${cfg.halfOpenAfterMs / 1000}s)`);
    }
}
export function getCircuitStates() {
    return [...circuits.values()];
}
export function resetCircuit(providerId) {
    const cb = getCircuit(providerId);
    cb.state = "closed";
    cb.failures = 0;
    cb.retryAfter = null;
    log("info", `[circuit] ${providerId} manually reset`);
}
//# sourceMappingURL=circuit-breaker.js.map