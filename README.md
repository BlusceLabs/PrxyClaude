<div align="center">

# PxyClaude

Use Claude Code CLI, VS Code, JetBrains ACP, or chat bots through your own Anthropic-compatible proxy.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=for-the-badge)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)

PxyClaude routes Anthropic Messages API traffic from Claude Code to OpenAI, Anthropic, NVIDIA NIM, OpenRouter, DeepSeek, z.ai, Kimi, Gemini, Cloudflare AI Gateway, LM Studio, llama.cpp, and Ollama. It keeps Claude Code's client-side protocol stable while letting you choose free, paid, or local models.

[Quick Start](#quick-start) | [Providers](#choose-a-provider) | [Clients](#connect-claude-code) | [Troubleshooting](#troubleshooting) | [Development](#development)

</div>

## What You Get

- Drop-in proxy for Claude Code's Anthropic API calls.
- Twelve provider backends: OpenAI, Anthropic, NVIDIA NIM, OpenRouter, DeepSeek, z.ai, Kimi, Gemini, Cloudflare AI Gateway, LM Studio, llama.cpp, and Ollama.
- Per-model routing: send Opus, Sonnet, Haiku, and fallback traffic to different providers.
- Native Claude Code `/model` picker support through the proxy's `/v1/models` endpoint.
- Streaming, tool use, reasoning/thinking block handling, and local request optimizations.
- Built in Rust for high performance, low memory usage, and fast startup.

## Quick Start

### 1. Install Rust

Install [Rust](https://www.rust-lang.org/tools/install) if not already installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. Clone And Configure

```bash
git clone https://github.com/BlusceLabs/PxyClaude.git
cd PxyClaude
cp .env.example .env
```

Edit `.env` and choose one provider. For the default NVIDIA NIM path:

```dotenv
NVIDIA_NIM_API_KEY="nvapi-your-key"
MODEL="nvidia_nim/z-ai/glm4.7"
ANTHROPIC_AUTH_TOKEN="proxycc"
```

Use any local secret for `ANTHROPIC_AUTH_TOKEN`; Claude Code will send the same value back to this proxy. Leave it empty only for local/private testing.

### 3. Build And Start The Proxy

```bash
cargo build --release
cargo run --release
```

The proxy starts on `http://localhost:8082` by default.

### 4. Run Claude Code

Point `ANTHROPIC_BASE_URL` at the proxy root. Do not append `/v1`. Set `CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY=1` if you use `/model` to list models from this proxy.

Bash:

```bash
ANTHROPIC_AUTH_TOKEN="proxycc" ANTHROPIC_BASE_URL="http://localhost:8082" CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY=1 claude
```

PowerShell:

```powershell
$env:ANTHROPIC_AUTH_TOKEN="proxycc"; $env:ANTHROPIC_BASE_URL="http://localhost:8082"; $env:CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY="1"; claude
```

## Choose A Provider

Model values use this format:

```text
provider_id/model/name
```

`MODEL` is the fallback. `MODEL_OPUS`, `MODEL_SONNET`, and `MODEL_HAIKU` override routing for requests that Claude Code sends for those tiers.

| Provider | Prefix | Transport | Key | Default base URL |
| --- | --- | --- | --- | --- |
| NVIDIA NIM | `nvidia_nim/...` | OpenAI chat translation | `NVIDIA_NIM_API_KEY` | `https://integrate.api.nvidia.com/v1` |
| OpenAI | `openai/...` | OpenAI chat translation | `OPENAI_API_KEY` | `https://api.openai.com/v1` |
| Kimi | `kimi/...` | OpenAI chat translation | `KIMI_API_KEY` | `https://api.moonshot.ai/v1` |
| OpenRouter | `open_router/...` | Anthropic Messages | `OPENROUTER_API_KEY` | `https://openrouter.ai/api/v1` |
| DeepSeek | `deepseek/...` | Anthropic Messages | `DEEPSEEK_API_KEY` | `https://api.deepseek.com/anthropic` |
| z.ai | `z_ai/...` | OpenAI chat translation | `ZAI_API_KEY` | `https://api.z.ai/api/paas/v4` |
| Cloudflare AI Gateway | `cloudflare_gateway/...` | Anthropic Messages | `CF_AIG_TOKEN` | `https://gateway.ai.cloudflare.com/v1/.../anthropic/v1` |
| Gemini | `gemini/...` | OpenAI chat translation | `GEMINI_API_KEY` | `https://generativelanguage.googleapis.com/v1beta/openai` |
| Anthropic | `anthropic/...` | Anthropic Messages | `ANTHROPIC_API_KEY` | `https://api.anthropic.com/v1` |
| LM Studio | `lmstudio/...` | Anthropic Messages | none | `http://localhost:1234/v1` |
| llama.cpp | `llamacpp/...` | Anthropic Messages | none | `http://localhost:8080/v1` |
| Ollama | `ollama/...` | Anthropic Messages | none | `http://localhost:11434` |

<details>
<summary><b>NVIDIA NIM</b></summary>

Get a key at [build.nvidia.com/settings/api-keys](https://build.nvidia.com/settings/api-keys).

```dotenv
NVIDIA_NIM_API_KEY="nvapi-your-key"
MODEL="nvidia_nim/z-ai/glm4.7"
```

Browse models at [build.nvidia.com](https://build.nvidia.com/explore/discover).

</details>

<details>
<summary><b>OpenRouter</b></summary>

Get a key at [openrouter.ai/keys](https://openrouter.ai/keys).

```dotenv
OPENROUTER_API_KEY="sk-or-your-key"
MODEL="open_router/stepfun/step-3.5-flash:free"
```

Browse [all models](https://openrouter.ai/models) or [free models](https://openrouter.ai/collections/free-models).

</details>

<details>
<summary><b>DeepSeek</b></summary>

Get a key at [platform.deepseek.com/api_keys](https://platform.deepseek.com/api_keys).

```dotenv
DEEPSEEK_API_KEY="your-deepseek-key"
MODEL="deepseek/deepseek-chat"
```

</details>

<details>
<summary><b>LM Studio</b></summary>

Start LM Studio's local server, load a model, then configure:

```dotenv
LM_STUDIO_BASE_URL="http://localhost:1234/v1"
MODEL="lmstudio/your-loaded-model"
```

</details>

<details>
<summary><b>llama.cpp</b></summary>

Start `llama-server` with an Anthropic-compatible `/v1/messages` endpoint:

```dotenv
LLAMACPP_BASE_URL="http://localhost:8080/v1"
MODEL="llamacpp/local-model"
```

</details>

<details>
<summary><b>Ollama</b></summary>

Run Ollama and pull a model:

```bash
ollama pull llama3.1
ollama serve
```

```dotenv
OLLAMA_BASE_URL="http://localhost:11434"
MODEL="ollama/llama3.1"
```

</details>

<details>
<summary><b>OpenAI</b></summary>

Get a key at [platform.openai.com/api-keys](https://platform.openai.com/api-keys).

```dotenv
OPENAI_API_KEY="sk-your-key"
MODEL="openai/gpt-4o"
```

</details>

<details>
<summary><b>Anthropic Direct</b></summary>

Get a key at [console.anthropic.com/settings/keys](https://console.anthropic.com/settings/keys).

```dotenv
ANTHROPIC_API_KEY="sk-ant-your-key"
MODEL="anthropic/claude-sonnet-4-20250514"
```

</details>

<details>
<summary><b>z.ai</b></summary>

Get a key at [z.ai/manage-apikey/apikey-list](https://z.ai/manage-apikey/apikey-list).

```dotenv
ZAI_API_KEY="your-zai-key"
MODEL="z_ai/glm-4.7-flash"
```

</details>

<details>
<summary><b>Cloudflare AI Gateway</b></summary>

```dotenv
CF_AIG_TOKEN="your-cf-token"
CF_GATEWAY_BASE_URL="https://gateway.ai.cloudflare.com/v1/ACCOUNT_ID/GATEWAY_NAME/anthropic/v1"
MODEL="cloudflare_gateway/claude-sonnet-4-20250514"
```

</details>

<details>
<summary><b>Gemini</b></summary>

Get a key at [aistudio.google.com/apikey](https://aistudio.google.com/apikey).

```dotenv
GEMINI_API_KEY="your-gemini-key"
MODEL="gemini/gemini-2.5-flash"
```

</details>

<details>
<summary><b>Mix providers by model tier</b></summary>

Each tier can use a different provider:

```dotenv
NVIDIA_NIM_API_KEY="nvapi-your-key"
OPENROUTER_API_KEY="sk-or-your-key"

MODEL_OPUS="nvidia_nim/moonshotai/kimi-k2.5"
MODEL_SONNET="open_router/deepseek/deepseek-r1-0528:free"
MODEL_HAIKU="lmstudio/unsloth/GLM-4.7-Flash-GGUF"
MODEL="nvidia_nim/z-ai/glm4.7"
```

</details>

## Connect Claude Code

### Claude Code CLI

```bash
ANTHROPIC_AUTH_TOKEN="proxycc" ANTHROPIC_BASE_URL="http://localhost:8082" CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY=1 claude
```

### VS Code Extension

Open Settings, search for `claude-code.environmentVariables`, choose **Edit in settings.json**, and add:

```json
"claudeCode.environmentVariables": [
  { "name": "ANTHROPIC_BASE_URL", "value": "http://localhost:8082" },
  { "name": "ANTHROPIC_AUTH_TOKEN", "value": "proxycc" },
  { "name": "CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY", "value": "1" }
]
```

### JetBrains ACP

Edit the installed Claude ACP config:

- Windows: `C:\Users\%USERNAME%\AppData\Roaming\JetBrains\acp-agents\installed.json`
- Linux/macOS: `~/.jetbrains/acp.json`

```json
"env": {
  "ANTHROPIC_BASE_URL": "http://localhost:8082",
  "ANTHROPIC_AUTH_TOKEN": "proxycc",
  "CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY": "1"
}
```

### Model Picker

Claude Code 2.1.126 or later can populate `/model` from this proxy's Gateway `/v1/models` response when `ANTHROPIC_BASE_URL` points here. Newer releases require `CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY=1`.

## Configuration Reference

[`.env.example`](.env.example) is the canonical list of variables.

### Model Routing

```dotenv
MODEL="nvidia_nim/z-ai/glm4.7"
MODEL_OPUS=
MODEL_SONNET=
MODEL_HAIKU=
ENABLE_MODEL_THINKING=true
ENABLE_OPUS_THINKING=
ENABLE_SONNET_THINKING=
ENABLE_HAIKU_THINKING=
```

### Provider Keys And URLs

```dotenv
NVIDIA_NIM_API_KEY=""
OPENAI_API_KEY=""
ANTHROPIC_API_KEY=""
OPENROUTER_API_KEY=""
DEEPSEEK_API_KEY=""
KIMI_API_KEY=""
ZAI_API_KEY=""
CF_AIG_TOKEN=""
GEMINI_API_KEY=""
LM_STUDIO_BASE_URL="http://localhost:1234/v1"
LLAMACPP_BASE_URL="http://localhost:8080/v1"
OLLAMA_BASE_URL="http://localhost:11434"
```

### Security

```dotenv
ANTHROPIC_AUTH_TOKEN=
```

## Troubleshooting

### Claude Code says `undefined ... input_tokens`

- Update to the latest version.
- `ANTHROPIC_BASE_URL` is `http://localhost:8082`, not `http://localhost:8082/v1`.
- The proxy is returning Server-Sent Events for `/v1/messages`.

### llama.cpp or LM Studio returns HTTP 400

- The local server supports `POST /v1/messages`.
- The model supports the requested context length and tools.
- llama.cpp was started with enough `--ctx-size`.

### Provider disconnects during streaming

Reduce concurrency, raise timeouts, or retry later.

## How It Works

```text
Claude Code CLI / IDE
        |
        | Anthropic Messages API
        v
PxyClaude proxy (:8082)
        |
        | provider-specific request/stream adapter
        v
OpenAI / Anthropic / NVIDIA NIM / OpenRouter / DeepSeek / z.ai
Kimi / Gemini / Cloudflare AI Gateway / LM Studio / llama.cpp / Ollama
```

- Axum exposes Anthropic-compatible routes: `/v1/messages`, `/v1/messages/count_tokens`, `/v1/models`.
- Model routing resolves the Claude model name to `MODEL_OPUS`, `MODEL_SONNET`, `MODEL_HAIKU`, or `MODEL`.
- **OpenAI-chat providers** (OpenAI, NVIDIA NIM, Kimi, z.ai, Gemini) use OpenAI chat streaming translated into Anthropic SSE.
- **Native Anthropic providers** (Anthropic, OpenRouter, DeepSeek, Cloudflare AI Gateway, LM Studio, llama.cpp, Ollama) use Anthropic Messages style transports.
- The proxy normalizes thinking blocks, tool calls, token usage metadata, and provider errors.

## Development

### Prerequisites

- [Rust 1.75+](https://www.rust-lang.org/tools/install)

### Project Structure

```text
PxyClaude/
├── src/
│   ├── bin/                 # Binary entry points
│   ├── api/                 # HTTP server, routes, middleware
│   │   ├── server/          # Axum server, routes, gateway model IDs
│   │   ├── web_tools/       # Web fetch, search, egress policy
│   │   └── models/          # API data models
│   ├── core/                # Shared Anthropic protocol helpers
│   │   └── anthropic/       # SSE, conversion, thinking, tools, tokens
│   ├── providers/           # Provider transports, registry, rate limiting
│   └── cli/                 # CLI session and process management
├── Cargo.toml               # Rust dependencies
└── .env                     # Configuration
```

### Commands

```bash
cargo check          # Type-check
cargo test           # Run tests
cargo build          # Debug build
cargo build --release # Release build
```

### Adding Providers

1. Implement the `Provider` trait in `src/providers/`.
2. Register in `src/providers/provider_catalog.rs` and `src/providers/registry.rs`.
3. Add tests.

## Contributing

- Report bugs and feature requests in [Issues](https://github.com/BlusceLabs/PxyClaude/issues).
- Keep changes small and covered by focused tests.
- Run `cargo check` and `cargo test` before opening a pull request.

## License

MIT License. See [LICENSE](LICENSE) for details.
