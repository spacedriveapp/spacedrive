#!/usr/bin/env python3
"""
Slack Export adapter for Spacedrive.

Reads a Slack workspace export (the unzipped JSON directory structure).
Export format: channels.json, users.json, and one folder per channel
containing daily JSON files (YYYY-MM-DD.json).

This is a full-scan adapter — no incremental cursor since exports are
point-in-time snapshots. Re-exporting and re-syncing will upsert all data.
"""

import json
import sys
import os
import glob
from datetime import datetime, timezone


def log(level: str, message: str):
    print(json.dumps({"log": level, "message": message}), flush=True)


def emit(operation: dict):
    print(json.dumps(operation), flush=True)


def ts_to_iso(ts: str) -> str:
    """Convert Slack timestamp (Unix float as string, e.g. '1706123456.789012') to ISO 8601 UTC."""
    try:
        if not ts:
            return ""
        dt = datetime.fromtimestamp(float(ts), tz=timezone.utc)
        return dt.isoformat()
    except (ValueError, OSError, TypeError):
        return ""


def load_json(path: str):
    """Load a JSON file, returning None on failure."""
    try:
        with open(path, "r", encoding="utf-8") as f:
            return json.load(f)
    except (json.JSONDecodeError, OSError) as e:
        log("warn", f"Failed to read {path}: {e}")
        return None


def main():
    try:
        input_data = json.loads(sys.stdin.read())
    except json.JSONDecodeError as e:
        log("error", f"Invalid input JSON: {e}")
        sys.exit(2)

    config = input_data.get("config", {})

    export_path = config.get("export_path", "")
    if not export_path:
        log("error", "Missing required config: export_path")
        sys.exit(2)

    export_path = os.path.expanduser(export_path)
    if not os.path.isdir(export_path):
        log("error", f"Export directory not found: {export_path}")
        sys.exit(2)

    max_messages = int(config.get("max_messages", 0))

    # ── Load users for display name lookup ───────────────────────────────
    users_map = {}
    users_file = os.path.join(export_path, "users.json")
    users_data = load_json(users_file)
    if users_data:
        for user in users_data:
            uid = user.get("id", "")
            profile = user.get("profile", {})
            display = (
                profile.get("display_name")
                or profile.get("real_name")
                or user.get("real_name")
                or user.get("name", uid)
            )
            users_map[uid] = display
    else:
        log("warn", "No users.json found, author names will be user IDs")

    # ── Load channels ────────────────────────────────────────────────────
    channels = []
    for filename in ["channels.json", "groups.json", "mpims.json", "dms.json"]:
        channels_file = os.path.join(export_path, filename)
        data = load_json(channels_file)
        if data:
            channels.extend(data)

    if not channels:
        log("error", "No channel data found in export (expected channels.json)")
        sys.exit(2)

    channel_name_map = {}
    for ch in channels:
        ch_id = ch.get("id", "")
        ch_name = ch.get("name", ch_id)
        purpose = ch.get("purpose", {})
        purpose_text = purpose.get("value", "") if isinstance(purpose, dict) else str(purpose)
        topic = ch.get("topic", {})
        topic_text = topic.get("value", "") if isinstance(topic, dict) else str(topic)

        channel_name_map[ch_id] = ch_name
        # Also map folder name -> channel id (folders are named by channel name)
        channel_name_map[ch_name] = ch_id

        members = ch.get("members", [])

        emit({
            "upsert": "channel",
            "external_id": ch_id,
            "fields": {
                "name": ch_name,
                "purpose": purpose_text[:5000],
                "topic": topic_text[:500],
                "is_archived": ch.get("is_archived", False),
                "member_count": len(members) if isinstance(members, list) else 0,
            }
        })

    log("info", f"Loaded {len(channels)} channels")

    # ── Load messages from channel folders ───────────────────────────────
    msg_count = 0
    thread_parents = {}  # ts -> external_id, for parent linking

    # Iterate channel directories
    for entry in sorted(os.listdir(export_path)):
        channel_dir = os.path.join(export_path, entry)
        if not os.path.isdir(channel_dir) or entry.startswith("."):
            continue

        # Find the channel ID for this folder
        ch_id = channel_name_map.get(entry)
        if not ch_id:
            # Folder name might be the channel ID itself
            ch_id = entry

        # Read all daily JSON files sorted chronologically
        day_files = sorted(glob.glob(os.path.join(channel_dir, "*.json")))

        for day_file in day_files:
            messages = load_json(day_file)
            if not messages or not isinstance(messages, list):
                continue

            for msg in messages:
                if max_messages > 0 and msg_count >= max_messages:
                    break

                ts = msg.get("ts", "")
                if not ts:
                    continue

                # Skip subtypes that aren't real messages
                subtype = msg.get("subtype", "")
                if subtype in ("channel_join", "channel_leave", "channel_topic",
                               "channel_purpose", "channel_name", "channel_archive",
                               "channel_unarchive", "pinned_item", "unpinned_item"):
                    continue

                user_id = msg.get("user", msg.get("bot_id", "unknown"))
                author = users_map.get(user_id, user_id)
                text = msg.get("text", "")

                # Expand user mentions: <@U1234> -> @display_name
                for uid, uname in users_map.items():
                    text = text.replace(f"<@{uid}>", f"@{uname}")

                timestamp_iso = ts_to_iso(ts)
                thread_ts = msg.get("thread_ts", "")
                reply_count = msg.get("reply_count", 0)

                # Build reactions string
                reactions = msg.get("reactions", [])
                reactions_str = ""
                if reactions:
                    parts = []
                    for r in reactions:
                        name = r.get("name", "")
                        count = r.get("count", 0)
                        if name:
                            parts.append(f":{name}: {count}")
                    reactions_str = ", ".join(parts)

                msg_id = f"{ch_id}_{ts}"

                emit({
                    "upsert": "message",
                    "external_id": msg_id,
                    "fields": {
                        "text": text[:50000],
                        "author": author[:200],
                        "timestamp": timestamp_iso,
                        "thread_ts": thread_ts,
                        "reply_count": reply_count,
                        "reactions": reactions_str[:1000],
                        "channel_id": ch_id,
                    }
                })

                # Track thread parents for linking
                if not thread_ts or thread_ts == ts:
                    thread_parents[ts] = msg_id

                msg_count += 1

            if max_messages > 0 and msg_count >= max_messages:
                break

        if max_messages > 0 and msg_count >= max_messages:
            break

    log("info", f"Synced {msg_count} messages across {len(channels)} channels")


if __name__ == "__main__":
    main()
