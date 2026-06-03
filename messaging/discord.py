"""PrxyClaude · Discord bot implementation."""

import asyncio
from typing import TYPE_CHECKING

from discord import (
    Client,
    DMChannel,
    GroupChannel,
    Intents,
    Message,
    TextChannel,
    Thread,
)
from loguru import logger

from config.settings import Settings
from messaging.base import MessagingPlatform

if TYPE_CHECKING:
    from messaging.handler import SessionRouter


class DiscordBot(MessagingPlatform):
    """Discord bot using discord.py Client."""

    def __init__(self, settings: Settings, router: SessionRouter | None = None):
        self._token = settings.discord_bot_token
        allowed = settings.allowed_discord_channels
        self._allowed_channels: set[int] = set()
        if allowed:
            for cid in allowed.split(","):
                cid = cid.strip()
                if cid:
                    self._allowed_channels.add(int(cid))

        self._handler_callback = None
        self._client: Client | None = None
        self._ready = asyncio.Event()
        self._router = router

    async def start(self) -> None:
        if not self._token:
            logger.warning("Discord: no token configured, skipping")
            return

        intents = Intents.default()
        intents.message_content = True

        self._client = Client(intents=intents)

        @self._client.event
        async def on_ready():
            client = self._client
            if client is None:
                return
            logger.info(
                f"Discord bot logged in as {client.user} "
                f"(in {len(self._allowed_channels)} channel(s))"
            )
            self._ready.set()

        @self._client.event
        async def on_message(message: Message):
            if message.author.bot:
                return
            if message.channel.id not in self._allowed_channels:
                return

            chat_id = str(message.channel.id)

            if message.content.startswith("/"):
                await self._handle_command(chat_id, message)
                return

            if message.attachments:
                text = await self._handle_attachment(message)
                if text is not None:
                    reply_to = await self._resolve_reply_to(message)
                    if self._handler_callback:
                        await self._handler_callback(chat_id, text, reply_to)
                    return

            reply_to = await self._resolve_reply_to(message)

            if self._handler_callback:
                await self._handler_callback(chat_id, message.content, reply_to)

        asyncio.create_task(self._client.start(self._token))

    async def _resolve_reply_to(self, message: Message) -> str | None:
        if (
            message.reference
            and message.reference.message_id
            and isinstance(message.channel, TextChannel)
        ):
            ref = await message.channel.fetch_message(message.reference.message_id)
            if self._client is not None and ref.author == self._client.user:
                return str(message.reference.message_id)
        return None

    async def _handle_attachment(self, message: Message) -> str | None:
        for attachment in message.attachments:
            if attachment.filename and attachment.filename.endswith(
                (".ogg", ".mp3", ".wav", ".m4a")
            ):
                try:
                    audio_data = await attachment.read()
                    from config.settings import get_settings
                    from voice.factory import create_transcription_backend

                    transcriber = create_transcription_backend(get_settings())
                    if transcriber is not None:
                        text = await transcriber.transcribe(audio_data)
                        if text:
                            return text
                except Exception as e:
                    logger.error(f"Discord voice transcription failed: {e}")
        return None

    async def stop(self) -> None:
        if self._client:
            await self._client.close()
            self._client = None

    async def send_message(
        self,
        chat_id: str,
        text: str,
        reply_to: str | None = None,
        parse_mode: str | None = None,
    ) -> str | None:
        if not self._client:
            return None
        channel = self._client.get_channel(int(chat_id))
        if not isinstance(channel, TextChannel | Thread | DMChannel | GroupChannel):
            return None

        msg = await channel.send(text)
        return str(msg.id) if msg else None

    async def edit_message(
        self,
        chat_id: str,
        message_id: str,
        text: str,
        parse_mode: str | None = None,
    ) -> None:
        if not self._client:
            return
        channel = self._client.get_channel(int(chat_id))
        if not isinstance(channel, TextChannel | Thread | DMChannel | GroupChannel):
            return
        try:
            msg = await channel.fetch_message(int(message_id))
            await msg.edit(content=text[:2000])
        except Exception as e:
            logger.debug(f"Discord edit_message failed: {e}")

    def on_message(self, handler) -> None:
        self._handler_callback = handler

    async def _handle_command(self, chat_id: str, message: Message) -> None:
        cmd = message.content.split()[0].lower()

        if self._router is None:
            await message.channel.send("❌ Router not available.")
            return

        if cmd == "/stop":
            response = await self._router.stop_task(chat_id)
            await message.channel.send(response)
        elif cmd == "/clear":
            response = await self._router.clear_sessions(
                chat_id if message.reference else None
            )
            await message.channel.send(response)
        elif cmd == "/stats":
            response = await self._router.get_stats()
            await message.channel.send(response)
