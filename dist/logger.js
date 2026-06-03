// ─────────────────────────────────────────────────────────────────────────────
// PrxyClaude · Logger
// ─────────────────────────────────────────────────────────────────────────────
const LEVEL_ORDER = {
    debug: 0,
    info: 1,
    warn: 2,
    error: 3,
};
const COLORS = {
    debug: "\x1b[90m", // gray
    info: "\x1b[36m", // cyan
    warn: "\x1b[33m", // yellow
    error: "\x1b[31m", // red
};
const RESET = "\x1b[0m";
const DIM = "\x1b[2m";
let currentLevel = process.env.LOG_LEVEL ?? "info";
export function setLogLevel(level) {
    currentLevel = level;
}
export function log(level, msg, meta) {
    if (LEVEL_ORDER[level] < LEVEL_ORDER[currentLevel])
        return;
    const ts = new Date().toISOString();
    const color = COLORS[level];
    const lvlStr = level.toUpperCase().padEnd(5);
    let line = `${DIM}${ts}${RESET} ${color}${lvlStr}${RESET} ${msg}`;
    if (meta !== undefined) {
        line += ` ${DIM}${JSON.stringify(meta)}${RESET}`;
    }
    if (level === "error") {
        console.error(line);
    }
    else {
        console.log(line);
    }
}
export const logger = {
    debug: (msg, meta) => log("debug", msg, meta),
    info: (msg, meta) => log("info", msg, meta),
    warn: (msg, meta) => log("warn", msg, meta),
    error: (msg, meta) => log("error", msg, meta),
};
//# sourceMappingURL=logger.js.map