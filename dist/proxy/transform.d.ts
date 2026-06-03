import type { AnthropicRequest, ContentBlock, OpenAIRequest, ModelTier, ProviderConfig } from "./types.js";
export declare function detectTier(modelName: string): ModelTier;
export declare function resolveProviderModel(tier: ModelTier, provider: ProviderConfig): string;
export declare function contentToString(content: string | ContentBlock[]): string;
export declare function anthropicToOpenAI(req: AnthropicRequest, targetModel: string): OpenAIRequest;
export declare function openAIToAnthropic(oaiResp: Record<string, unknown>, originalModel: string): Record<string, unknown>;
export declare function translateSSELine(line: string, originalModel: string, state: {
    messageId: string;
    inputTokens: number;
    outputTokens: number;
}): string[];
//# sourceMappingURL=transform.d.ts.map