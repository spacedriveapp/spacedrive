#!/usr/bin/env python3
"""
Safari History adapter for Spacedrive.

Reads the Safari History.db SQLite database on macOS.
Copies the DB to a temp file first since Safari holds a lock on it.

Requires Full Disk Access to read ~/Library/Safari/History.db.

Supports incremental sync via visit_time cursor.
"""

import json
import sys
import os
import sqlite3
import shutil
import tempfile
from datetime import datetime, timezone

# Safari stores timestamps as seconds since 2001-01-01 (Core Data / Cocoa epoch)
CORE_DATA_EPOCH = 978307200


def log(level: str, message: str):
    print(json.dumps({"log": level, "message": message}), flush=True)


def emit(operation: dict):
    print(json.dumps(operation), flush=True)


def safari_time_to_iso(timestamp: float) -> str:
    """Convert Safari/Core Data timestamp (seconds since 2001-01-01) to ISO 8601 UTC."""
    try:
        if not timestamp or timestamp == 0:
            return ""
        unix_seconds = float(timestamp) + CORE_DATA_EPOCH
        if unix_seconds < 0:
            return ""
        dt = datetime.fromtimestamp(unix_seconds, tz=timezone.utc)
        return dt.isoformat()
    except (ValueError, OSError, TypeError):
        return ""


def main():
    try:
        input_data = json.loads(sys.stdin.read())
    except json.JSONDecodeError as e:
        log("error", f"Invalid input JSON: {e}")
        sys.exit(2)

    config = input_data.get("config", {})
    cursor = input_data.get("cursor")

    # Safari History.db location
    history_path = os.path.expanduser("~/Library/Safari/History.db")
    if not os.path.exists(history_path):
        log("error", f"Safari history database not found: {history_path}")
        sys.exit(2)

    min_visit_count = int(config.get("min_visit_count", 1))
    max_results = int(config.get("max_results", 10000))

    # Copy the database since Safari holds a lock
    tmp_db = tempfile.mktemp(suffix=".db")
    try:
        shutil.copy2(history_path, tmp_db)
        for ext in ["-wal", "-shm"]:
            src = history_path + ext
            if os.path.exists(src):
                shutil.copy2(src, tmp_db + ext)
    except PermissionError:
        log("error", f"Permission denied reading: {history_path}. Grant Full Disk Access to the running process.")
        sys.exit(2)
    except Exception as e:
        log("error", f"Failed to copy Safari history database: {e}")
        sys.exit(2)

    try:
        conn = sqlite3.connect(tmp_db)
        conn.row_factory = sqlite3.Row

        # Safari schema:
        #   history_items: id, url, domain_expansion, visit_count, daily_visit_counts, ...
        #   history_visits: id, history_item, visit_time, title, ...
        #
        # We join to get the most recent visit per URL with its title.

        conditions = ["hi.visit_count >= ?"]
        params = [min_visit_count]

        if cursor:
            conditions.append("hv.visit_time > ?")
            params.append(float(cursor))

        where = " AND ".join(conditions)

        query = f"""
            SELECT
                hi.id AS item_id,
                hi.url,
                hi.visit_count,
                hv.visit_time,
                hv.title
            FROM history_items hi
            JOIN history_visits hv ON hv.history_item = hi.id
            WHERE hv.id = (
                SELECT hv2.id FROM history_visits hv2
                WHERE hv2.history_item = hi.id
                ORDER BY hv2.visit_time DESC LIMIT 1
            )
            AND {where}
            ORDER BY hv.visit_time DESC
            LIMIT ?
        """
        params.append(max_results)

        rows = conn.execute(query, params).fetchall()

        max_visit_time = float(cursor) if cursor else 0
        count = 0

        for row in rows:
            item_id = str(row["item_id"])
            url = row["url"] or ""
            title = row["title"] or url
            visit_count = row["visit_count"] or 0
            visit_time = row["visit_time"] or 0

            # Skip internal pages
            if url.startswith(("about:", "blob:", "data:")):
                continue

            last_visit_iso = safari_time_to_iso(visit_time)

            emit({
                "upsert": "page",
                "external_id": item_id,
                "fields": {
                    "title": title[:500],
                    "url": url[:2000],
                    "visit_count": visit_count,
                    "last_visit": last_visit_iso,
                }
            })
            count += 1

            if visit_time > max_visit_time:
                max_visit_time = visit_time

        # Emit cursor for incremental sync
        if max_visit_time > 0:
            emit({"cursor": str(max_visit_time)})

        log("info", f"Synced {count} pages (min visits: {min_visit_count})")

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
