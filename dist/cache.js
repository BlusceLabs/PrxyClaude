// ─────────────────────────────────────────────────────────────────────────────
// PrxyClaude · LRU Response Cache
// ─────────────────────────────────────────────────────────────────────────────
import crypto from "crypto";
import { getConfig } from "./config.js";
import { log } from "./logger.js";
const store = new Map();
function cacheKey(req) {
    const payload = JSON.stringify({
        model: req.model,
        messages: req.messages,
        system: req.system,
        tools: req.tools,
        temperature: req.temperature,
        max_tokens: req.max_tokens,
    });
    return crypto.createHash("sha256").update(payload).digest("hex").slice(0, 32);
}
function evict() {
    const cfg = getConfig().cache;
    if (store.size <= cfg.maxEntries)
        return;
    // Evict oldest by createdAt
    const entries = [...store.entries()].sort(([, a], [, b]) => a.createdAt - b.createdAt);
    const toRemove = entries.slice(0, Math.ceil(cfg.maxEntries * 0.2));
    for (const [k] of toRemove)
        store.delete(k);
    log("debug", `[cache] evicted ${toRemove.length} entries`);
}
export function cacheGet(req) {
    const cfg = getConfig().cache;
    if (!cfg.enabled || req.stream)
        return null; // never cache streams
    const key = cacheKey(req);
    const entry = store.get(key);
    if (!entry)
        return null;
    if (Date.now() - entry.createdAt > cfg.ttlMs) {
        store.delete(key);
        return null;
    }
    entry.hits++;
    log("debug", `[cache] HIT ${key.slice(0, 8)}… (hits=${entry.hits})`);
    return entry.value;
}
export function cacheSet(req, value) {
    const cfg = getConfig().cache;
    if (!cfg.enabled || req.stream)
        return;
    const key = cacheKey(req);
    store.set(key, { value, createdAt: Date.now(), hits: 0 });
    evict();
}
export function cacheStats() {
    return {
        size: store.size,
        maxEntries: getConfig().cache.maxEntries,
        ttlMs: getConfig().cache.ttlMs,
        enabled: getConfig().cache.enabled,
    };
}
export function cacheClear() {
    store.clear();
    log("info", "[cache] cleared");
}
//# sourceMappingURL=cache.js.map