// ─────────────────────────────────────────────────────────────────────────────
// PrxyClaude · Provider Registry
// ─────────────────────────────────────────────────────────────────────────────

import { getConfig } from "../config.js";
import { isCircuitOpen, recordSuccess, recordFailure } from "../circuit-breaker.js";
import { nextKey, banKey, markKeyError, initKeyPool } from "../key-manager.js";
import { recordProviderSuccess, recordProviderFailure } from "../metrics.js";
import { anthropicToOpenAI, openAIToAnthropic, detectTier, resolveProviderModel } from "../proxy/transform.ts";
import type { AnthropicRequest, ProviderConfig, ModelTier } from "../types.js";
import { log } from "../logger.js";

// ─── Init key pools for all providers ────────────────────────────────────────

export function initProviders(): void {
  const cfg = getConfig();
  for (const p of cfg.providers) {
    if (p.apiKeys.length) initKeyPool(p.id, p.apiKeys);
  }
  log("info", `[registry] Loaded ${cfg.providers.length} provider(s)`);
}

// ─── Provider selection ───────────────────────────────────────────────────────

function getEnabledProviders(): ProviderConfig[] {
  return getConfig()
    .providers.filter((p) => p.enabled)
    .sort((a, b) => a.priority - b.priority);
}

// ─── Fetch with optional HTTP proxy ──────────────────────────────────────────

async function fetchWithProxy(
  url: string,
  init: RequestInit,
  _httpProxy?: string
): Promise<Response> {
  // Node 18+ native fetch doesn't support socks5 proxy natively.
  // For production, use undici or node-fetch with ProxyAgent.
  // We keep it simple here — proxy support can be wired via env HTTP_PROXY.
  return fetch(url, init);
}

// ─── Core provider call (non-streaming) ──────────────────────────────────────

export async function callProvider(
  provider: ProviderConfig,
  req: AnthropicRequest,
  tier: ModelTier
): Promise<Record<string, unknown>> {
  const targetModel = resolveProviderModel(tier, provider);
  const apiKey = nextKey(provider.id);

  if (!apiKey) {
    throw new Error(`${provider.label}: no available API keys`);
  }

  const start = Date.now();

  // ── Anthropic native ──
  if (provider.type === "anthropic") {
    const body = { ...req, model: targetModel };
    const resp = await fetchWithProxy(
      `${provider.baseUrl}/v1/messages`,
      {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "x-api-key": apiKey,
          "anthropic-version": "2023-06-01",
          ...provider.extraHeaders,
        },
        body: JSON.stringify(body),
      },
      provider.httpProxy
    );

    if (resp.status === 429) {
      const retryAfter = parseInt(resp.headers.get("retry-after") ?? "60") * 1000;
      banKey(provider.id, apiKey, retryAfter);
      throw new Error(`429: rate limited by ${provider.label}`);
    }

    if (!resp.ok) {
      const errText = await resp.text().catch(() => "unknown error");
      markKeyError(provider.id, apiKey);
      throw new Error(`${provider.label} HTTP ${resp.status}: ${errText}`);
    }

    const data = await resp.json() as Record<string, unknown>;
    const latency = Date.now() - start;
    const usage = data.usage as { input_tokens?: number; output_tokens?: number } ?? {};
    recordProviderSuccess(provider.id, latency, usage.input_tokens, usage.output_tokens);
    return data;
  }

  // ── Ollama native ──
  if (provider.type === "ollama") {
    const ollamaBody = {
      model: targetModel,
      messages: anthropicToOpenAI(req, targetModel).messages,
      stream: false,
      options: {
        temperature: req.temperature,
        top_p: req.top_p,
      },
    };
    const resp = await fetchWithProxy(
      `${provider.baseUrl}/chat`,
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(ollamaBody),
      }
    );

    if (!resp.ok) throw new Error(`Ollama HTTP ${resp.status}`);
    const data = await resp.json() as { message?: { content?: string } };
    const latency = Date.now() - start;
    recordProviderSuccess(provider.id, latency, 0, 0);
    return {
      id: `msg_${Date.now()}`,
      type: "message",
      role: "assistant",
      content: [{ type: "text", text: data.message?.content ?? "" }],
      model: req.model,
      stop_reason: "end_turn",
      usage: { input_tokens: 0, output_tokens: 0 },
    };
  }

  // ── OpenAI-compatible (OpenRouter, NVIDIA NIM, Groq, Together, Mistral, LMStudio) ──
  const oaiReq = anthropicToOpenAI(req, targetModel);

  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    Authorization: `Bearer ${apiKey}`,
    ...provider.extraHeaders,
  };

  if (provider.type === "openrouter") {
    headers["HTTP-Referer"] = "https://github.com/prxyclaude/prxyclaude";
    headers["X-Title"] = "PrxyClaude";
  }

  const resp = await fetchWithProxy(
    `${provider.baseUrl}/chat/completions`,
    {
      method: "POST",
      headers,
      body: JSON.stringify(oaiReq),
    },
    provider.httpProxy
  );

  if (resp.status === 429) {
    const retryAfter = parseInt(resp.headers.get("retry-after") ?? "60") * 1000;
    banKey(provider.id, apiKey, retryAfter);
    throw new Error(`429: rate limited by ${provider.label}`);
  }

  if (!resp.ok) {
    const errText = await resp.text().catch(() => "");
    markKeyError(provider.id, apiKey);
    throw new Error(`${provider.label} HTTP ${resp.status}: ${errText.slice(0, 200)}`);
  }

  const data = await resp.json() as Record<string, unknown>;
  const latency = Date.now() - start;
  const usage = data.usage as { prompt_tokens?: number; completion_tokens?: number } ?? {};
  recordProviderSuccess(provider.id, latency, usage.prompt_tokens, usage.completion_tokens);

  return openAIToAnthropic(data, req.model);
}

