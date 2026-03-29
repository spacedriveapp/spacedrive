#!/usr/bin/env python3
"""
OpenCode adapter for Spacedrive.

Indexes coding session transcripts from OpenCode.
Reads the opencode.db SQLite database and extracts sessions with their
conversation messages, aggregating token usage, cost, and tool call metadata.

The DB is copied to a temp file first since OpenCode holds an exclusive lock
via redb (though the SQLite portion may be readable, we copy for safety).

Supports incremental sync via time_updated cursor.
"""

import json
import sys
import os
import sqlite3
import shutil
import tempfile
from datetime import datetime, timezone


def log(level: str, message: str):
    print(json.dumps({"log": level, "message": message}), flush=True)


def emit(operation: dict):
    print(json.dumps(operation), flush=True)


def ms_to_iso(timestamp_ms: int) -> str:
    """Convert millisecond Unix timestamp to ISO 8601."""
    try:
        if not timestamp_ms or timestamp_ms == 0:
            return ""
        dt = datetime.fromtimestamp(timestamp_ms / 1000, tz=timezone.utc)
        return dt.isoformat()
    except (ValueError, OSError):
        return ""


def extract_text_parts(parts_data: list[dict]) -> str:
    """Extract and concatenate text content from message parts."""
    texts = []
    for part in parts_data:
        data = part.get("data", {})
        ptype = data.get("type", "")
        if ptype == "text":
            text = data.get("text", "").strip()
            if text:
                texts.append(text)
    return "\n\n".join(texts)


def extract_tool_summary(parts_data: list[dict]) -> str:
    """Extract a compact summary of tool calls from message parts."""
    tools = []
    for part in parts_data:
        data = part.get("data", {})
        if data.get("type") == "tool":
            tool_name = data.get("tool", "")
            # Extract a brief description of what the tool did
            tool_input = data.get("input", {})
            if tool_name == "read":
                path = tool_input.get("filePath", tool_input.get("path", ""))
                if path:
                    tools.append(f"read:{os.path.basename(path)}")
            elif tool_name == "edit":
                path = tool_input.get("filePath", tool_input.get("path", ""))
                if path:
                    tools.append(f"edit:{os.path.basename(path)}")
            elif tool_name == "write":
                path = tool_input.get("filePath", tool_input.get("path", ""))
                if path:
                    tools.append(f"write:{os.path.basename(path)}")
            elif tool_name == "bash":
                cmd = tool_input.get("command", "")[:80]
                if cmd:
                    tools.append(f"bash:{cmd}")
            elif tool_name == "glob":
                pattern = tool_input.get("pattern", "")
                if pattern:
                    tools.append(f"glob:{pattern}")
            elif tool_name == "grep":
                pattern = tool_input.get("pattern", "")
                if pattern:
                    tools.append(f"grep:{pattern}")
            elif tool_name:
                tools.append(tool_name)
    return "; ".join(tools[:50])  # Cap at 50 tool calls


