// ─────────────────────────────────────────────────────────────────────────────
// PrxyClaude · Request/Response Transformer
// ─────────────────────────────────────────────────────────────────────────────

import type {
  AnthropicRequest,
  AnthropicMessage,
  ContentBlock,
  OpenAIRequest,
  OpenAIMessage,
  ModelTier,
  ProviderConfig,
  SystemBlock,
  Tool,
} from "../types.ts";

// ─── Model tier detection ─────────────────────────────────────────────────────

const OPUS_PATTERNS = /opus/i;
const HAIKU_PATTERNS = /haiku/i;

export function detectTier(modelName: string): ModelTier {
  if (OPUS_PATTERNS.test(modelName)) return "opus";
  if (HAIKU_PATTERNS.test(modelName)) return "haiku";
  return "sonnet";
}

export function resolveProviderModel(
  tier: ModelTier,
  provider: ProviderConfig
): string {
  return provider.models[tier] ?? provider.defaultModel;
}

// ─── Content block → string ───────────────────────────────────────────────────

export function contentToString(
  content: string | ContentBlock[]
): string {
  if (typeof content === "string") return content;
  return content
    .filter((b) => b.type === "text")
    .map((b) => (b as { type: "text"; text: string }).text)
    .join("\n");
}

// ─── Anthropic → OpenAI ───────────────────────────────────────────────────────

export function anthropicToOpenAI(
  req: AnthropicRequest,
  targetModel: string
): OpenAIRequest {
  const messages: OpenAIMessage[] = [];

  // System prompt
  if (req.system) {
    const systemText =
      typeof req.system === "string"
        ? req.system
        : req.system.map((b: SystemBlock) => b.text).join("\n");
    messages.push({ role: "system", content: systemText });
  }

  // Conversation turns
  for (const msg of req.messages) {
    if (msg.role === "user" || msg.role === "assistant") {
       const textContent = contentToString(msg.content as string | ContentBlock[]);

      // Handle tool use from assistant
      if (Array.isArray(msg.content)) {
        const toolCalls = msg.content.filter((b: ContentBlock) => b.type === "tool_use");
        const texts = msg.content.filter((b: ContentBlock) => b.type === "text");

         if (toolCalls.length > 0 && msg.role === "assistant") {
           messages.push({
             role: "assistant",
             content: texts.length > 0 ? contentToString(texts) : null,
             tool_calls: toolCalls.map((b: ContentBlock) => {
               const tc = b as { type: "tool_use"; id: string; name: string; input: unknown };
               return {
                 id: tc.id,
                 type: "function" as const,
                 function: {
                   name: tc.name,
                   arguments: JSON.stringify(tc.input),
                 },
               };
             }),
           });
          continue;
        }

        // Handle tool results from user
        const toolResults = msg.content.filter((b: ContentBlock) => b.type === "tool_result");
        if (toolResults.length > 0 && msg.role === "user") {
          for (const tr of toolResults) {
            const r = tr as { type: "tool_result"; tool_use_id: string; content: string | ContentBlock[] };
            messages.push({
              role: "tool",
              content: typeof r.content === "string" ? r.content : contentToString(r.content),
              tool_call_id: r.tool_use_id,
            });
          }
          // Also push any text content
          if (texts.length > 0) {
            messages.push({ role: "user", content: contentToString(texts) });
          }
          continue;
        }
      }

      messages.push({ role: msg.role, content: textContent });
    }
  }

  const oaiReq: OpenAIRequest = {
    model: targetModel,
    messages,
    max_tokens: req.max_tokens ?? 8192,
    stream: req.stream ?? false,
  };

  if (req.temperature !== undefined) oaiReq.temperature = req.temperature;
  if (req.top_p !== undefined) oaiReq.top_p = req.top_p;

  if (req.tools?.length) {
    oaiReq.tools = req.tools.map((t: Tool) => ({
      type: "function" as const,
      function: {
        name: t.name,
        description: t.description,
        parameters: t.input_schema,
      },
    }));
    oaiReq.tool_choice = "auto";
  }

  return oaiReq;
}

