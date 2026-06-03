// ─────────────────────────────────────────────────────────────────────────────
// PrxyClaude · Priority Request Queue
// ─────────────────────────────────────────────────────────────────────────────
import { getConfig } from "./config.js";
import { log } from "./logger.js";
import { recordQueued } from "./metrics.js";
import crypto from "crypto";
const queue = [];
let activeCount = 0;
let draining = false;
const TIER_PRIORITY = {
    opus: 3,
    sonnet: 2,
    haiku: 1,
};
function sortQueue() {
    queue.sort((a, b) => {
        if (b.priority !== a.priority)
            return b.priority - a.priority;
        return a.createdAt - b.createdAt; // FIFO within same priority
    });
}
async function drain() {
    if (draining)
        return;
    draining = true;
    const cfg = getConfig().queue;
    while (queue.length > 0 && activeCount < cfg.maxConcurrent) {
        sortQueue();
        const item = queue.shift();
        // Check timeout
        if (Date.now() > item.timeoutAt) {
            item.reject(new Error("Request timed out in queue"));
            continue;
        }
        activeCount++;
        (async () => {
            try {
                const result = await item.execute();
                item.resolve(result);
            }
            catch (err) {
                item.reject(err instanceof Error ? err : new Error(String(err)));
            }
            finally {
                activeCount--;
                setImmediate(drain);
            }
        })();
    }
    draining = false;
}
export function enqueue(execute, tier = "sonnet") {
    const cfg = getConfig().queue;
    if (queue.length >= cfg.maxSize) {
        return Promise.reject(new Error(`Queue full (${cfg.maxSize} requests). Please retry later.`));
    }
    recordQueued();
    return new Promise((resolve, reject) => {
        const item = {
            id: crypto.randomUUID(),
            priority: TIER_PRIORITY[tier],
            tier,
            createdAt: Date.now(),
            timeoutAt: Date.now() + cfg.timeoutMs,
            resolve: resolve,
            reject,
            execute,
        };
        queue.push(item);
        log("debug", `[queue] enqueued ${item.id} tier=${tier} depth=${queue.length}`);
        setImmediate(drain);
    });
}
export function queueStats() {
    const cfg = getConfig().queue;
    return {
        depth: queue.length,
        active: activeCount,
        maxConcurrent: cfg.maxConcurrent,
        maxSize: cfg.maxSize,
    };
}
//# sourceMappingURL=queue.js.map