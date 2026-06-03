import type { GlobalMetrics, ProviderMetrics } from "./types.js";
export declare function recordRequest(): void;
export declare function recordCacheHit(providerId?: string): void;
export declare function recordQueued(): void;
export declare function recordProviderSuccess(providerId: string, latencyMs: number, tokensIn?: number, tokensOut?: number): void;
export declare function recordProviderFailure(providerId: string, errMsg?: string): void;
export declare function getMetrics(): GlobalMetrics;
export declare function getProviderMetrics(id: string): ProviderMetrics | undefined;
//# sourceMappingURL=metrics.d.ts.map