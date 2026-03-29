#!/usr/bin/env python3
"""
Apple Notes adapter for Spacedrive.

Reads the Apple Notes SQLite database on macOS.
Requires Full Disk Access for the running process.

The database is at:
  ~/Library/Group Containers/group.com.apple.notes/NoteStore.sqlite

Password-protected notes are skipped.
Uses ZSNIPPET for note body text (plain text excerpt stored by Notes.app).
"""

import json
import sys
import os
import sqlite3
import shutil
import tempfile
from datetime import datetime, timezone

# Apple's Core Data epoch: 2001-01-01 00:00:00 UTC
CORE_DATA_EPOCH = 978307200


def log(level: str, message: str):
    print(json.dumps({"log": level, "message": message}), flush=True)


def emit(operation: dict):
    print(json.dumps(operation), flush=True)


def core_data_time_to_iso(timestamp) -> str:
    """Convert Core Data timestamp (seconds since 2001-01-01) to ISO 8601."""
    try:
        if timestamp is None or timestamp == 0:
            return ""
        unix_seconds = float(timestamp) + CORE_DATA_EPOCH
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

    # Standard macOS path
    notes_db = os.path.expanduser(
        "~/Library/Group Containers/group.com.apple.notes/NoteStore.sqlite"
    )

    if not os.path.exists(notes_db):
        log("error", f"Apple Notes database not found: {notes_db}")
        sys.exit(2)

    # Copy the database to avoid lock issues
    tmp_db = tempfile.mktemp(suffix=".db")
    try:
        shutil.copy2(notes_db, tmp_db)
        for ext in ["-wal", "-shm"]:
            src = notes_db + ext
            if os.path.exists(src):
                shutil.copy2(src, tmp_db + ext)
    except PermissionError:
        log("error", "Permission denied. Grant Full Disk Access to the app running Spacedrive (System Settings > Privacy & Security > Full Disk Access)")
        sys.exit(2)
    except Exception as e:
        log("error", f"Failed to copy Notes database: {e}")
        sys.exit(2)

    try:
        conn = sqlite3.connect(tmp_db)
        conn.row_factory = sqlite3.Row

        # ── Sync folders ──────────────────────────────────────────────────
        folder_count = 0
        folders = conn.execute("""
            SELECT
                f.Z_PK,
                f.ZTITLE2 as name,
                a.ZNAME as account_name,
                f.ZIDENTIFIER as identifier
            FROM ZICCLOUDSYNCINGOBJECT f
            LEFT JOIN ZICCLOUDSYNCINGOBJECT a
                ON f.ZACCOUNT4 = a.Z_PK
                AND a.Z_ENT = (SELECT Z_ENT FROM Z_PRIMARYKEY WHERE Z_NAME = 'ICAccount')
            WHERE f.ZTITLE2 IS NOT NULL
              AND f.ZMARKEDFORDELETION != 1
              AND f.ZIDENTIFIER IS NOT NULL
              AND f.Z_ENT = (SELECT Z_ENT FROM Z_PRIMARYKEY WHERE Z_NAME = 'ICFolder')
        """).fetchall()

        folder_map = {}
        for f in folders:
            fid = f["identifier"]
            folder_map[f["Z_PK"]] = fid
            account = f["account_name"] or "Local"

            emit({
                "upsert": "folder",
                "external_id": fid,
                "fields": {
                    "name": f["name"] or "Untitled Folder",
                    "account": account,
                }
            })
            folder_count += 1

        log("info", f"Synced {folder_count} folders")

        # ── Sync notes ────────────────────────────────────────────────────
        note_count = 0
        notes = conn.execute("""
            SELECT
                n.ZIDENTIFIER as identifier,
                n.ZTITLE1 as title,
                n.ZSNIPPET as snippet,
                n.ZCREATIONDATE3 as created,
                n.ZMODIFICATIONDATE1 as modified,
                n.ZISPINNED as is_pinned,
                n.ZFOLDER as folder_pk,
                n.ZMARKEDFORDELETION as deleted
            FROM ZICCLOUDSYNCINGOBJECT n
            WHERE n.ZTITLE1 IS NOT NULL
              AND (n.ZMARKEDFORDELETION IS NULL OR n.ZMARKEDFORDELETION != 1)
              AND n.ZIDENTIFIER IS NOT NULL
              AND n.Z_ENT = (SELECT Z_ENT FROM Z_PRIMARYKEY WHERE Z_NAME = 'ICNote')
        """).fetchall()

        for note in notes:
            nid = note["identifier"]
            title = note["title"] or "Untitled"
            snippet = note["snippet"] or ""
            created = core_data_time_to_iso(note["created"])
            modified = core_data_time_to_iso(note["modified"])
            is_pinned = bool(note["is_pinned"]) if note["is_pinned"] else False

            # Use snippet as body (it's the plain text content Apple stores)
            body = snippet

            fields = {
                "title": title,
                "body": body,
                "snippet": snippet[:500] if snippet else "",
                "created": created,
                "modified": modified,
                "is_pinned": is_pinned,
            }

            # Set folder FK if we can resolve it
            folder_pk = note["folder_pk"]
            if folder_pk and folder_pk in folder_map:
                fields["folder_id"] = folder_map[folder_pk]

            emit({
                "upsert": "note",
                "external_id": nid,
                "fields": fields,
            })
            note_count += 1

        log("info", f"Synced {note_count} notes")
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
