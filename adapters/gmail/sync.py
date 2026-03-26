#!/usr/bin/env python3
"""
Gmail adapter for Spacedrive.

Syncs threads, messages, labels, and attachments from Gmail via the Gmail API.
Uses only Python stdlib (no pip dependencies).

Protocol: reads JSON config from stdin, writes JSONL operations to stdout.

Incremental sync: uses Gmail history ID as cursor. First sync fetches all
messages; subsequent syncs fetch only changes since the last history ID.
"""

import json
import sys
import time
import urllib.request
import urllib.error
import urllib.parse
import base64
import email
import email.utils
import email.header
from datetime import datetime, timezone

# ── Constants ────────────────────────────────────────────────────────────────

GMAIL_API = "https://gmail.googleapis.com/gmail/v1"
MAX_RETRIES = 3
RETRY_DELAY = 2  # seconds, doubled on each retry
BATCH_SIZE = 100  # messages per page

# ── Helpers ──────────────────────────────────────────────────────────────────


def log(level: str, message: str):
    """Emit a log operation."""
    print(json.dumps({"log": level, "message": message}), flush=True)


def emit(operation: dict):
    """Emit a JSONL operation."""
    print(json.dumps(operation), flush=True)


def api_get(path: str, token: str, params: dict = None) -> dict:
    """Make an authenticated GET request to the Gmail API with retries."""
    url = f"{GMAIL_API}{path}"
    if params:
        url += "?" + urllib.parse.urlencode(params)

    headers = {"Authorization": f"Bearer {token}"}
    req = urllib.request.Request(url, headers=headers)

    for attempt in range(MAX_RETRIES):
        try:
            with urllib.request.urlopen(req, timeout=30) as resp:
                return json.loads(resp.read().decode("utf-8"))
        except urllib.error.HTTPError as e:
            if e.code == 429 or e.code >= 500:
                delay = RETRY_DELAY * (2 ** attempt)
                log("warn", f"API returned {e.code}, retrying in {delay}s...")
                time.sleep(delay)
                continue
            elif e.code == 401:
                log("error", "OAuth token expired or invalid (401 Unauthorized)")
                sys.exit(2)
            elif e.code == 403:
                log("error", f"Access denied (403 Forbidden): {e.read().decode('utf-8', errors='replace')}")
                sys.exit(2)
            else:
                raise
        except urllib.error.URLError as e:
            if attempt < MAX_RETRIES - 1:
                delay = RETRY_DELAY * (2 ** attempt)
                log("warn", f"Network error: {e}, retrying in {delay}s...")
                time.sleep(delay)
                continue
            raise

    log("error", f"Failed after {MAX_RETRIES} retries")
    sys.exit(2)


def decode_header(header_value: str) -> str:
    """Decode a MIME-encoded email header."""
    if not header_value:
        return ""
    decoded_parts = email.header.decode_header(header_value)
    result = []
    for part, charset in decoded_parts:
        if isinstance(part, bytes):
            result.append(part.decode(charset or "utf-8", errors="replace"))
        else:
            result.append(part)
    return " ".join(result)


def get_header(headers: list, name: str) -> str:
    """Extract a header value from Gmail's header list."""
    for h in headers:
        if h.get("name", "").lower() == name.lower():
            return h.get("value", "")
    return ""


def extract_body(payload: dict) -> str:
    """Extract the plain text body from a Gmail message payload."""
    mime_type = payload.get("mimeType", "")

    # Direct text/plain
    if mime_type == "text/plain":
        data = payload.get("body", {}).get("data", "")
        if data:
            return base64.urlsafe_b64decode(data).decode("utf-8", errors="replace")

    # Multipart — recurse
    parts = payload.get("parts", [])
    for part in parts:
        part_mime = part.get("mimeType", "")
        if part_mime == "text/plain":
            data = part.get("body", {}).get("data", "")
            if data:
                return base64.urlsafe_b64decode(data).decode("utf-8", errors="replace")

    # Fallback: try text/html
    if mime_type == "text/html":
        data = payload.get("body", {}).get("data", "")
        if data:
            html = base64.urlsafe_b64decode(data).decode("utf-8", errors="replace")
            # Strip HTML tags (basic)
            import re
            return re.sub(r"<[^>]+>", "", html).strip()

    for part in parts:
        body = extract_body(part)
        if body:
            return body

    return ""


def extract_attachments(payload: dict, message_id: str) -> list:
    """Extract attachment metadata from a Gmail message payload."""
    attachments = []
    parts = payload.get("parts", [])

    for part in parts:
        filename = part.get("filename", "")
        if filename:
            body = part.get("body", {})
            attachments.append({
                "filename": filename,
                "mime_type": part.get("mimeType", "application/octet-stream"),
                "size": body.get("size", 0),
            })
        # Recurse into nested parts
        attachments.extend(extract_attachments(part, message_id))

    return attachments