// ─── Core provider call (streaming) ──────────────────────────────────────────

export async function callProviderStream(
  provider: ProviderConfig,
  req: AnthropicRequest,
  tier: ModelTier
): Promise<ReadableStream<Uint8Array>> {
  const targetModel = resolveProviderModel(tier, provider);
  const apiKey = nextKey(provider.id);

  if (!apiKey) throw new Error(`${provider.label}: no available API keys`);

  const start = Date.now();

  if (provider.type === "anthropic") {
    const body = { ...req, model: targetModel, stream: true };
    const resp = await fetchWithProxy(
      `${provider.baseUrl}/v1/messages`,
      {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "x-api-key": apiKey,
          "anthropic-version": "2023-06-01",
          ...provider.extraHeaders,
        },
        body: JSON.stringify(body),
      }
    );

    if (!resp.ok || !resp.body) {
      const errText = await resp.text().catch(() => "");
      throw new Error(`${provider.label} HTTP ${resp.status}: ${errText.slice(0, 200)}`);
    }

    recordProviderSuccess(provider.id, Date.now() - start);
    return resp.body;
  }

  // OpenAI-compatible streaming
  const oaiReq = { ...anthropicToOpenAI(req, targetModel), stream: true };

  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    Authorization: `Bearer ${apiKey}`,
    ...provider.extraHeaders,
  };
  if (provider.type === "openrouter") {
    headers["HTTP-Referer"] = "https://github.com/prxyclaude/prxyclaude";
    headers["X-Title"] = "PrxyClaude";
  }

  const resp = await fetchWithProxy(
    `${provider.baseUrl}/chat/completions`,
    { method: "POST", headers, body: JSON.stringify(oaiReq) },
    provider.httpProxy
  );

  if (!resp.ok || !resp.body) {
    if (resp.status === 429) {
      banKey(provider.id, apiKey, 60_000);
      throw new Error(`429: rate limited by ${provider.label}`);
    }
    const errText = await resp.text().catch(() => "");
    throw new Error(`${provider.label} HTTP ${resp.status}: ${errText.slice(0, 200)}`);
  }

  recordProviderSuccess(provider.id, Date.now() - start);

  // Re-encode the OpenAI SSE stream as Anthropic SSE
  const { translateSSELine } = await import("../proxy/transform.ts");
  const msgId = `msg_${Date.now()}`;
  const state = { messageId: msgId, inputTokens: 0, outputTokens: 0 };
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  let buffer = "";
  let headerSent = false;

  const transformed = new TransformStream<Uint8Array, Uint8Array>({
    transform(chunk, ctrl) {
      buffer += decoder.decode(chunk, { stream: true });
      const lines = buffer.split("\n");
      buffer = lines.pop() ?? "";

      for (const line of lines) {
        const trimmed = line.trimEnd();
        if (!trimmed) continue;

        const translated = translateSSELine(trimmed, req.model, state);
        for (const tl of translated) {
          ctrl.enqueue(encoder.encode(tl + "\n\n"));
        }
      }
    },
    flush(ctrl) {
      if (buffer.trim()) {
        const translated = translateSSELine(buffer.trim(), req.model, state);
        for (const tl of translated) {
          ctrl.enqueue(encoder.encode(tl + "\n\n"));
        }
      }
    },
  });

  return resp.body.pipeThrough(transformed);
}

// ─── Failover orchestrator ────────────────────────────────────────────────────

export async function dispatch(req: AnthropicRequest): Promise<Record<string, unknown>> {
  const tier = detectTier(req.model);
  const providers = getEnabledProviders();
  const errors: string[] = [];

  for (const provider of providers) {
    if (isCircuitOpen(provider.id)) {
      log("debug", `[dispatch] skipping ${provider.id} (circuit open)`);
      continue;
    }

    try {
      log("debug", `[dispatch] trying ${provider.id} tier=${tier}`);
      const result = await callProvider(provider, req, tier);
      recordSuccess(provider.id);
      return result;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      errors.push(`[${provider.id}] ${msg}`);
      recordFailure(provider.id, msg);
      recordProviderFailure(provider.id, msg);
      log("warn", `[dispatch] ${provider.id} failed: ${msg}`);
    }
  }

  throw new Error(`All providers failed:\n${errors.join("\n")}`);
}

export async function dispatchStream(req: AnthropicRequest): Promise<ReadableStream<Uint8Array>> {
  const tier = detectTier(req.model);
  const providers = getEnabledProviders();
  const errors: string[] = [];

  for (const provider of providers) {
    if (isCircuitOpen(provider.id)) continue;

    try {
      const stream = await callProviderStream(provider, req, tier);
      recordSuccess(provider.id);
      return stream;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      errors.push(`[${provider.id}] ${msg}`);
      recordFailure(provider.id, msg);
      recordProviderFailure(provider.id, msg);
      log("warn", `[dispatch-stream] ${provider.id} failed: ${msg}`);
    }
  }

  throw new Error(`All providers failed:\n${errors.join("\n")}`);
}
