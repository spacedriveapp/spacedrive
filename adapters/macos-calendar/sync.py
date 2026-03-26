#!/usr/bin/env python3
"""
macOS Calendar adapter for Spacedrive.

Reads the macOS Calendar SQLite database (CalendarAgent).
Located at ~/Library/Calendars/Calendar.sqlitedb

Requires Calendar access (or Full Disk Access) for the running process.

Incremental sync via modification date cursor.
"""

import json
import sys
import os
import sqlite3
import shutil
import tempfile
from datetime import datetime, timezone, timedelta

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


def find_calendar_db() -> str:
    """Find the Calendar database path."""
    # Primary location
    primary = os.path.expanduser("~/Library/Calendars/Calendar.sqlitedb")
    if os.path.exists(primary):
        return primary

    # Some macOS versions use a different path
    alt = os.path.expanduser("~/Library/Group Containers/group.com.apple.CalendarAgent/Calendar.sqlitedb")
    if os.path.exists(alt):
        return alt

    return ""


def main():
    try:
        input_data = json.loads(sys.stdin.read())
    except json.JSONDecodeError as e:
        log("error", f"Invalid input JSON: {e}")
        sys.exit(2)

    config = input_data.get("config", {})
    cursor = input_data.get("cursor")

    months_back = int(config.get("months_back", 12))

    db_path = find_calendar_db()
    if not db_path:
        log("error", "macOS Calendar database not found. Ensure Calendar access is granted.")
        sys.exit(2)

    # Copy the database since CalendarAgent holds a lock
    tmp_db = tempfile.mktemp(suffix=".sqlitedb")
    try:
        shutil.copy2(db_path, tmp_db)
        for ext in ["-wal", "-shm"]:
            src = db_path + ext
            if os.path.exists(src):
                shutil.copy2(src, tmp_db + ext)
    except PermissionError:
        log("error", f"Permission denied reading: {db_path}. Grant Calendar or Full Disk Access.")
        sys.exit(2)
    except Exception as e:
        log("error", f"Failed to copy Calendar database: {e}")
        sys.exit(2)

    try:
        conn = sqlite3.connect(tmp_db)
        conn.row_factory = sqlite3.Row

        # ── Load calendars ───────────────────────────────────────────────
        calendars = {}
        try:
            cal_rows = conn.execute("""
                SELECT ROWID, ZTITLE, ZCOLOR, ZSOURCEACCOUNT
                FROM ZCALENDAR
                WHERE ZTITLE IS NOT NULL
            """).fetchall()

            for cal in cal_rows:
                cal_id = str(cal["ROWID"])
                cal_name = cal["ZTITLE"] or ""
                # Color is stored as an integer in some schemas
                color_raw = cal["ZCOLOR"]
                color = str(color_raw) if color_raw else ""

                # Try to get account name
                account = ""
                try:
                    if cal["ZSOURCEACCOUNT"]:
                        acct_row = conn.execute(
                            "SELECT ZACCOUNTNAME FROM ZSOURCE WHERE ROWID = ?",
                            [cal["ZSOURCEACCOUNT"]]
                        ).fetchone()
                        if acct_row:
                            account = acct_row["ZACCOUNTNAME"] or ""
                except sqlite3.Error:
                    pass

                calendars[cal["ROWID"]] = cal_name

                emit({
                    "upsert": "calendar",
                    "external_id": cal_id,
                    "fields": {
                        "name": cal_name,
                        "color": color,
                        "account": account,
                    }
                })
        except sqlite3.Error as e:
            log("warn", f"Could not read calendars: {e}")

        # ── Load events ──────────────────────────────────────────────────
        conditions = []
        params = []

        # Date range filter
        if months_back > 0:
            cutoff = datetime.now(timezone.utc) - timedelta(days=months_back * 30)
            cutoff_cd = cutoff.timestamp() - CORE_DATA_EPOCH
            conditions.append("ZSTARTDATE >= ?")
            params.append(cutoff_cd)

        if cursor:
            conditions.append("ZLASTMODIFIEDDATE > ?")
            params.append(float(cursor))

        where = " AND ".join(conditions) if conditions else "1=1"

        events = conn.execute(f"""
            SELECT
                ROWID,
                ZSUMMARY,
                ZLOCATION,
                ZNOTES,
                ZSTARTDATE,
                ZENDDATE,
                ZISALLDAY,
                ZRECURRENCERULE,
                ZURL,
                ZSTATUS,
                ZCALENDAR,
                ZLASTMODIFIEDDATE
            FROM ZCALENDARITEM
            WHERE ZENTITYTYPE = 1 AND {where}
            ORDER BY ZSTARTDATE DESC
        """, params).fetchall()

        max_mod_date = float(cursor) if cursor else 0
        count = 0

        for event in events:
            rowid = event["ROWID"]
            title = event["ZSUMMARY"] or "(No Title)"
            location = event["ZLOCATION"] or ""
            notes = event["ZNOTES"] or ""
            start_date = cd_time_to_iso(event["ZSTARTDATE"])
            end_date = cd_time_to_iso(event["ZENDDATE"])
            is_all_day = bool(event["ZISALLDAY"])
            recurrence = str(event["ZRECURRENCERULE"] or "")
            url = event["ZURL"] or ""
            status_code = event["ZSTATUS"]
            cal_rowid = event["ZCALENDAR"]
            mod_date = event["ZLASTMODIFIEDDATE"] or 0

            # Map status codes
            status_map = {0: "none", 1: "confirmed", 2: "tentative", 3: "cancelled"}
            status = status_map.get(status_code, "unknown") if status_code is not None else ""

            # Fetch attendees
            attendees = []
            try:
                att_rows = conn.execute("""
                    SELECT ZCOMMONNAME, ZADDRESS
                    FROM ZATTENDEE WHERE ZEVENT = ?
                """, [rowid]).fetchall()
                for att in att_rows:
                    name = att["ZCOMMONNAME"] or ""
                    addr = att["ZADDRESS"] or ""
                    if addr.startswith("mailto:"):
                        addr = addr[7:]
                    if name and addr:
                        attendees.append(f"{name} <{addr}>")
                    elif name:
                        attendees.append(name)
                    elif addr:
                        attendees.append(addr)
            except sqlite3.Error:
                pass

            event_fields = {
                "title": title[:500],
                "location": location[:500],
                "notes": notes[:10000],
                "start_date": start_date,
                "end_date": end_date,
                "is_all_day": is_all_day,
                "recurrence": recurrence[:200],
                "attendees": "\n".join(attendees)[:5000],
                "url": url[:2000],
                "status": status,
            }

            # Add calendar FK if we know the calendar
            if cal_rowid and cal_rowid in calendars:
                event_fields["calendar_id"] = str(cal_rowid)

            emit({
                "upsert": "event",
                "external_id": str(rowid),
                "fields": event_fields,
            })
            count += 1

            if mod_date > max_mod_date:
                max_mod_date = mod_date

        # Emit cursor
        if max_mod_date > 0:
            emit({"cursor": str(max_mod_date)})

        log("info", f"Synced {count} events from {len(calendars)} calendars")
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