def parse_date(date_str: str) -> str:
    """Parse an email date header into ISO 8601 format, normalized to UTC."""
    if not date_str:
        return datetime.now(timezone.utc).isoformat()
    try:
        parsed = email.utils.parsedate_to_datetime(date_str)
        return parsed.astimezone(timezone.utc).isoformat()
    except Exception:
        return datetime.now(timezone.utc).isoformat()


# ── Sync Logic ───────────────────────────────────────────────────────────────


def sync_labels(token: str, user: str, label_filter: list = None):
    """Fetch and emit all Gmail labels."""
    data = api_get(f"/users/{user}/labels", token)
    labels = data.get("labels", [])

    count = 0
    for label in labels:
        label_id = label["id"]

        # Apply label filter if specified
        if label_filter and label["name"] not in label_filter:
            continue

        # Get full label details
        detail = api_get(f"/users/{user}/labels/{label_id}", token)

        color_bg = ""
        if "color" in detail:
            color_bg = detail["color"].get("backgroundColor", "")

        label_type = detail.get("type", "user").lower()

        emit({
            "upsert": "label",
            "external_id": label_id,
            "fields": {
                "name": detail.get("name", label_id),
                "color": color_bg,
                "type": label_type,
            }
        })
        count += 1

    log("info", f"Synced {count} labels")
    return count


def sync_messages_full(token: str, user: str, max_results: int, label_filter: list = None):
    """Full initial sync: fetch all messages."""
    log("info", "Starting full sync...")

    # Build query params
    params = {"maxResults": min(BATCH_SIZE, max_results)}
    if label_filter:
        params["labelIds"] = ",".join(label_filter)

    total_fetched = 0
    threads_seen = set()
    page_token = None

    while total_fetched < max_results:
        if page_token:
            params["pageToken"] = page_token

        # List messages
        data = api_get(f"/users/{user}/messages", token, params)
        messages = data.get("messages", [])

        if not messages:
            break

        for msg_ref in messages:
            if total_fetched >= max_results:
                break

            msg_id = msg_ref["id"]

            # Fetch full message
            msg = api_get(
                f"/users/{user}/messages/{msg_id}",
                token,
                {"format": "full"}
            )

            process_message(msg, user, threads_seen)
            total_fetched += 1

            if total_fetched % 50 == 0:
                log("info", f"Processed {total_fetched} messages...")

        page_token = data.get("nextPageToken")
        if not page_token:
            break

    log("info", f"Full sync complete: {total_fetched} messages, {len(threads_seen)} threads")
    return total_fetched


def sync_messages_incremental(token: str, user: str, history_id: str, max_results: int):
    """Incremental sync: fetch changes since last history ID."""
    log("info", f"Incremental sync from history ID {history_id}...")

    params = {
        "startHistoryId": history_id,
        "maxResults": min(BATCH_SIZE, max_results),
        "historyTypes": "messageAdded,messageDeleted,labelAdded,labelRemoved",
    }

    total_changes = 0
    threads_seen = set()
    page_token = None

    while True:
        if page_token:
            params["pageToken"] = page_token

        try:
            data = api_get(f"/users/{user}/history", token, params)
        except urllib.error.HTTPError as e:
            if e.code == 404:
                # History ID too old — need full sync
                log("warn", "History ID expired, falling back to full sync")
                return sync_messages_full(token, user, max_results)
            raise

        history = data.get("history", [])

        for record in history:
            # Messages added
            for added in record.get("messagesAdded", []):
                msg_ref = added.get("message", {})
                msg_id = msg_ref.get("id")
                if msg_id:
                    msg = api_get(
                        f"/users/{user}/messages/{msg_id}",
                        token,
                        {"format": "full"}
                    )
                    process_message(msg, user, threads_seen)
                    total_changes += 1

            # Messages deleted
            for deleted in record.get("messagesDeleted", []):
                msg_ref = deleted.get("message", {})
                msg_id = msg_ref.get("id")
                if msg_id:
                    emit({"delete": "message", "external_id": msg_id})
                    total_changes += 1

            # Label changes — re-fetch the message to update links
            for label_change in record.get("labelsAdded", []) + record.get("labelsRemoved", []):
                msg_ref = label_change.get("message", {})
                msg_id = msg_ref.get("id")
                if msg_id:
                    try:
                        msg = api_get(
                            f"/users/{user}/messages/{msg_id}",
                            token,
                            {"format": "metadata", "metadataHeaders": ""}
                        )
                        # Re-link labels
                        label_ids = msg.get("labelIds", [])
                        for label_id in label_ids:
                            emit({
                                "link": "message",
                                "id": msg_id,
                                "to": "label",
                                "to_id": label_id,
                            })
                    except Exception:
                        pass  # message may have been deleted

        page_token = data.get("nextPageToken")
        if not page_token:
            break

    new_history_id = data.get("historyId", history_id)
    log("info", f"Incremental sync complete: {total_changes} changes")

    return total_changes, new_history_id


