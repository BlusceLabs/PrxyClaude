import type { KeySlot } from "./types.js";
export declare function initKeyPool(providerId: string, keys: string[]): void;
/** Returns the next available API key or null if all are banned. */
export declare function nextKey(providerId: string): string | null;
/**
 * Ban a key for a period after a 429.
 * Default: 60 s; on repeated hits, exponential up to 10 min.
 */
export declare function banKey(providerId: string, key: string, retryAfterMs?: number): void;
export declare function markKeyError(providerId: string, key: string): void;
export declare function getKeyStats(providerId: string): Omit<KeySlot, "key">[];
export declare function allKeyPoolStats(): Record<string, ReturnType<typeof getKeyStats>>;
//# sourceMappingURL=key-manager.d.ts.map