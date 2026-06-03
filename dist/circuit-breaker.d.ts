import type { CircuitBreakerState } from "./types.js";
export declare function isCircuitOpen(providerId: string): boolean;
export declare function recordSuccess(providerId: string): void;
export declare function recordFailure(providerId: string, reason?: string): void;
export declare function getCircuitStates(): CircuitBreakerState[];
export declare function resetCircuit(providerId: string): void;
//# sourceMappingURL=circuit-breaker.d.ts.map