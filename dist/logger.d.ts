type Level = "debug" | "info" | "warn" | "error";
export declare function setLogLevel(level: Level): void;
export declare function log(level: Level, msg: string, meta?: unknown): void;
export declare const logger: {
    debug: (msg: string, meta?: unknown) => void;
    info: (msg: string, meta?: unknown) => void;
    warn: (msg: string, meta?: unknown) => void;
    error: (msg: string, meta?: unknown) => void;
};
export {};
//# sourceMappingURL=logger.d.ts.map