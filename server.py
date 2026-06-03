"""PrxyClaude · Server Entry Point"""

from __future__ import annotations

import sys
from pathlib import Path

# Add project root to path
sys.path.insert(0, str(Path(__file__).parent))

import uvicorn

from cli.process_registry import kill_all_best_effort
from config.settings import get_settings


def main() -> None:
    settings = get_settings()

    # Print banner
    port = settings.port
    banner = f"""
  \033[36m██████╗ ██████╗ ██╗  ██╗██╗   ██╗ ██████╗██╗      █████╗ ██╗   ██╗██████╗ ███████╗
  ██╔══██╗██╔══██╗╚██╗██╔╝╚██╗ ██╔╝██╔════╝██║     ██╔══██╗██║   ██║██╔══██╗██╔════╝
  ██████╔╝██████╔╝ ╚███╔╝  ╚████╔╝ ██║     ██║     ███████║██║   ██║██║  ██║█████╗
  ██╔═══╝ ██╔══██╗ ██╔██╗   ╚██╔╝  ██║     ██║     ██╔══██║██║   ██║██║  ██║██╔══╝
  ██║     ██║  ██║██╔╝ ██╗   ██║   ╚██████╗███████╗██║  ██║╚██████╔╝██████╔╝███████╗
  ╚═╝     ╚═╝  ╚═╝╚═╝  ╚═╝   ╚═╝    ╚═════╝╚══════╝╚═╝  ╚═╝ ╚═════╝ ╚═════╝ ╚══════╝\033[0m

  \U0001f500  Proxy  \u2192  http://localhost:{port}
  \u26a1  Health  \u2192  http://localhost:{port}/health
  \U0001f5a5\ufe0f   Admin   \u2192  http://localhost:{port}/admin
  \U0001f511  Auth token: {settings.proxy_auth_token}

  Usage: ANTHROPIC_AUTH_TOKEN={settings.proxy_auth_token} ANTHROPIC_BASE_URL=http://localhost:{port} claude
"""
    print(banner)

    try:
        # timeout_graceful_shutdown ensures uvicorn doesn't hang on task cleanup.
        uvicorn.run(
            "api.app:app",
            host=settings.host,
            port=settings.port,
            log_level="debug",
            timeout_graceful_shutdown=5,
        )
    finally:
        # Safety net: cleanup subprocesses if lifespan shutdown doesn't fully run.
        kill_all_best_effort()


if __name__ == "__main__":
    main()
