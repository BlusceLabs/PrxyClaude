// ─────────────────────────────────────────────────────────────────────────────
// PrxyClaude · Admin API  (/admin/api/*)
// ─────────────────────────────────────────────────────────────────────────────
import { Router as ExpressRouter } from "express";
import { getConfig, patchConfig } from "../config.js";
import { getCircuitStates, resetCircuit } from "../circuit-breaker.js";
import { allKeyPoolStats } from "../key-manager.js";
import { getMetrics } from "../metrics.js";
import { cacheStats, cacheClear } from "../cache.js";
import { queueStats } from "../queue.js";
import { initProviders } from "../providers/index.js";
import { log } from "../logger.js";
export function adminRouter() {
    const router = ExpressRouter();
    // ── GET /admin/api/status ──────────────────────────────────────────────────
    router.get("/status", (_req, res) => {
        const cfg = getConfig();
        res.json({
            ok: true,
            version: "1.0.0",
            uptime: Math.floor((Date.now() - getMetrics().startedAt) / 1000),
            port: cfg.port,
            providers: cfg.providers.map((p) => ({
                id: p.id,
                label: p.label,
                type: p.type,
                enabled: p.enabled,
                priority: p.priority,
                keyCount: p.apiKeys.length,
            })),
            circuits: getCircuitStates(),
            keys: allKeyPoolStats(),
            metrics: getMetrics(),
            cache: cacheStats(),
            queue: queueStats(),
        });
    });
    // ── GET /admin/api/config ──────────────────────────────────────────────────
    router.get("/config", (_req, res) => {
        const cfg = getConfig();
        // Redact API keys
        const safe = {
            ...cfg,
            providers: cfg.providers.map((p) => ({
                ...p,
                apiKeys: p.apiKeys.map((k) => `...${k.slice(-6)}`),
            })),
        };
        res.json(safe);
    });
    // ── PATCH /admin/api/config ────────────────────────────────────────────────
    router.patch("/config", (req, res) => {
        try {
            const updated = patchConfig(req.body);
            initProviders(); // reinit key pools on config change
            log("info", "[admin] config updated via API");
            res.json({ ok: true, config: updated });
        }
        catch (e) {
            res.status(400).json({ error: String(e) });
        }
    });
    // ── POST /admin/api/provider/:id/enable ───────────────────────────────────
    router.post("/provider/:id/enable", (req, res) => {
        const cfg = getConfig();
        const p = cfg.providers.find((p) => p.id === req.params.id);
        if (!p)
            return res.status(404).json({ error: "provider not found" });
        p.enabled = true;
        patchConfig(cfg);
        res.json({ ok: true });
    });
    // ── POST /admin/api/provider/:id/disable ──────────────────────────────────
    router.post("/provider/:id/disable", (req, res) => {
        const cfg = getConfig();
        const p = cfg.providers.find((p) => p.id === req.params.id);
        if (!p)
            return res.status(404).json({ error: "provider not found" });
        p.enabled = false;
        patchConfig(cfg);
        res.json({ ok: true });
    });
    // ── POST /admin/api/provider/:id/reset-circuit ────────────────────────────
    router.post("/provider/:id/reset-circuit", (req, res) => {
        resetCircuit(req.params.id);
        res.json({ ok: true });
    });
    // ── POST /admin/api/cache/clear ───────────────────────────────────────────
    router.post("/cache/clear", (_req, res) => {
        cacheClear();
        res.json({ ok: true });
    });
    // ── PUT /admin/api/provider/:id/priority ──────────────────────────────────
    router.put("/provider/:id/priority", (req, res) => {
        const cfg = getConfig();
        const p = cfg.providers.find((p) => p.id === req.params.id);
        if (!p)
            return res.status(404).json({ error: "provider not found" });
        const priority = Number(req.body.priority);
        if (isNaN(priority))
            return res.status(400).json({ error: "invalid priority" });
        p.priority = priority;
        patchConfig(cfg);
        res.json({ ok: true, priority: p.priority });
    });
    // ── GET /admin/api/metrics ────────────────────────────────────────────────
    router.get("/metrics", (_req, res) => {
        res.json(getMetrics());
    });
    return router;
}
//# sourceMappingURL=routes.js.map