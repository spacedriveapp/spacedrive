#!/usr/bin/env python3
"""
Obsidian/Markdown vault adapter for Spacedrive.

Recursively scans a directory for .md files, extracts YAML frontmatter
and body text, and emits notes. Detects [[wikilinks]] between notes and
creates many_to_many links.

Incremental sync via file modification time cursor.
"""

import json
import sys
import os
import re
import fnmatch
import hashlib
from datetime import datetime, timezone


def log(level: str, message: str):
    print(json.dumps({"log": level, "message": message}), flush=True)


def normalize_date(value: str, fallback: str) -> str:
    """Try to parse an arbitrary date string into UTC ISO 8601.

    Handles common frontmatter formats:
      - 2025-01-15T10:30:00Z
      - 2025-01-15T10:30:00+05:00
      - 2025-01-15T10:30:00
      - 2025-01-15 10:30:00
      - 2025-01-15

    Falls back to *fallback* (which should already be UTC ISO 8601) if parsing
    fails, so we never store unparseable user strings.
    """
    if not value:
        return fallback
    s = str(value).strip()
    # Try datetime.fromisoformat (Python 3.11+ accepts Z, earlier versions don't)
    for candidate in (s, s.replace("Z", "+00:00")):
        try:
            dt = datetime.fromisoformat(candidate)
            if dt.tzinfo is None:
                dt = dt.replace(tzinfo=timezone.utc)
            return dt.astimezone(timezone.utc).isoformat()
        except (ValueError, TypeError):
            continue
    # Try date-only: YYYY-MM-DD
    try:
        dt = datetime.strptime(s[:10], "%Y-%m-%d").replace(tzinfo=timezone.utc)
        return dt.isoformat()
    except (ValueError, TypeError):
        pass
    return fallback


def emit(operation: dict):
    print(json.dumps(operation), flush=True)


def parse_frontmatter(content: str):
    """Extract YAML frontmatter and body from markdown content."""
    frontmatter = {}
    body = content

    if content.startswith("---"):
        parts = content.split("---", 2)
        if len(parts) >= 3:
            yaml_str = parts[1].strip()
            body = parts[2].strip()

            # Simple YAML key: value parser (no dependency on pyyaml)
            for line in yaml_str.split("\n"):
                line = line.strip()
                if ":" in line and not line.startswith("#"):
                    key, _, value = line.partition(":")
                    key = key.strip()
                    value = value.strip()
                    # Handle lists
                    if value.startswith("[") and value.endswith("]"):
                        items = [
                            v.strip().strip("'\"")
                            for v in value[1:-1].split(",")
                            if v.strip()
                        ]
                        frontmatter[key] = items
                    elif value.startswith("'") or value.startswith('"'):
                        frontmatter[key] = value.strip("'\"")
                    else:
                        frontmatter[key] = value

    return frontmatter, body


def extract_wikilinks(body: str) -> list:
    """Extract [[wikilink]] targets from markdown body."""
    # Match [[Page Name]] and [[Page Name|Display Text]]
    pattern = r"\[\[([^\]|]+)(?:\|[^\]]+)?\]\]"
    matches = re.findall(pattern, body)
    # Normalize: strip whitespace, convert to lowercase for matching
    return list(set(m.strip() for m in matches))


def file_id(path: str, vault_root: str) -> str:
    """Generate a stable ID from the relative path."""
    rel = os.path.relpath(path, vault_root)
    return hashlib.sha256(rel.encode("utf-8")).hexdigest()[:16]


def should_exclude(rel_path: str, patterns: list) -> bool:
    """Check if a relative path matches any exclude pattern."""
    parts = rel_path.split(os.sep)
    for pattern in patterns:
        pattern = pattern.strip()
        if not pattern:
            continue
        # Match against directory components and filename
        for part in parts:
            if fnmatch.fnmatch(part, pattern):
                return True
        if fnmatch.fnmatch(rel_path, pattern):
            return True
    return False


