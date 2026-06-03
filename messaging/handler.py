"""PrxyClaude · Message handler — bridges messaging platforms to CLI sessions."""

import asyncio

from loguru import logger

from cli.session_manager import CLISessionManager
from messaging.base import MessagingPlatform


class SessionRouter:
    """Routes incoming messages to CLI sessions and streams responses."""

    def __init__(
        self,
        session_manager: CLISessionManager,
        api_url: str,
    ):
        self._session_manager = session_manager
        self._api_url = api_url
        self._running_tasks: dict[str, asyncio.Task] = {}

    async def handle_message(
        self,
        platform: MessagingPlatform,
        chat_id: str,
        text: str,
        reply_to: str | None = None,
    ) -> None:
        """Process an incoming message through a CLI session."""
        session_id: str | None = None

        if reply_to:
            reply_to = reply_to.replace("pending_", "")

        session, sid, is_new = await self._session_manager.get_or_create_session(
            reply_to
        )

        if is_new and reply_to:
            session_id = reply_to
        elif is_new:
            session_id = sid
        else:
            session_id = sid

        loading_msg = await platform.send_message(
            chat_id, "🤔 Thinking...", reply_to=session_id
        )

        accumulated_text = ""
        task = asyncio.current_task()
        if task is not None:
            self._running_tasks[chat_id] = task
        try:
            async for event in session.start_task(text, session_id=session_id):
                event_type = event.get("type", "")

                if event_type == "session_info":
                    real_sid = event.get("session_id")
                    if real_sid and real_sid != sid:
                        await self._session_manager.register_real_session_id(
                            sid, real_sid
                        )
                        session_id = real_sid

                elif event_type == "text":
                    chunk = event.get("content", "")
                    if chunk:
                        accumulated_text += chunk
                        if len(accumulated_text) > 50 and loading_msg:
                            await platform.edit_message(
                                chat_id, loading_msg, accumulated_text
                            )

                elif event_type == "tool_use":
                    tool_name = event.get("name", "unknown")
                    logger.info(f"Tool call: {tool_name}")

                elif event_type == "error":
                    err_msg = event.get("error", {}).get("message", "Unknown error")
                    await platform.send_message(
                        chat_id, f"❌ Error: {err_msg}", reply_to=session_id
                    )
                    accumulated_text = ""

                elif event_type == "exit":
                    if accumulated_text:
                        if loading_msg:
                            await platform.edit_message(
                                chat_id, loading_msg, accumulated_text
                            )
                        else:
                            await platform.send_message(
                                chat_id, accumulated_text, reply_to=session_id
                            )

        except asyncio.CancelledError:
            await platform.send_message(
                chat_id, "⏹️ Task cancelled.", reply_to=session_id
            )
            raise
        except Exception as e:
            logger.error(f"Message handler error: {e}")
            await platform.send_message(chat_id, f"❌ Error: {e}", reply_to=session_id)
        finally:
            self._running_tasks.pop(chat_id, None)

    async def stop_task(self, chat_id: str) -> str:
        """Cancel the running task for a chat_id."""
        task = self._running_tasks.pop(chat_id, None)
        if task is not None and not task.done():
            task.cancel()
            return "⏹️ Task cancelled."
        return "No running task to stop."

    async def clear_sessions(self, chat_id: str | None) -> str:
        """Clear sessions, optionally scoped to a branch."""
        if chat_id:
            task = self._running_tasks.pop(chat_id, None)
            if task is not None and not task.done():
                task.cancel()
        await self._session_manager.stop_all()
        return "🗑️ All sessions cleared."

    async def get_stats(self) -> str:
        """Get session statistics."""
        stats = self._session_manager.get_stats()
        lines = [
            f"**Sessions:** {stats['active_sessions']} active, {stats['pending_sessions']} pending",
            f"**Busy:** {stats['busy_count']}",
        ]
        return "\n".join(lines)