def process_message(msg: dict, user: str, threads_seen: set):
    """Process a single Gmail message: emit thread (if new), message, attachments, and label links."""
    msg_id = msg["id"]
    thread_id = msg.get("threadId", msg_id)
    payload = msg.get("payload", {})
    headers = payload.get("headers", [])

    # Extract message fields
    subject = decode_header(get_header(headers, "Subject"))
    from_addr = get_header(headers, "From")
    to_addr = get_header(headers, "To")
    cc_addr = get_header(headers, "Cc")
    date_str = get_header(headers, "Date")
    date_iso = parse_date(date_str)

    body = extract_body(payload)
    # Truncate very long bodies for storage
    if len(body) > 50000:
        body = body[:50000] + "..."

    label_ids = msg.get("labelIds", [])
    is_read = "UNREAD" not in label_ids
    is_starred = "STARRED" in label_ids

    # Emit thread (upsert — first message wins for subject, subsequent updates are fine)
    if thread_id not in threads_seen:
        threads_seen.add(thread_id)
        snippet = msg.get("snippet", "")
        # The thread subject is typically the first message's subject
        emit({
            "upsert": "thread",
            "external_id": thread_id,
            "fields": {
                "subject": subject,
                "last_date": date_iso,
                "message_count": 1,
                "snippet": snippet,
            }
        })

    # Emit message
    emit({
        "upsert": "message",
        "external_id": msg_id,
        "fields": {
            "subject": subject,
            "body": body,
            "from": from_addr,
            "to": to_addr,
            "cc": cc_addr,
            "date": date_iso,
            "is_read": is_read,
            "is_starred": is_starred,
            "thread_id": thread_id,
        }
    })

    # Emit attachments
    attachments = extract_attachments(payload, msg_id)
    for i, att in enumerate(attachments):
        att_id = f"{msg_id}_att_{i}"
        emit({
            "upsert": "attachment",
            "external_id": att_id,
            "fields": {
                "filename": att["filename"],
                "mime_type": att["mime_type"],
                "size": att["size"],
                "message_id": msg_id,
            }
        })

    # Link message to labels
    for label_id in label_ids:
        emit({
            "link": "message",
            "id": msg_id,
            "to": "label",
            "to_id": label_id,
        })


# ── Main ─────────────────────────────────────────────────────────────────────


def main():
    # Read input from Spacedrive
    try:
        input_data = json.loads(sys.stdin.read())
    except json.JSONDecodeError as e:
        log("error", f"Invalid input JSON: {e}")
        sys.exit(2)

    config = input_data.get("config", {})
    cursor = input_data.get("cursor")

    # Required config
    oauth_token = config.get("oauth_token")
    if not oauth_token:
        log("error", "Missing required config: oauth_token")
        sys.exit(2)

    user_email = config.get("email", "me")
    max_results = int(config.get("max_results", 500))

    # Optional label filter
    labels_str = config.get("labels", "")
    label_filter = [l.strip() for l in labels_str.split(",") if l.strip()] if labels_str else None

    # Use "me" as the Gmail user (authenticated user)
    user = "me"

    try:
        # Step 1: Sync labels (always full)
        sync_labels(oauth_token, user, label_filter)

        # Step 2: Sync messages
        if cursor:
            # Incremental sync
            total, new_history_id = sync_messages_incremental(
                oauth_token, user, cursor, max_results
            )
            # Emit new cursor
            emit({"cursor": str(new_history_id)})
        else:
            # Full sync — get current profile for history ID
            profile = api_get(f"/users/{user}/profile", oauth_token)
            current_history_id = profile.get("historyId", "")

            total = sync_messages_full(
                oauth_token, user, max_results, label_filter
            )

            # Set cursor to current history ID for next incremental sync
            if current_history_id:
                emit({"cursor": str(current_history_id)})

        log("info", f"Sync complete: {total} messages processed")

    except urllib.error.HTTPError as e:
        body = e.read().decode("utf-8", errors="replace") if hasattr(e, "read") else str(e)
        log("error", f"Gmail API error {e.code}: {body}")
        sys.exit(1)  # Partial failure — some records may have been written
    except Exception as e:
        log("error", f"Unexpected error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