def main():
    try:
        input_data = json.loads(sys.stdin.read())
    except json.JSONDecodeError as e:
        log("error", f"Invalid input JSON: {e}")
        sys.exit(2)

    config = input_data.get("config", {})
    cursor = input_data.get("cursor")

    vault_path = config.get("vault_path", "")
    if not vault_path:
        log("error", "Missing required config: vault_path")
        sys.exit(2)

    vault_path = os.path.expanduser(vault_path)
    if not os.path.isdir(vault_path):
        log("error", f"Vault directory not found: {vault_path}")
        sys.exit(2)

    # Parse exclude patterns
    exclude_str = config.get("exclude_patterns", "")
    exclude_patterns = [p.strip() for p in exclude_str.split(",") if p.strip()] if exclude_str else []
    # Always exclude hidden dirs
    exclude_patterns.extend([".obsidian", ".git", ".trash", "node_modules"])

    # Parse cursor (last sync timestamp as float)
    last_sync = float(cursor) if cursor else 0

    # Collect all markdown files
    md_files = []
    for root, dirs, files in os.walk(vault_path):
        # Skip excluded directories
        dirs[:] = [d for d in dirs if not should_exclude(d, exclude_patterns)]

        for fname in files:
            if not fname.endswith(".md"):
                continue

            full_path = os.path.join(root, fname)
            rel_path = os.path.relpath(full_path, vault_path)

            if should_exclude(rel_path, exclude_patterns):
                continue

            mtime = os.path.getmtime(full_path)

            # For incremental sync, only process modified files
            if last_sync > 0 and mtime <= last_sync:
                continue

            md_files.append((full_path, rel_path, mtime))

    log("info", f"Found {len(md_files)} markdown files to process")

    # First pass: emit all notes
    note_links = {}  # note_id -> [wikilink_targets]
    title_to_id = {}  # lowercase title -> note_id (for link resolution)
    max_mtime = last_sync
    count = 0

    for full_path, rel_path, mtime in md_files:
        try:
            with open(full_path, "r", encoding="utf-8", errors="replace") as f:
                content = f.read()
        except Exception as e:
            log("warn", f"Failed to read {rel_path}: {e}")
            continue

        frontmatter, body = parse_frontmatter(content)

        # Derive title from filename (without .md)
        title = os.path.splitext(os.path.basename(full_path))[0]

        # Extract metadata
        tags_raw = frontmatter.get("tags", [])
        if isinstance(tags_raw, list):
            tags = ", ".join(str(t) for t in tags_raw)
        else:
            tags = str(tags_raw)

        # Also extract #tags from body
        body_tags = re.findall(r"(?:^|\s)#([a-zA-Z][a-zA-Z0-9_/-]*)", body)
        if body_tags:
            all_tags = set(t.strip() for t in tags.split(",") if t.strip())
            all_tags.update(body_tags)
            tags = ", ".join(sorted(all_tags))

        word_count = len(body.split())

        # Dates — normalize to UTC ISO 8601 for consistent temporal queries
        ctime = os.path.getctime(full_path)
        ctime_iso = datetime.fromtimestamp(ctime, tz=timezone.utc).isoformat()
        modified = datetime.fromtimestamp(mtime, tz=timezone.utc).isoformat()

        raw_created = frontmatter.get("created", frontmatter.get("date", ""))
        created = normalize_date(str(raw_created), fallback=ctime_iso)

        # Truncate long bodies
        if len(body) > 50000:
            body = body[:50000] + "..."

        nid = file_id(full_path, vault_path)
        title_to_id[title.lower()] = nid

        emit({
            "upsert": "note",
            "external_id": nid,
            "fields": {
                "title": title,
                "body": body,
                "path": rel_path,
                "tags": tags,
                "created": created,
                "modified": modified,
                "word_count": word_count,
            }
        })
        count += 1

        # Collect wikilinks for second pass
        wikilinks = extract_wikilinks(body)
        if wikilinks:
            note_links[nid] = wikilinks

        if mtime > max_mtime:
            max_mtime = mtime

    # Second pass: emit links between notes
    link_count = 0
    for source_id, targets in note_links.items():
        for target_name in targets:
            target_id = title_to_id.get(target_name.lower())
            if target_id and target_id != source_id:
                emit({
                    "link": "note",
                    "id": source_id,
                    "to": "note",
                    "to_id": target_id,
                })
                link_count += 1

    # Emit cursor
    if max_mtime > 0:
        emit({"cursor": str(max_mtime)})

    log("info", f"Synced {count} notes, {link_count} links")


if __name__ == "__main__":
    main()
