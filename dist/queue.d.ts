import type { ModelTier } from "./types.js";
export declare function enqueue<T>(execute: () => Promise<T>, tier?: ModelTier): Promise<T>;
export declare function queueStats(): {
    depth: number;
    active: number;
    maxConcurrent: number;
    maxSize: number;
};
//# sourceMappingURL=queue.d.ts.map