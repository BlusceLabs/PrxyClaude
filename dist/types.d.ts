export type ModelTier = "opus" | "sonnet" | "haiku";
export type CircuitState = "closed" | "open" | "half-open";
export type ProviderType = "openrouter" | "nvidia_nim" | "groq" | "openai_compat" | "lmstudio" | "ollama" | "mistral" | "together" | "anthropic";
export interface ProviderConfig {
    id: string;
    type: ProviderType;
    label: string;
    baseUrl: string;
    /** Array of API keys – rotated round-robin */
    apiKeys: string[];
    /** Per-tier model overrides */
    models: Partial<Record<ModelTier, string>>;
    /** Fallback model if tier not mapped */
    defaultModel: string;
    priority: number;
    enabled: boolean;
    /** Optional HTTP proxy (e.g. socks5://user:pass@host:port) */
    httpProxy?: string;
    maxConcurrent?: number;
    rateLimitRpm?: number;
    /** Extra headers to forward to provider */
    extraHeaders?: Record<string, string>;
}
export interface CircuitBreakerState {
    providerId: string;
    state: CircuitState;
    failures: number;
    lastFailureAt: number | null;
    lastSuccessAt: number | null;
    /** When to try again (half-open transition) */
    retryAfter: number | null;
    totalRequests: number;
    totalFailures: number;
}
export interface KeySlot {
    key: string;
    usageCount: number;
    errorCount: number;
    lastUsedAt: number | null;
    /** Temporarily banned until this epoch ms */
    bannedUntil: number | null;
    rateLimitHits: number;
}
export interface AnthropicMessage {
    role: "user" | "assistant";
    content: string | ContentBlock[];
}
export type ContentBlock = {
    type: "text";
    text: string;
} | {
    type: "image";
    source: ImageSource;
} | {
    type: "tool_use";
    id: string;
    name: string;
    input: unknown;
} | {
    type: "tool_result";
    tool_use_id: string;
    content: string | ContentBlock[];
} | {
    type: "thinking";
    thinking: string;
} | {
    type: "redacted_thinking";
    data: string;
};
export interface ImageSource {
    type: "base64" | "url";
    media_type: string;
    data?: string;
    url?: string;
}
export interface Tool {
    name: string;
    description?: string;
    input_schema: Record<string, unknown>;
}
export interface AnthropicRequest {
    model: string;
    messages: AnthropicMessage[];
    max_tokens?: number;
    temperature?: number;
    top_p?: number;
    top_k?: number;
    stream?: boolean;
    system?: string | SystemBlock[];
    tools?: Tool[];
    tool_choice?: ToolChoice;
    thinking?: {
        type: "enabled";
        budget_tokens: number;
    };
    metadata?: Record<string, unknown>;
}
export interface SystemBlock {
    type: "text";
    text: string;
    cache_control?: {
        type: "ephemeral";
    };
}
export interface ToolChoice {
    type: "auto" | "any" | "tool" | "none";
    name?: string;
}
export interface OpenAIMessage {
    role: "system" | "user" | "assistant" | "tool";
    content: string | OpenAIContentPart[] | null;
    name?: string;
    tool_calls?: OpenAIToolCall[];
    tool_call_id?: string;
}
export interface OpenAIContentPart {
    type: "text" | "image_url";
    text?: string;
    image_url?: {
        url: string;
        detail?: string;
    };
}
export interface OpenAIToolCall {
    id: string;
    type: "function";
    function: {
        name: string;
        arguments: string;
    };
}
export interface OpenAIRequest {
    model: string;
    messages: OpenAIMessage[];
    max_tokens?: number;
    temperature?: number;
    top_p?: number;
    stream?: boolean;
    tools?: OpenAITool[];
    tool_choice?: string | {
        type: string;
        function?: {
            name: string;
        };
    };
}
export interface OpenAITool {
    type: "function";
    function: {
        name: string;
        description?: string;
        parameters: Record<string, unknown>;
    };
}
export interface QueuedRequest {
    id: string;
    priority: number;
    tier: ModelTier;
    createdAt: number;
    timeoutAt: number;
    resolve: (value: unknown) => void;
    reject: (err: Error) => void;
    execute: () => Promise<unknown>;
}
export interface ProviderMetrics {
    providerId: string;
    requests: number;
    successes: number;
    failures: number;
    cachedHits: number;
    totalTokensIn: number;
    totalTokensOut: number;
    avgLatencyMs: number;
    latencies: number[];
    lastErrorMsg?: string;
    lastUsedAt?: number;
}
export interface GlobalMetrics {
    totalRequests: number;
    cachedRequests: number;
    queuedRequests: number;
    providers: Record<string, ProviderMetrics>;
    startedAt: number;
}
export interface PrxyConfig {
    port: number;
    adminToken: string;
    proxyAuthToken: string;
    providers: ProviderConfig[];
    cache: {
        enabled: boolean;
        maxEntries: number;
        ttlMs: number;
    };
    queue: {
        maxSize: number;
        timeoutMs: number;
        maxConcurrent: number;
    };
    circuitBreaker: {
        failureThreshold: number;
        halfOpenAfterMs: number;
        successThreshold: number;
    };
    logging: {
        level: "debug" | "info" | "warn" | "error";
        requests: boolean;
    };
    webhookUrl?: string;
}
//# sourceMappingURL=types.d.ts.map