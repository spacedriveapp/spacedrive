#!/usr/bin/env python3
"""
macOS Contacts adapter for Spacedrive.

Reads the macOS AddressBook SQLite database directly.
The database is at ~/Library/Application Support/AddressBook/AddressBook-v22.abcddb

Requires Contacts permission for the running process.

Full-scan adapter — re-reads all contacts each sync and upserts.
Incremental via modification date cursor.
"""

import json
import sys
import os
import sqlite3
import shutil
import tempfile
from datetime import datetime, timezone

# Core Data epoch: 2001-01-01 00:00:00 UTC
CORE_DATA_EPOCH = 978307200


def log(level: str, message: str):
    print(json.dumps({"log": level, "message": message}), flush=True)


def emit(operation: dict):
    print(json.dumps(operation), flush=True)


def cd_time_to_iso(timestamp) -> str:
    """Convert Core Data timestamp (seconds since 2001-01-01) to ISO 8601 UTC."""
    try:
        if timestamp is None or timestamp == 0:
            return ""
        unix_seconds = float(timestamp) + CORE_DATA_EPOCH
        dt = datetime.fromtimestamp(unix_seconds, tz=timezone.utc)
        return dt.isoformat()
    except (ValueError, OSError, TypeError):
        return ""


def find_addressbook_db() -> str:
    """Find the AddressBook database path."""
    base = os.path.expanduser("~/Library/Application Support/AddressBook")

    # Modern macOS: Sources/<UUID>/AddressBook-v22.abcddb
    sources_dir = os.path.join(base, "Sources")
    if os.path.isdir(sources_dir):
        for entry in os.listdir(sources_dir):
            candidate = os.path.join(sources_dir, entry, "AddressBook-v22.abcddb")
            if os.path.exists(candidate):
                return candidate

    # Legacy location
    legacy = os.path.join(base, "AddressBook-v22.abcddb")
    if os.path.exists(legacy):
        return legacy

    return ""


