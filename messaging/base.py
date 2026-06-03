"""Messaging platform abstract base class."""

from abc import ABC, abstractmethod
from collections.abc import Callable, Coroutine
from typing import Any

MessageHandler = Callable[[str, str, str | None], Coroutine[Any, Any, None]]


class MessagingPlatform(ABC):
    """Abstract base for messaging platforms (Discord, Telegram, etc.)."""

    @abstractmethod
    async def start(self) -> None:
        """Start the bot and connect to the platform."""

    @abstractmethod
    async def stop(self) -> None:
        """Disconnect and clean up."""

    @abstractmethod
    async def send_message(
        self,
        chat_id: str,
        text: str,
        reply_to: str | None = None,
        parse_mode: str | None = None,
    ) -> str | None:
        """Send a message. Returns the message ID if available."""

    @abstractmethod
    async def edit_message(
        self,
        chat_id: str,
        message_id: str,
        text: str,
        parse_mode: str | None = None,
    ) -> None:
        """Edit an existing message."""

    @abstractmethod
    def on_message(self, handler: MessageHandler) -> None:
        """Register a handler for incoming user messages."""