def main():
    try:
        input_data = json.loads(sys.stdin.read())
    except json.JSONDecodeError as e:
        log("error", f"Invalid input JSON: {e}")
        sys.exit(2)

    config = input_data.get("config", {})
    cursor = input_data.get("cursor")

    db_path = config.get("db_path", "~/.local/share/opencode/opencode.db")
    db_path = os.path.expanduser(db_path)
    include_tool_calls = config.get("include_tool_calls", False)
    project_filter = config.get("project_filter", "")

    if not os.path.exists(db_path):
        log("error", f"OpenCode database not found: {db_path}")
        sys.exit(2)

    # Copy the database to avoid lock conflicts
    tmp_db = tempfile.mktemp(suffix=".db")
    try:
        shutil.copy2(db_path, tmp_db)
        for ext in ["-wal", "-shm"]:
            src = db_path + ext
            if os.path.exists(src):
                shutil.copy2(src, tmp_db + ext)
    except PermissionError:
        log("error", f"Permission denied reading: {db_path}")
        sys.exit(2)
    except Exception as e:
        log("error", f"Failed to copy database: {e}")
        sys.exit(2)

    try:
        conn = sqlite3.connect(tmp_db)
        conn.row_factory = sqlite3.Row

        # ── Fetch sessions ──────────────────────────────────────────────
        conditions = ["s.title != ''"]
        params = []

        if cursor:
            conditions.append("s.time_updated > ?")
            params.append(int(cursor))

        if project_filter:
            conditions.append("s.directory LIKE ?")
            params.append(f"%{project_filter}%")

        where = " AND ".join(conditions)
        sessions = conn.execute(f"""
            SELECT s.id, s.title, s.directory, s.parent_id,
                   s.summary_files, s.summary_additions, s.summary_deletions,
                   s.time_created, s.time_updated,
                   p.name as project_name, p.worktree as project_worktree
            FROM session s
            LEFT JOIN project p ON s.project_id = p.id
            WHERE {where}
            ORDER BY s.time_updated ASC
        """, params).fetchall()

        max_updated = int(cursor) if cursor else 0
        session_count = 0
        message_count = 0

        for session in sessions:
            sid = session["id"]
            time_updated = session["time_updated"] or 0

            # ── Fetch messages for this session ─────────────────────────
            messages = conn.execute("""
                SELECT m.id, m.data, m.time_created, m.time_updated
                FROM message m
                WHERE m.session_id = ?
                ORDER BY m.time_created ASC
            """, (sid,)).fetchall()

            if not messages:
                continue

            # ── Fetch parts for all messages in this session ────────────
            parts_by_message = {}
            parts = conn.execute("""
                SELECT p.id, p.message_id, p.data, p.time_created
                FROM part p
                WHERE p.session_id = ?
                ORDER BY p.time_created ASC
            """, (sid,)).fetchall()

            for part in parts:
                mid = part["message_id"]
                try:
                    part_data = json.loads(part["data"])
                except json.JSONDecodeError:
                    continue
                if mid not in parts_by_message:
                    parts_by_message[mid] = []
                parts_by_message[mid].append({"data": part_data})

            # ── Aggregate session stats ─────────────────────────────────
            total_input = 0
            total_output = 0
            total_cost = 0.0
            models_used = set()
            session_ended = session["time_created"]
            summary_parts = []
            summary_budget = 4000  # chars for FTS summary

            for msg in messages:
                try:
                    msg_data = json.loads(msg["data"])
                except json.JSONDecodeError:
                    continue

                tokens = msg_data.get("tokens", {})
                total_input += (tokens.get("input", 0) or 0)
                total_output += (tokens.get("output", 0) or 0)
                cache = tokens.get("cache", {})
                total_input += (cache.get("read", 0) or 0)
                total_input += (cache.get("write", 0) or 0)
                total_cost += (msg_data.get("cost", 0) or 0)

                model = msg_data.get("modelID", "")
                if model:
                    models_used.add(model)

                msg_time = msg["time_created"] or msg["time_updated"] or 0
                if msg_time > session_ended:
                    session_ended = msg_time

                # Build summary from conversation text
                if summary_budget > 0:
                    msg_parts = parts_by_message.get(msg["id"], [])
                    text = extract_text_parts(msg_parts)
                    if text:
                        role = msg_data.get("role", "")
                        # Take first N chars of each message
                        chunk = text[:min(800, summary_budget)]
                        summary_parts.append(chunk)
                        summary_budget -= len(chunk)

            session_summary = "\n".join(summary_parts)

            # Determine primary model (most common or first seen)
            primary_model = next(iter(models_used), "")
            # Clean model name — strip provider prefix
            if "/" in primary_model:
                primary_model = primary_model.split("/", 1)[1]

            project_name = session["project_name"] or ""
            if not project_name:
                # Derive from directory
                directory = session["directory"] or ""
                if directory:
                    project_name = os.path.basename(directory.rstrip("/"))

            # ── Emit session ────────────────────────────────────────────
            emit({
                "upsert": "session",
                "external_id": sid,
                "fields": {
                    "title": (session["title"] or "Untitled session")[:500],
                    "summary": session_summary,
                    "directory": session["directory"] or "",
                    "project": project_name,
                    "model": primary_model,
                    "message_count": len(messages),
                    "total_input_tokens": total_input,
                    "total_output_tokens": total_output,
                    "total_cost": round(total_cost, 6),
                    "files_changed": session["summary_files"] or 0,
                    "lines_added": session["summary_additions"] or 0,
                    "lines_deleted": session["summary_deletions"] or 0,
                    "started_at": ms_to_iso(session["time_created"]),
                    "ended_at": ms_to_iso(session_ended),
                }
            })
            session_count += 1

            # ── Emit messages ───────────────────────────────────────────
            for msg in messages:
                try:
                    msg_data = json.loads(msg["data"])
                except json.JSONDecodeError:
                    continue

                role = msg_data.get("role", "unknown")
                msg_parts = parts_by_message.get(msg["id"], [])

                # Build message body from text parts
                body = extract_text_parts(msg_parts)

                # Optionally include tool call summaries
                tool_summary = ""
                if include_tool_calls:
                    tool_summary = extract_tool_summary(msg_parts)
                    if tool_summary and body:
                        body = body + "\n\n[Tools: " + tool_summary + "]"
                    elif tool_summary:
                        body = "[Tools: " + tool_summary + "]"

                # Skip empty messages (e.g. step-start/step-finish only)
                if not body:
                    continue

                tokens = msg_data.get("tokens", {})
                input_tokens = (tokens.get("input", 0) or 0)
                cache = tokens.get("cache", {})
                input_tokens += (cache.get("read", 0) or 0) + (cache.get("write", 0) or 0)
                output_tokens = (tokens.get("output", 0) or 0)
                cost = msg_data.get("cost", 0) or 0

                model = msg_data.get("modelID", "")
                if "/" in model:
                    model = model.split("/", 1)[1]

                msg_time = msg["time_created"] or msg["time_updated"]

                emit({
                    "upsert": "message",
                    "external_id": msg["id"],
                    "fields": {
                        "role": role,
                        "body": body[:50000],  # Cap at 50KB per message
                        "model": model,
                        "input_tokens": input_tokens,
                        "output_tokens": output_tokens,
                        "cost": round(cost, 6),
                        "tool_calls": tool_summary[:5000] if tool_summary else "",
                        "timestamp": ms_to_iso(msg_time),
                    },
                    "relations": {
                        "session": sid
                    }
                })
                message_count += 1

            if time_updated > max_updated:
                max_updated = time_updated

        # Emit cursor
        if max_updated > 0:
            emit({"cursor": str(max_updated)})

        log("info", f"Synced {session_count} sessions, {message_count} messages")

        conn.close()

    except sqlite3.Error as e:
        log("error", f"SQLite error: {e}")
        sys.exit(1)
    finally:
        for f in [tmp_db, tmp_db + "-wal", tmp_db + "-shm"]:
            if os.path.exists(f):
                os.unlink(f)


if __name__ == "__main__":
    main()
