#!/usr/bin/env python3
"""
Chrome History adapter for Spacedrive.

Reads the Chromium History SQLite database (works with Chrome, Arc, Brave, Edge).
Copies the DB to a temp file first since the browser holds a lock on it.

Supports incremental sync via last_visit_time cursor.
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


def chromium_time_to_iso(timestamp: int) -> str:
    """Convert Chromium timestamp (microseconds since 1601-01-01) to ISO 8601."""
    try:
        if timestamp == 0:
            return ""
        unix_seconds = (timestamp / 1_000_000) - 11644473600
        if unix_seconds < 0:
            return ""
        dt = datetime.fromtimestamp(unix_seconds, tz=timezone.utc)
        return dt.isoformat()
    except (ValueError, OSError):
        return ""


def main():
    try:
        input_data = json.loads(sys.stdin.read())
    except json.JSONDecodeError as e:
        log("error", f"Invalid input JSON: {e}")
        sys.exit(2)

    config = input_data.get("config", {})
    cursor = input_data.get("cursor")

    history_path = config.get("history_path", "")
    if not history_path:
        log("error", "Missing required config: history_path")
        sys.exit(2)

    history_path = os.path.expanduser(history_path)
    if not os.path.exists(history_path):
        log("error", f"History database not found: {history_path}")
        sys.exit(2)

    min_visit_count = int(config.get("min_visit_count", 1))
    max_results = int(config.get("max_results", 10000))

    # Copy the database since the browser holds a lock
    tmp_db = tempfile.mktemp(suffix=".db")
    try:
        shutil.copy2(history_path, tmp_db)
        # Also copy WAL if it exists
        for ext in ["-wal", "-shm"]:
            src = history_path + ext
            if os.path.exists(src):
                shutil.copy2(src, tmp_db + ext)
    except PermissionError:
        log("error", f"Permission denied reading: {history_path}")
        sys.exit(2)
    except Exception as e:
        log("error", f"Failed to copy history database: {e}")
        sys.exit(2)

    try:
        conn = sqlite3.connect(tmp_db)
        conn.row_factory = sqlite3.Row

        # Build query
        conditions = ["visit_count >= ?"]
        params = [min_visit_count]

        if cursor:
            conditions.append("last_visit_time > ?")
            params.append(int(cursor))

        where = " AND ".join(conditions)
        query = f"""
            SELECT id, url, title, visit_count, last_visit_time
            FROM urls
            WHERE {where}
            ORDER BY last_visit_time DESC
            LIMIT ?
        """
        params.append(max_results)

        rows = conn.execute(query, params).fetchall()

        max_visit_time = int(cursor) if cursor else 0
        count = 0

        for row in rows:
            url_id = str(row["id"])
            url = row["url"] or ""
            title = row["title"] or url
            visit_count = row["visit_count"] or 0
            last_visit = row["last_visit_time"] or 0

            # Skip internal browser pages
            if url.startswith(("chrome://", "chrome-extension://", "about:", "arc://", "brave://")):
                continue

            last_visit_iso = chromium_time_to_iso(last_visit)

            emit({
                "upsert": "page",
                "external_id": url_id,
                "fields": {
                    "title": title[:500],
                    "url": url[:2000],
                    "visit_count": visit_count,
                    "last_visit": last_visit_iso,
                }
            })
            count += 1

            if last_visit > max_visit_time:
                max_visit_time = last_visit

        # Emit cursor for incremental sync
        if max_visit_time > 0:
            emit({"cursor": str(max_visit_time)})

        log("info", f"Synced {count} pages (min visits: {min_visit_count})")

        conn.close()

    except sqlite3.Error as e:
        log("error", f"SQLite error: {e}")
        sys.exit(1)
    finally:
        # Cleanup temp files
        for f in [tmp_db, tmp_db + "-wal", tmp_db + "-shm"]:
            if os.path.exists(f):
                os.unlink(f)


if __name__ == "__main__":
    main()
