import type { AnthropicRequest } from "./types.js";
export declare function cacheGet(req: AnthropicRequest): unknown | null;
export declare function cacheSet(req: AnthropicRequest, value: unknown): void;
export declare function cacheStats(): {
    size: number;
    maxEntries: number;
    ttlMs: number;
    enabled: boolean;
};
export declare function cacheClear(): void;
//# sourceMappingURL=cache.d.ts.map