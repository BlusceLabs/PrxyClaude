// ─────────────────────────────────────────────────────────────────────────────
// PrxyClaude · Logger
// ─────────────────────────────────────────────────────────────────────────────

type Level = "debug" | "info" | "warn" | "error";

const LEVEL_ORDER: Record<Level, number> = {
  debug: 0,
  info: 1,
  warn: 2,
  error: 3,
};

const COLORS: Record<Level, string> = {
  debug: "\x1b[90m",   // gray
  info:  "\x1b[36m",   // cyan
  warn:  "\x1b[33m",   // yellow
  error: "\x1b[31m",   // red
};
const RESET = "\x1b[0m";
const DIM   = "\x1b[2m";

let currentLevel: Level = (process.env.LOG_LEVEL as Level) ?? "info";

export function setLogLevel(level: Level): void {
  currentLevel = level;
}

export function log(level: Level, msg: string, meta?: unknown): void {
  if (LEVEL_ORDER[level] < LEVEL_ORDER[currentLevel]) return;

  const ts = new Date().toISOString();
  const color = COLORS[level];
  const lvlStr = level.toUpperCase().padEnd(5);

  let line = `${DIM}${ts}${RESET} ${color}${lvlStr}${RESET} ${msg}`;
  if (meta !== undefined) {
    line += ` ${DIM}${JSON.stringify(meta)}${RESET}`;
  }

  if (level === "error") {
    console.error(line);
  } else {
    console.log(line);
  }
}

export const logger = {
  debug: (msg: string, meta?: unknown) => log("debug", msg, meta),
  info:  (msg: string, meta?: unknown) => log("info",  msg, meta),
  warn:  (msg: string, meta?: unknown) => log("warn",  msg, meta),
  error: (msg: string, meta?: unknown) => log("error", msg, meta),
};
