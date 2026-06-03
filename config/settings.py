"""PrxyClaude · Settings (canonical config loader)"""

from __future__ import annotations

import os
from dataclasses import dataclass, field
from pathlib import Path

from pydantic_settings import BaseSettings

ENV_FILE = Path(os.getenv("PRXYCLAUDE_ENV", ".env"))


class Settings(BaseSettings):
    """All settings loaded from .env file."""

    # Server
    host: str = "0.0.0.0"
    port: int = 8082
    proxy_auth_token: str = "prxyclaude"
    admin_token: str = "admin-change-me"
    log_level: str = "info"

    # API Keys
    nvidia_nim_api_key: str = ""
    openrouter_api_key: str = ""
    lm_studio_base_url: str = ""

    # Model mappings: provider_type/model_name
    model_opus: str = "nvidia_nim/z-ai/glm4.7"
    model_sonnet: str = "nvidia_nim/z-ai/glm4.7"
    model_haiku: str = "nvidia_nim/z-ai/glm4.7"
    model: str = "nvidia_nim/z-ai/glm4.7"

    # Provider config
    provider_rate_limit: int = 40
    provider_rate_window: int = 60
    provider_max_concurrency: int = 5

    # HTTP timeouts (seconds)
    http_read_timeout: float = 120
    http_write_timeout: float = 10
    http_connect_timeout: float = 2

    # Messaging
    messaging_platform: str = "discord"
    messaging_rate_limit: int = 1
    messaging_rate_window: int = 1

    # Voice
    voice_note_enabled: bool = False
    whisper_device: str = "nvidia_nim"
    whisper_model: str = "openai/whisper-large-v3"
    hf_token: str = ""

    # Telegram
    telegram_bot_token: str = ""
    allowed_telegram_user_id: str = ""

    # Discord
    discord_bot_token: str = ""
    allowed_discord_channels: str = ""

    # Agent
    claude_workspace: str = "./agent_workspace"
    allowed_dir: str = ""

    # Optimization settings
    fast_prefix_detection: bool = True
    enable_network_probe_mock: bool = True
    enable_title_generation_skip: bool = True
    enable_suggestion_mode_skip: bool = True
    enable_filepath_extraction_mock: bool = True

    @property
    def provider_type(self) -> str:
        """Extract provider type from model string."""
        return self.parse_provider_type(self.model)

    @staticmethod
    def parse_provider_type(model_str: str) -> str:
        """Parse provider type from model string (e.g., 'nvidia_nim/model' -> 'nvidia_nim')."""
        if "/" in model_str:
            return model_str.split("/")[0]
        return "nvidia_nim"

    model_config = {
        "env_file": str(ENV_FILE),
        "env_file_encoding": "utf-8",
        "extra": "ignore",
    }


# ─── Provider Base URLs ──────────────────────────────────────────────────────

PROVIDER_BASE_URLS: dict[str, str] = {
    "nvidia_nim": "https://integrate.api.nvidia.com/v1",
    "open_router": "https://openrouter.ai/api/v1",
    "lmstudio": "",  # set from LM_STUDIO_BASE_URL
    "groq": "https://api.groq.com/openai/v1",
    "together": "https://api.together.xyz/v1",
    "mistral": "https://api.mistral.ai/v1",
    "ollama": "http://localhost:11434/api",
    "anthropic": "https://api.anthropic.com",
}


# ─── Parsed Model Mapping ───────────────────────────────────────────────────


@dataclass
class ModelMapping:
    """Parsed model string: provider_type/model_name"""

    provider_type: str
    model_name: str

    @classmethod
    def parse(cls, raw: str) -> ModelMapping:
        parts = raw.split("/", 1)
        if len(parts) != 2 or not parts[0] or not parts[1]:
            raise ValueError(
                f"Invalid model mapping format: {raw!r} (expected provider_type/model_name)"
            )
        return cls(provider_type=parts[0], model_name=parts[1])


def parse_model_mappings(settings: Settings) -> dict[str, ModelMapping]:
    """Parse all MODEL_* env vars into ModelMapping objects."""
    return {
        "opus": ModelMapping.parse(settings.model_opus),
        "sonnet": ModelMapping.parse(settings.model_sonnet),
        "haiku": ModelMapping.parse(settings.model_haiku),
        "default": ModelMapping.parse(settings.model),
    }


# ─── Runtime Config ─────────────────────────────────────────────────────────


@dataclass
class RuntimeConfig:
    """Resolved runtime configuration"""

    settings: Settings
    model_mappings: dict[str, ModelMapping] = field(default_factory=dict)
    api_keys: dict[str, list[str]] = field(default_factory=dict)
    providers: list[str] = field(default_factory=list)

    @classmethod
    def from_settings(cls, settings: Settings) -> RuntimeConfig:
        model_mappings = parse_model_mappings(settings)
        api_keys: dict[str, list[str]] = {}
        providers: list[str] = []

        if settings.nvidia_nim_api_key:
            api_keys["nvidia_nim"] = [settings.nvidia_nim_api_key]
            providers.append("nvidia_nim")

        if settings.openrouter_api_key:
            api_keys["open_router"] = [settings.openrouter_api_key]
            providers.append("open_router")

        if settings.lm_studio_base_url:
            providers.append("lmstudio")

        return cls(
            settings=settings,
            model_mappings=model_mappings,
            api_keys=api_keys,
            providers=providers,
        )


# ─── Singletons ─────────────────────────────────────────────────────────────

_settings: Settings | None = None
_config: RuntimeConfig | None = None


def get_settings() -> Settings:
    global _settings
    if _settings is None:
        _settings = Settings()
    return _settings


def get_config() -> RuntimeConfig:
    global _config
    if _config is None:
        _config = RuntimeConfig.from_settings(get_settings())
    return _config


def reload_config() -> RuntimeConfig:
    global _settings, _config
    _settings = None
    _config = None
    return get_config()


def reload_settings() -> Settings:
    global _settings
    _settings = None
    return get_settings()
