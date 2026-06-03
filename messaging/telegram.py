"""PrxyClaude · Telegram bot implementation."""

from typing import TYPE_CHECKING

from loguru import logger
from telegram import Bot, Update
from telegram.ext import Application, CommandHandler, MessageHandler, filters

from config.settings import Settings
from messaging.base import MessagingPlatform

if TYPE_CHECKING:
    from messaging.handler import SessionRouter


class TelegramBot(MessagingPlatform):
    """Telegram bot using python-telegram-bot."""

    def __init__(self, settings: Settings, router: SessionRouter | None = None):
        self._token = settings.telegram_bot_token
        allowed = settings.allowed_telegram_user_id
        self._allowed_users: set[int] = set()
        if allowed:
            for uid in allowed.split(","):
                uid = uid.strip()
                if uid:
                    self._allowed_users.add(int(uid))

        self._handler_callback = None
        self._app: Application | None = None
        self._bot: Bot | None = None
        self._router = router

    async def start(self) -> None:
        if not self._token:
            logger.warning("Telegram: no token configured, skipping")
            return

        self._app = Application.builder().token(self._token).build()
        self._bot = self._app.bot

        self._app.add_handler(
            MessageHandler(filters.TEXT & ~filters.COMMAND, self._on_text)
        )
        if filters.VOICE:
            self._app.add_handler(MessageHandler(filters.VOICE, self._on_voice))
        self._app.add_handler(CommandHandler("stop", self._on_stop))
        self._app.add_handler(CommandHandler("clear", self._on_clear))
        self._app.add_handler(CommandHandler("stats", self._on_stats))

        await self._app.initialize()
        await self._app.start()
        logger.info("Telegram bot started")

    async def stop(self) -> None:
        if self._app:
            await self._app.stop()
            await self._app.shutdown()

    async def send_message(
        self,
        chat_id: str,
        text: str,
        reply_to: str | None = None,
        parse_mode: str | None = None,
    ) -> str | None:
        if not self._bot:
            return None
        msg = await self._bot.send_message(
            chat_id=int(chat_id),
            text=text,
            reply_to_message_id=int(reply_to) if reply_to else None,
            parse_mode=parse_mode,
        )
        return str(msg.message_id) if msg else None

    async def edit_message(
        self,
        chat_id: str,
        message_id: str,
        text: str,
        parse_mode: str | None = None,
    ) -> None:
        if not self._bot:
            return
        try:
            await self._bot.edit_message_text(
                chat_id=int(chat_id),
                message_id=int(message_id),
                text=text,
                parse_mode=parse_mode,
            )
        except Exception as e:
            logger.debug(f"Telegram edit_message failed: {e}")

    def on_message(self, handler) -> None:
        self._handler_callback = handler

    async def _on_text(self, update: Update, _context) -> None:
        if not update.message or not update.message.text:
            return
        user_id = update.effective_user.id if update.effective_user else None
        if user_id and self._allowed_users and user_id not in self._allowed_users:
            return

        chat = update.effective_chat
        if chat is None:
            return
        chat_id = str(chat.id)
        text = update.message.text
        reply_to: str | None = None

        if update.message.reply_to_message:
            reply_to = str(update.message.reply_to_message.message_id)

        if self._handler_callback:
            await self._handler_callback(chat_id, text, reply_to)

    async def _on_voice(self, update: Update, _context) -> None:
        if not update.message or not update.message.voice:
            return
        user_id = update.effective_user.id if update.effective_user else None
        if user_id and self._allowed_users and user_id not in self._allowed_users:
            return

        chat = update.effective_chat
        if chat is None:
            return
        chat_id = str(chat.id)

        try:
            voice = update.message.voice
            file = await voice.get_file()
            audio_data = await file.download_as_bytearray()

            from config.settings import get_settings
            from voice.factory import create_transcription_backend

            transcriber = create_transcription_backend(get_settings())
            if transcriber is None:
                if self._bot:
                    await self._bot.send_message(
                        chat_id=int(chat_id),
                        text="❌ Voice transcription not configured.",
                    )
                return

            text = await transcriber.transcribe(bytes(audio_data))
            if not text:
                if self._bot:
                    await self._bot.send_message(
                        chat_id=int(chat_id), text="❌ Could not transcribe voice note."
                    )
                return

            reply_to: str | None = None
            if update.message.reply_to_message:
                reply_to = str(update.message.reply_to_message.message_id)

            if self._handler_callback:
                await self._handler_callback(chat_id, text, reply_to)
        except Exception as e:
            logger.error(f"Telegram voice transcription failed: {e}")

    async def _on_stop(self, update: Update, _context) -> None:
        if update.message is None:
            return
        if self._router is None:
            await update.message.reply_text("❌ Router not available.")
            return
        chat = update.effective_chat
        if chat is None:
            return
        chat_id = str(chat.id)
        response = await self._router.stop_task(chat_id)
        await update.message.reply_text(response)

    async def _on_clear(self, update: Update, _context) -> None:
        if update.message is None:
            return
        if self._router is None:
            await update.message.reply_text("❌ Router not available.")
            return
        chat = update.effective_chat
        chat_id = str(chat.id) if chat and update.message.reply_to_message else None
        response = await self._router.clear_sessions(chat_id)
        await update.message.reply_text(response)

    async def _on_stats(self, update: Update, _context) -> None:
        if update.message is None:
            return
        if self._router is None:
            await update.message.reply_text("❌ Router not available.")
            return
        response = await self._router.get_stats()
        await update.message.reply_text(response)
