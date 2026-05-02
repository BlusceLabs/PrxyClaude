"""Lightweight token usage tracking (SQLite-backed)."""

from __future__ import annotations

import sqlite3
import time
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path

_DB_PATH = Path("usage.db")


def _ensure_table(conn: sqlite3.Connection) -> None:
    conn.execute(
        """
        CREATE TABLE IF NOT EXISTS usage (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp REAL NOT NULL,
            provider TEXT NOT NULL,
            model TEXT NOT NULL,
            input_tokens INTEGER,
            output_tokens INTEGER,
            request_id TEXT
        )
        """
    )
    conn.commit()


@contextmanager
def _get_conn() -> Iterator[sqlite3.Connection]:
    conn = sqlite3.connect(str(_DB_PATH), timeout=5)
    try:
        _ensure_table(conn)
        yield conn
    finally:
        conn.close()


def log_usage(
    *,
    provider: str,
    model: str,
    input_tokens: int | None = None,
    output_tokens: int | None = None,
    request_id: str | None = None,
) -> None:
    """Record a token usage event."""
    with _get_conn() as conn:
        conn.execute(
            "INSERT INTO usage (timestamp, provider, model, input_tokens, output_tokens, request_id) "
            "VALUES (?, ?, ?, ?, ?, ?)",
            (time.time(), provider, model, input_tokens, output_tokens, request_id),
        )
        conn.commit()


def get_usage_summary(since_hours: int = 24) -> dict:
    """Return usage summary for the last N hours."""
    cutoff = time.time() - since_hours * 3600
    with _get_conn() as conn:
        rows = conn.execute(
            "SELECT provider, model, "
            "SUM(input_tokens), SUM(output_tokens), COUNT(*) "
            "FROM usage WHERE timestamp >= ? "
            "GROUP BY provider, model",
            (cutoff,),
        ).fetchall()
    return [
        {
            "provider": r[0],
            "model": r[1],
            "input_tokens": r[2] or 0,
            "output_tokens": r[3] or 0,
            "requests": r[4],
        }
        for r in rows
    ]


def get_total_usage() -> dict:
    """Return grand total usage."""
    with _get_conn() as conn:
        row = conn.execute(
            "SELECT SUM(input_tokens), SUM(output_tokens), COUNT(*) FROM usage"
        ).fetchone()
    return {
        "input_tokens": row[0] or 0,
        "output_tokens": row[1] or 0,
        "requests": row[2] or 0,
    }
