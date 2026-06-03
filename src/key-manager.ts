// ─────────────────────────────────────────────────────────────────────────────
// PrxyClaude · Key Manager  (round-robin with ban-on-429)
// ─────────────────────────────────────────────────────────────────────────────

import type { KeySlot } from "./types.js";
import { log } from "./logger.js";

const pools = new Map<string, KeySlot[]>();

export function initKeyPool(providerId: string, keys: string[]): void {
  pools.set(
    providerId,
    keys.map((key) => ({
      key,
      usageCount: 0,
      errorCount: 0,
      lastUsedAt: null,
      bannedUntil: null,
      rateLimitHits: 0,
    }))
  );
}

/** Returns the next available API key or null if all are banned. */
export function nextKey(providerId: string): string | null {
  const slots = pools.get(providerId);
  if (!slots || slots.length === 0) return null;

  const now = Date.now();
  const available = slots.filter(
    (s) => s.bannedUntil === null || s.bannedUntil <= now
  );

  if (available.length === 0) {
    // All keys are rate-limited; find the one with the soonest unban
    const soonest = slots.reduce((a, b) =>
      (a.bannedUntil ?? 0) < (b.bannedUntil ?? 0) ? a : b
    );
    const waitMs = Math.max(0, (soonest.bannedUntil ?? 0) - now);
    log("warn", `[keys] ${providerId}: all keys banned, soonest free in ${waitMs}ms`);
    return null;
  }

  // Prefer the key with the fewest recent uses
  available.sort((a, b) => a.usageCount - b.usageCount);
  const chosen = available[0];
  chosen.usageCount++;
  chosen.lastUsedAt = now;
  return chosen.key;
}

/**
 * Ban a key for a period after a 429.
 * Default: 60 s; on repeated hits, exponential up to 10 min.
 */
export function banKey(
  providerId: string,
  key: string,
  retryAfterMs = 60_000
): void {
  const slots = pools.get(providerId);
  if (!slots) return;

  const slot = slots.find((s) => s.key === key);
  if (!slot) return;

  slot.rateLimitHits++;
  const backoff = Math.min(
    retryAfterMs * Math.pow(2, slot.rateLimitHits - 1),
    600_000
  );
  slot.bannedUntil = Date.now() + backoff;
  log(
    "warn",
    `[keys] ${providerId} key ...${key.slice(-6)} banned for ${backoff / 1000}s (hit #${slot.rateLimitHits})`
  );
}

export function markKeyError(providerId: string, key: string): void {
  const slots = pools.get(providerId);
  const slot = slots?.find((s) => s.key === key);
  if (slot) slot.errorCount++;
}

export function getKeyStats(providerId: string): Omit<KeySlot, "key">[] {
  const slots = pools.get(providerId);
  if (!slots) return [];
  return slots.map(({ key, ...rest }) => rest); // don't leak keys
}

export function allKeyPoolStats(): Record<string, ReturnType<typeof getKeyStats>> {
  const out: Record<string, ReturnType<typeof getKeyStats>> = {};
  for (const [id] of pools) out[id] = getKeyStats(id);
  return out;
}
