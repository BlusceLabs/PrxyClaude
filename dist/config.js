// ─────────────────────────────────────────────────────────────────────────────
// PrxyClaude · Config Loader
// ─────────────────────────────────────────────────────────────────────────────
import fs from "fs";
import path from "path";
const CONFIG_PATH = process.env.CONFIG_PATH ?? path.join(process.cwd(), "prxyclaude.json");
// ─── Defaults ────────────────────────────────────────────────────────────────
const DEFAULT_CONFIG = {
    port: Number(process.env.PORT ?? 8082),
    adminToken: process.env.ADMIN_TOKEN ?? "admin-change-me",
    proxyAuthToken: process.env.PROXY_AUTH_TOKEN ?? "prxyclaude",
    providers: [],
    cache: {
        enabled: true,
        maxEntries: 500,
        ttlMs: 5 * 60 * 1000,
    },
    queue: {
        maxSize: 200,
        timeoutMs: 120_000,
        maxConcurrent: 10,
    },
    circuitBreaker: {
        failureThreshold: 5,
        halfOpenAfterMs: 30_000,
        successThreshold: 2,
    },
    logging: {
        level: process.env.LOG_LEVEL ?? "info",
        requests: true,
    },
    webhookUrl: process.env.WEBHOOK_URL,
};
// ─── Env → Provider Hydration ────────────────────────────────────────────────
function buildProvidersFromEnv() {
    const providers = [];
    let priority = 0;
    // ── OpenRouter ──
    const orKeys = [
        process.env.OPENROUTER_API_KEY,
        process.env.OPENROUTER_API_KEY_2,
        process.env.OPENROUTER_API_KEY_3,
        process.env.OPENROUTER_API_KEY_4,
        process.env.OPENROUTER_API_KEY_5,
    ].filter(Boolean);
    if (orKeys.length) {
        providers.push({
            id: "openrouter",
            type: "openrouter",
            label: "OpenRouter",
            baseUrl: "https://openrouter.ai/api/v1",
            apiKeys: orKeys,
            models: {
                opus: process.env.OR_MODEL_OPUS ?? "anthropic/claude-opus-4",
                sonnet: process.env.OR_MODEL_SONNET ?? "anthropic/claude-sonnet-4-5",
                haiku: process.env.OR_MODEL_HAIKU ?? "anthropic/claude-haiku-4-5",
            },
            defaultModel: process.env.OR_MODEL ?? "anthropic/claude-sonnet-4-5",
            priority: priority++,
            enabled: true,
            httpProxy: process.env.OPENROUTER_PROXY,
            rateLimitRpm: 20,
        });
    }
    // ── NVIDIA NIM ──
    const nimKeys = [
        process.env.NVIDIA_NIM_API_KEY,
        process.env.NVIDIA_NIM_API_KEY_2,
        process.env.NVIDIA_NIM_API_KEY_3,
    ].filter(Boolean);
    if (nimKeys.length) {
        providers.push({
            id: "nvidia_nim",
            type: "nvidia_nim",
            label: "NVIDIA NIM",
            baseUrl: "https://integrate.api.nvidia.com/v1",
            apiKeys: nimKeys,
            models: {
                opus: process.env.NIM_MODEL_OPUS ?? "moonshotai/kimi-k2-instruct",
                sonnet: process.env.NIM_MODEL_SONNET ?? "moonshotai/kimi-k2-thinking",
                haiku: process.env.NIM_MODEL_HAIKU ?? "stepfun-ai/step-3.5-flash",
            },
            defaultModel: process.env.NIM_MODEL ?? "stepfun-ai/step-3.5-flash",
            priority: priority++,
            enabled: true,
            httpProxy: process.env.NVIDIA_NIM_PROXY,
            rateLimitRpm: 40,
        });
    }
    // ── Groq ──
    const groqKeys = [
        process.env.GROQ_API_KEY,
        process.env.GROQ_API_KEY_2,
    ].filter(Boolean);
    if (groqKeys.length) {
        providers.push({
            id: "groq",
            type: "groq",
            label: "Groq",
            baseUrl: "https://api.groq.com/openai/v1",
            apiKeys: groqKeys,
            models: {
                opus: process.env.GROQ_MODEL_OPUS ?? "llama-3.3-70b-versatile",
                sonnet: process.env.GROQ_MODEL_SONNET ?? "llama-3.3-70b-versatile",
                haiku: process.env.GROQ_MODEL_HAIKU ?? "llama-3.1-8b-instant",
            },
            defaultModel: process.env.GROQ_MODEL ?? "llama-3.3-70b-versatile",
            priority: priority++,
            enabled: true,
            rateLimitRpm: 30,
        });
    }
    // ── Together AI ──
    const togetherKeys = [process.env.TOGETHER_API_KEY].filter(Boolean);
    if (togetherKeys.length) {
        providers.push({
            id: "together",
            type: "together",
            label: "Together AI",
            baseUrl: "https://api.together.xyz/v1",
            apiKeys: togetherKeys,
            models: {
                opus: process.env.TOGETHER_MODEL_OPUS ?? "meta-llama/Llama-3.3-70B-Instruct-Turbo",
                sonnet: process.env.TOGETHER_MODEL_SONNET ?? "meta-llama/Llama-3.3-70B-Instruct-Turbo",
                haiku: process.env.TOGETHER_MODEL_HAIKU ?? "meta-llama/Llama-3.1-8B-Instruct-Turbo",
            },
            defaultModel: process.env.TOGETHER_MODEL ?? "meta-llama/Llama-3.3-70B-Instruct-Turbo",
            priority: priority++,
            enabled: true,
        });
    }
    // ── Mistral ──
    const mistralKeys = [process.env.MISTRAL_API_KEY].filter(Boolean);
    if (mistralKeys.length) {
        providers.push({
            id: "mistral",
            type: "mistral",
            label: "Mistral",
            baseUrl: "https://api.mistral.ai/v1",
            apiKeys: mistralKeys,
            models: {
                opus: process.env.MISTRAL_MODEL_OPUS ?? "mistral-large-latest",
                sonnet: process.env.MISTRAL_MODEL_SONNET ?? "mistral-medium-latest",
                haiku: process.env.MISTRAL_MODEL_HAIKU ?? "mistral-small-latest",
            },
            defaultModel: process.env.MISTRAL_MODEL ?? "mistral-medium-latest",
            priority: priority++,
            enabled: true,
        });
    }
    // ── LM Studio ──
    if (process.env.LMSTUDIO_BASE_URL) {
        providers.push({
            id: "lmstudio",
            type: "lmstudio",
            label: "LM Studio",
            baseUrl: process.env.LMSTUDIO_BASE_URL,
            apiKeys: ["lm-studio"],
            models: {
                opus: process.env.LMSTUDIO_MODEL_OPUS ?? process.env.LMSTUDIO_MODEL ?? "local-model",
                sonnet: process.env.LMSTUDIO_MODEL_SONNET ?? process.env.LMSTUDIO_MODEL ?? "local-model",
                haiku: process.env.LMSTUDIO_MODEL_HAIKU ?? process.env.LMSTUDIO_MODEL ?? "local-model",
            },
            defaultModel: process.env.LMSTUDIO_MODEL ?? "local-model",
            priority: priority++,
            enabled: true,
        });
    }
    // ── Ollama ──
    if (process.env.OLLAMA_BASE_URL) {
        providers.push({
            id: "ollama",
            type: "ollama",
            label: "Ollama",
            baseUrl: process.env.OLLAMA_BASE_URL.replace(/\/$/, "") + "/api",
            apiKeys: ["ollama"],
            models: {
                opus: process.env.OLLAMA_MODEL_OPUS ?? process.env.OLLAMA_MODEL ?? "llama3.1",
                sonnet: process.env.OLLAMA_MODEL_SONNET ?? process.env.OLLAMA_MODEL ?? "llama3.1",
                haiku: process.env.OLLAMA_MODEL_HAIKU ?? process.env.OLLAMA_MODEL ?? "llama3.1:8b",
            },
            defaultModel: process.env.OLLAMA_MODEL ?? "llama3.1",
            priority: priority++,
            enabled: true,
        });
    }
    // ── Real Anthropic (as fallback) ──
    const anthropicKeys = [process.env.ANTHROPIC_API_KEY].filter(Boolean);
    if (anthropicKeys.length) {
        providers.push({
            id: "anthropic",
            type: "anthropic",
            label: "Anthropic (Direct)",
            baseUrl: "https://api.anthropic.com",
            apiKeys: anthropicKeys,
            models: {
                opus: "claude-opus-4-5",
                sonnet: "claude-sonnet-4-5",
                haiku: "claude-haiku-4-5-20251001",
            },
            defaultModel: "claude-sonnet-4-5",
            priority: priority++,
            enabled: true,
        });
    }
    return providers;
}
// ─── Loader ──────────────────────────────────────────────────────────────────
function loadConfig() {
    let fileConfig = {};
    if (fs.existsSync(CONFIG_PATH)) {
        try {
            fileConfig = JSON.parse(fs.readFileSync(CONFIG_PATH, "utf-8"));
        }
        catch (e) {
            console.warn("[config] Failed to parse config file, using defaults:", e);
        }
    }
    const envProviders = buildProvidersFromEnv();
    const providers = fileConfig.providers && fileConfig.providers.length > 0
        ? fileConfig.providers
        : envProviders;
    return {
        ...DEFAULT_CONFIG,
        ...fileConfig,
        providers,
    };
}
export function saveConfig(cfg) {
    fs.writeFileSync(CONFIG_PATH, JSON.stringify(cfg, null, 2));
}
// Singleton
let _config = null;
export function getConfig() {
    if (!_config)
        _config = loadConfig();
    return _config;
}
export function reloadConfig() {
    _config = loadConfig();
    return _config;
}
export function patchConfig(patch) {
    _config = { ...getConfig(), ...patch };
    saveConfig(_config);
    return _config;
}
//# sourceMappingURL=config.js.map