def main():
    try:
        input_data = json.loads(sys.stdin.read())
    except json.JSONDecodeError as e:
        log("error", f"Invalid input JSON: {e}")
        sys.exit(2)

    config = input_data.get("config", {})
    cursor = input_data.get("cursor")

    db_path = find_addressbook_db()
    if not db_path:
        log("error", "macOS AddressBook database not found. Ensure Contacts permission is granted.")
        sys.exit(2)

    # Copy the database since Contacts may hold a lock
    tmp_db = tempfile.mktemp(suffix=".abcddb")
    try:
        shutil.copy2(db_path, tmp_db)
        for ext in ["-wal", "-shm"]:
            src = db_path + ext
            if os.path.exists(src):
                shutil.copy2(src, tmp_db + ext)
    except PermissionError:
        log("error", f"Permission denied reading: {db_path}. Grant Contacts access.")
        sys.exit(2)
    except Exception as e:
        log("error", f"Failed to copy AddressBook database: {e}")
        sys.exit(2)

    try:
        conn = sqlite3.connect(tmp_db)
        conn.row_factory = sqlite3.Row

        # ── Load groups ──────────────────────────────────────────────────
        try:
            groups = conn.execute(
                "SELECT ROWID, ZNAME FROM ZABCDGROUP WHERE ZNAME IS NOT NULL"
            ).fetchall()
            for g in groups:
                emit({
                    "upsert": "group",
                    "external_id": str(g["ROWID"]),
                    "fields": {"name": g["ZNAME"] or ""},
                })
        except sqlite3.Error:
            log("warn", "Could not read groups table")

        # ── Load group memberships ───────────────────────────────────────
        group_members = {}  # contact_rowid -> [group_rowid, ...]
        try:
            memberships = conn.execute("""
                SELECT ZGROUP, ZMEMBER FROM ZABCDGROUPMEMBERS
            """).fetchall()
            for m in memberships:
                group_id = m["ZGROUP"]
                member_id = m["ZMEMBER"]
                if member_id not in group_members:
                    group_members[member_id] = []
                group_members[member_id].append(group_id)
        except sqlite3.Error:
            pass

        # ── Load contacts ────────────────────────────────────────────────
        conditions = ["1=1"]
        params = []

        if cursor:
            conditions.append("ZMODIFICATIONDATE > ?")
            params.append(float(cursor))

        where = " AND ".join(conditions)

        contacts = conn.execute(f"""
            SELECT
                ROWID,
                ZFIRSTNAME,
                ZLASTNAME,
                ZORGANIZATION,
                ZJOBTITLE,
                ZNOTE,
                ZBIRTHDAY,
                ZCREATIONDATE,
                ZMODIFICATIONDATE
            FROM ZABCDRECORD
            WHERE ZENTITYNAME = 'ABPerson' AND {where}
            ORDER BY ZMODIFICATIONDATE DESC
        """, params).fetchall()

        max_mod_date = float(cursor) if cursor else 0
        count = 0

        for contact in contacts:
            rowid = contact["ROWID"]
            first = contact["ZFIRSTNAME"] or ""
            last = contact["ZLASTNAME"] or ""
            name = f"{first} {last}".strip() or "(No Name)"
            org = contact["ZORGANIZATION"] or ""
            job_title = contact["ZJOBTITLE"] or ""
            notes = contact["ZNOTE"] or ""
            birthday_raw = contact["ZBIRTHDAY"]
            created = cd_time_to_iso(contact["ZCREATIONDATE"])
            modified = cd_time_to_iso(contact["ZMODIFICATIONDATE"])
            mod_date = contact["ZMODIFICATIONDATE"] or 0

            birthday = ""
            if birthday_raw:
                birthday = cd_time_to_iso(birthday_raw)

            # Fetch emails
            emails = []
            try:
                email_rows = conn.execute(
                    "SELECT ZADDRESS FROM ZABCDEMAILADDRESS WHERE ZOWNER = ?",
                    [rowid]
                ).fetchall()
                emails = [r["ZADDRESS"] for r in email_rows if r["ZADDRESS"]]
            except sqlite3.Error:
                pass

            # Fetch phone numbers
            phones = []
            try:
                phone_rows = conn.execute(
                    "SELECT ZFULLNUMBER FROM ZABCDPHONENUMBER WHERE ZOWNER = ?",
                    [rowid]
                ).fetchall()
                phones = [r["ZFULLNUMBER"] for r in phone_rows if r["ZFULLNUMBER"]]
            except sqlite3.Error:
                pass

            # Fetch addresses
            addresses = []
            try:
                addr_rows = conn.execute("""
                    SELECT ZSTREET, ZCITY, ZSTATE, ZZIPCODE, ZCOUNTRYNAME
                    FROM ZABCDPOSTALADDRESS WHERE ZOWNER = ?
                """, [rowid]).fetchall()
                for a in addr_rows:
                    parts = [
                        a["ZSTREET"] or "",
                        a["ZCITY"] or "",
                        a["ZSTATE"] or "",
                        a["ZZIPCODE"] or "",
                        a["ZCOUNTRYNAME"] or "",
                    ]
                    addr_str = ", ".join(p for p in parts if p)
                    if addr_str:
                        addresses.append(addr_str)
            except sqlite3.Error:
                pass

            emit({
                "upsert": "contact",
                "external_id": str(rowid),
                "fields": {
                    "name": name,
                    "organization": org,
                    "job_title": job_title,
                    "emails": ", ".join(emails),
                    "phones": ", ".join(phones),
                    "addresses": "\n".join(addresses),
                    "notes": notes[:10000],
                    "birthday": birthday,
                    "created": created,
                    "modified": modified,
                }
            })
            count += 1

            # Link to groups
            if rowid in group_members:
                for gid in group_members[rowid]:
                    emit({
                        "link": "contact",
                        "id": str(rowid),
                        "to": "group",
                        "to_id": str(gid),
                    })

            if mod_date > max_mod_date:
                max_mod_date = mod_date

        # Emit cursor
        if max_mod_date > 0:
            emit({"cursor": str(max_mod_date)})

        log("info", f"Synced {count} contacts")
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
