// ─────────────────────────────────────────────────────────────────────────────
// PrxyClaude · Main Server
// ─────────────────────────────────────────────────────────────────────────────
import "dotenv/config";
import express from "express";
import { getConfig } from "./config.js";
import { initProviders, dispatch, dispatchStream } from "./providers/index.js";
import { adminRouter } from "./admin/routes.js";
import { ADMIN_HTML } from "./admin/ui.js";
import { cacheGet, cacheSet } from "./cache.js";
import { enqueue } from "./queue.js";
import { recordRequest, recordCacheHit } from "./metrics.js";
import { detectTier } from "./proxy/transform.ts";
import { log } from "./logger.js";
// ─── App ─────────────────────────────────────────────────────────────────────
const app = express();
app.disable("x-powered-by");
app.use(express.json({ limit: "64mb" }));
// ─── Auth middleware ──────────────────────────────────────────────────────────
function authMiddleware(req, res, next) {
    const cfg = getConfig();
    if (!cfg.proxyAuthToken || cfg.proxyAuthToken === "any") {
        return next();
    }
    const authHeader = req.headers.authorization;
    const authToken = req.headers["x-api-key"];
    const bearerToken = authHeader?.startsWith("Bearer ") ? authHeader.slice(7) : null;
    const provided = bearerToken ?? authToken ?? null;
    if (!provided || provided !== cfg.proxyAuthToken) {
        res.status(401).json({
            type: "error",
            error: { type: "authentication_error", message: "Invalid or missing proxy auth token" },
        });
        return;
    }
    next();
}
function adminAuthMiddleware(req, res, next) {
    const cfg = getConfig();
    const token = req.headers["x-admin-token"] ?? req.query.token;
    if (token !== cfg.adminToken) {
        res.status(403).json({ error: "Forbidden" });
        return;
    }
    next();
}
// ─── CORS ────────────────────────────────────────────────────────────────────
app.use((req, res, next) => {
    res.setHeader("Access-Control-Allow-Origin", "*");
    res.setHeader("Access-Control-Allow-Methods", "GET,POST,PUT,PATCH,DELETE,OPTIONS");
    res.setHeader("Access-Control-Allow-Headers", "Content-Type,Authorization,x-api-key,anthropic-version,anthropic-beta");
    if (req.method === "OPTIONS") {
        res.sendStatus(204);
        return;
    }
    next();
});
// ─── Request logger ──────────────────────────────────────────────────────────
app.use((req, _res, next) => {
    const cfg = getConfig();
    if (cfg.logging.requests) {
        log("debug", `→ ${req.method} ${req.path}`);
    }
    next();
});
// ─── Health ───────────────────────────────────────────────────────────────────
app.get("/health", (_req, res) => {
    res.json({ ok: true, version: "1.0.0", ts: new Date().toISOString() });
});
// ─── Models (Claude Code expects this) ───────────────────────────────────────
app.get("/v1/models", authMiddleware, (_req, res) => {
    res.json({
        data: [
            { id: "claude-opus-4-5", object: "model", created: 1700000000 },
            { id: "claude-sonnet-4-5", object: "model", created: 1700000000 },
            { id: "claude-haiku-4-5-20251001", object: "model", created: 1700000000 },
            { id: "claude-opus-4-6", object: "model", created: 1700000000 },
            { id: "claude-sonnet-4-6", object: "model", created: 1700000000 },
        ],
    });
});
// ─── Messages endpoint ────────────────────────────────────────────────────────
app.post("/v1/messages", authMiddleware, async (req, res) => {
    const body = req.body;
    if (!body.model || !body.messages) {
        res.status(400).json({
            type: "error",
            error: { type: "invalid_request_error", message: "Missing required fields: model, messages" },
        });
        return;
    }
    recordRequest();
    const tier = detectTier(body.model);
    const isStream = body.stream === true;
    // ── Streaming ──
    if (isStream) {
        res.setHeader("Content-Type", "text/event-stream");
        res.setHeader("Cache-Control", "no-cache");
        res.setHeader("Connection", "keep-alive");
        res.setHeader("X-Accel-Buffering", "no");
        try {
            const stream = await enqueue(() => dispatchStream(body), tier);
            const reader = stream.getReader();
            const write = async () => {
                while (true) {
                    const { done, value } = await reader.read();
                    if (done)
                        break;
                    res.write(value);
                }
                res.write("data: [DONE]\n\n");
                res.end();
            };
            await write();
        }
        catch (err) {
            const msg = err instanceof Error ? err.message : String(err);
            log("error", `[stream] ${msg}`);
            const errEvent = `event: error\ndata: ${JSON.stringify({
                type: "error",
                error: { type: "api_error", message: msg },
            })}\n\n`;
            res.write(errEvent);
            res.end();
        }
        return;
    }
    // ── Non-streaming ──
    // Check cache first
    const cached = cacheGet(body);
    if (cached) {
        recordCacheHit();
        res.json(cached);
        return;
    }
    try {
        const result = await enqueue(() => dispatch(body), tier);
        cacheSet(body, result);
        res.json(result);
    }
    catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        log("error", `[messages] ${msg}`);
        res.status(503).json({
            type: "error",
            error: { type: "overloaded_error", message: msg },
        });
    }
});
// ─── Admin UI ─────────────────────────────────────────────────────────────────
app.get("/admin", (_req, res) => {
    res.setHeader("Content-Type", "text/html");
    res.send(ADMIN_HTML);
});
app.use("/admin/api", adminAuthMiddleware, adminRouter());
// ─── 404 ─────────────────────────────────────────────────────────────────────
app.use((_req, res) => {
    res.status(404).json({ error: "Not found" });
});
// ─── Error handler ────────────────────────────────────────────────────────────
app.use((err, _req, res, _next) => {
    log("error", `[unhandled] ${err.message}`);
    res.status(500).json({
        type: "error",
        error: { type: "api_error", message: "Internal server error" },
    });
});
// ─── Start ────────────────────────────────────────────────────────────────────
const cfg = getConfig();
initProviders();
const server = app.listen(cfg.port, "0.0.0.0", () => {
    const lines = [
        "",
        `  ██████╗ ██████╗ ██╗  ██╗██╗   ██╗ ██████╗██╗      █████╗ ██╗   ██╗██████╗ ███████╗`,
        `  ██╔══██╗██╔══██╗╚██╗██╔╝╚██╗ ██╔╝██╔════╝██║     ██╔══██╗██║   ██║██╔══██╗██╔════╝`,
        `  ██████╔╝██████╔╝ ╚███╔╝  ╚████╔╝ ██║     ██║     ███████║██║   ██║██║  ██║█████╗  `,
        `  ██╔═══╝ ██╔══██╗ ██╔██╗   ╚██╔╝  ██║     ██║     ██╔══██║██║   ██║██║  ██║██╔══╝  `,
        `  ██║     ██║  ██║██╔╝ ██╗   ██║   ╚██████╗███████╗██║  ██║╚██████╔╝██████╔╝███████╗`,
        `  ╚═╝     ╚═╝  ╚═╝╚═╝  ╚═╝   ╚═╝    ╚═════╝╚══════╝╚═╝  ╚═╝ ╚═════╝ ╚═════╝ ╚══════╝`,
        "",
        `  🔀  Proxy  →  http://localhost:${cfg.port}`,
        `  ⚡  Health  →  http://localhost:${cfg.port}/health`,
        `  🖥️   Admin   →  http://localhost:${cfg.port}/admin`,
        `  🔑  Auth token: ${cfg.proxyAuthToken}`,
        "",
        `  Providers loaded: ${cfg.providers.filter(p => p.enabled).length}`,
        `  Usage: ANTHROPIC_AUTH_TOKEN=${cfg.proxyAuthToken} ANTHROPIC_BASE_URL=http://localhost:${cfg.port} claude`,
        "",
    ];
    console.log("\x1b[36m" + lines.join("\n") + "\x1b[0m");
});
// ─── Graceful shutdown ────────────────────────────────────────────────────────
process.on("SIGTERM", () => {
    log("info", "SIGTERM received, shutting down...");
    server.close(() => process.exit(0));
});
process.on("SIGINT", () => {
    log("info", "SIGINT received, shutting down...");
    server.close(() => process.exit(0));
});
export default app;
//# sourceMappingURL=server.js.map