// ─── OpenAI response → Anthropic response ────────────────────────────────────

export function openAIToAnthropic(oaiResp: Record<string, unknown>, originalModel: string): Record<string, unknown> {
  const choices = (oaiResp.choices as Array<{
    message?: { role: string; content: string | null; tool_calls?: Array<{ id: string; function: { name: string; arguments: string } }> };
    finish_reason?: string;
  }>) ?? [];

  const choice = choices[0];
  if (!choice) {
    return {
      id: `msg_${Date.now()}`,
      type: "message",
      role: "assistant",
      content: [{ type: "text", text: "" }],
      model: originalModel,
      stop_reason: "end_turn",
      usage: { input_tokens: 0, output_tokens: 0 },
    };
  }

  const content: ContentBlock[] = [];

  if (choice.message?.content) {
    content.push({ type: "text", text: choice.message.content });
  }

  if (choice.message?.tool_calls) {
    for (const tc of choice.message.tool_calls) {
      content.push({
        type: "tool_use",
        id: tc.id,
        name: tc.function.name,
        input: (() => {
          try { return JSON.parse(tc.function.arguments); }
          catch { return tc.function.arguments; }
        })(),
      });
    }
  }

  const usage = oaiResp.usage as { prompt_tokens?: number; completion_tokens?: number } ?? {};
  const finishReason = choice.finish_reason ?? "end_turn";
  const stopReason =
    finishReason === "tool_calls" ? "tool_use" :
    finishReason === "length" ? "max_tokens" :
    "end_turn";

  return {
    id: `msg_${Date.now()}`,
    type: "message",
    role: "assistant",
    content,
    model: originalModel,
    stop_reason: stopReason,
    stop_sequence: null,
    usage: {
      input_tokens: usage.prompt_tokens ?? 0,
      output_tokens: usage.completion_tokens ?? 0,
    },
  };
}

// ─── SSE line translation (streaming) ─────────────────────────────────────────

export function translateSSELine(
  line: string,
  originalModel: string,
  state: { messageId: string; inputTokens: number; outputTokens: number }
): string[] {
  if (!line.startsWith("data: ")) return [];
  const raw = line.slice(6).trim();
  if (raw === "[DONE]") {
    return [
      `event: message_delta\ndata: ${JSON.stringify({
        type: "message_delta",
        delta: { stop_reason: "end_turn", stop_sequence: null },
        usage: { output_tokens: state.outputTokens },
      })}`,
      `event: message_stop\ndata: ${JSON.stringify({ type: "message_stop" })}`,
    ];
  }

  let chunk: Record<string, unknown>;
  try {
    chunk = JSON.parse(raw);
  } catch {
    return [];
  }

  const choices = chunk.choices as Array<{
    delta?: { content?: string | null; tool_calls?: unknown[]; role?: string };
    finish_reason?: string | null;
  }> | undefined;

  if (!choices || choices.length === 0) return [];
  const delta = choices[0].delta ?? {};
  const lines: string[] = [];

  // First chunk: emit message_start
  if (delta.role === "assistant" || state.outputTokens === 0) {
    if (state.outputTokens === 0) {
      lines.push(
        `event: message_start\ndata: ${JSON.stringify({
          type: "message_start",
          message: {
            id: state.messageId,
            type: "message",
            role: "assistant",
            content: [],
            model: originalModel,
            stop_reason: null,
            usage: { input_tokens: state.inputTokens, output_tokens: 0 },
          },
        })}`,
        `event: content_block_start\ndata: ${JSON.stringify({
          type: "content_block_start",
          index: 0,
          content_block: { type: "text", text: "" },
        })}`
      );
    }
  }

  if (delta.content) {
    state.outputTokens += Math.ceil(delta.content.length / 4);
    lines.push(
      `event: content_block_delta\ndata: ${JSON.stringify({
        type: "content_block_delta",
        index: 0,
        delta: { type: "text_delta", text: delta.content },
      })}`
    );
  }

  return lines;
}
