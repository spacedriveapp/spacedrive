#!/usr/bin/env python3
"""
Chrome Bookmarks adapter for Spacedrive.

Reads the Chromium Bookmarks JSON file (works with Chrome, Arc, Brave, Edge).
Emits folders and bookmarks as JSONL operations.

Full sync every time (file is small, no cursor needed).
"""

import json
import sys
import os
from datetime import datetime, timezone


def log(level: str, message: str):
    print(json.dumps({"log": level, "message": message}), flush=True)


def emit(operation: dict):
    print(json.dumps(operation), flush=True)


def chromium_time_to_iso(timestamp_str: str) -> str:
    """Convert Chromium timestamp (microseconds since 1601-01-01) to ISO 8601."""
    try:
        ts = int(timestamp_str)
        if ts == 0:
            return ""
        # Chromium epoch: 1601-01-01 00:00:00 UTC
        # Unix epoch offset: 11644473600 seconds
        unix_seconds = (ts / 1_000_000) - 11644473600
        if unix_seconds < 0:
            return ""
        dt = datetime.fromtimestamp(unix_seconds, tz=timezone.utc)
        return dt.isoformat()
    except (ValueError, OSError):
        return ""


def process_node(node: dict, path: str = ""):
    """Recursively process a bookmark tree node."""
    node_type = node.get("type", "")
    name = node.get("name", "")

    if node_type == "folder":
        folder_path = f"{path}/{name}" if path else name
        folder_id = node.get("guid", node.get("id", name))

        emit({
            "upsert": "folder",
            "external_id": folder_id,
            "fields": {
                "name": name,
                "path": folder_path,
            }
        })

        for child in node.get("children", []):
            process_node(child, folder_path)

    elif node_type == "url":
        bookmark_id = node.get("guid", node.get("id", ""))
        url = node.get("url", "")
        title = name or url
        date_added = chromium_time_to_iso(node.get("date_added", "0"))
        folder_id = None

        # Find parent folder ID from path
        if path:
            # The folder_id for belongs_to linking
            pass

        emit({
            "upsert": "bookmark",
            "external_id": bookmark_id,
            "fields": {
                "title": title,
                "url": url,
                "date_added": date_added,
                "folder_path": path or "Uncategorized",
            }
        })


def main():
    try:
        input_data = json.loads(sys.stdin.read())
    except json.JSONDecodeError as e:
        log("error", f"Invalid input JSON: {e}")
        sys.exit(2)

    config = input_data.get("config", {})
    bookmarks_path = config.get("bookmarks_path", "")

    if not bookmarks_path:
        log("error", "Missing required config: bookmarks_path")
        sys.exit(2)

    bookmarks_path = os.path.expanduser(bookmarks_path)

    if not os.path.exists(bookmarks_path):
        log("error", f"Bookmarks file not found: {bookmarks_path}")
        sys.exit(2)

    try:
        with open(bookmarks_path, "r", encoding="utf-8") as f:
            data = json.load(f)
    except Exception as e:
        log("error", f"Failed to read bookmarks file: {e}")
        sys.exit(2)

    roots = data.get("roots", {})
    total = 0

    for root_name, root_node in roots.items():
        if isinstance(root_node, dict) and root_node.get("type") == "folder":
            process_node(root_node)

    # Count what we emitted (approximate from the file)
    def count_bookmarks(node):
        c = 0
        if node.get("type") == "url":
            c = 1
        for child in node.get("children", []):
            c += count_bookmarks(child)
        return c

    for root_node in roots.values():
        if isinstance(root_node, dict):
            total += count_bookmarks(root_node)

    log("info", f"Synced {total} bookmarks")


if __name__ == "__main__":
    main()
