import type { AnthropicRequest, ProviderConfig, ModelTier } from "../types.js";
export declare function initProviders(): void;
export declare function callProvider(provider: ProviderConfig, req: AnthropicRequest, tier: ModelTier): Promise<Record<string, unknown>>;
export declare function callProviderStream(provider: ProviderConfig, req: AnthropicRequest, tier: ModelTier): Promise<ReadableStream<Uint8Array>>;
export declare function dispatch(req: AnthropicRequest): Promise<Record<string, unknown>>;
export declare function dispatchStream(req: AnthropicRequest): Promise<ReadableStream<Uint8Array>>;
//# sourceMappingURL=index.d.ts.map