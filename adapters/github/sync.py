#!/usr/bin/env python3
"""
GitHub adapter for Spacedrive.

Indexes issues, pull requests, and comments from GitHub repositories
using the REST API. Supports incremental sync via `updated_at` cursor.

Requires a personal access token with `repo` scope (or fine-grained
token with Issues and Pull Requests read permissions).
"""

import json
import sys
import urllib.request
import urllib.error
from datetime import datetime, timezone


API_BASE = "https://api.github.com"


def log(level: str, message: str):
    print(json.dumps({"log": level, "message": message}), flush=True)


def emit(operation: dict):
    print(json.dumps(operation), flush=True)


def api_get(path: str, token: str, params: dict = None) -> list:
    """Make a GET request to GitHub API. Handles pagination, returns all results."""
    results = []
    url = f"{API_BASE}{path}"

    if params:
        query_parts = []
        for k, v in params.items():
            query_parts.append(f"{k}={v}")
        url += "?" + "&".join(query_parts)

    page = 1
    while True:
        separator = "&" if "?" in url else "?"
        page_url = f"{url}{separator}page={page}&per_page=100"

        req = urllib.request.Request(page_url)
        req.add_header("Authorization", f"Bearer {token}")
        req.add_header("Accept", "application/vnd.github+json")
        req.add_header("X-GitHub-Api-Version", "2022-11-28")
        req.add_header("User-Agent", "spacedrive-adapter/0.1")

        try:
            with urllib.request.urlopen(req) as resp:
                data = json.loads(resp.read().decode())
        except urllib.error.HTTPError as e:
            if e.code == 403:
                # Rate limited — stop pagination
                log("warn", f"GitHub API rate limited on {path}")
                break
            elif e.code == 404:
                log("warn", f"Not found: {path}")
                return []
            else:
                raise

        if not isinstance(data, list):
            # Single object response (e.g., /repos/owner/repo)
            return [data]

        results.extend(data)

        if len(data) < 100:
            break
        page += 1

        # Safety limit
        if page > 50:
            log("warn", f"Pagination limit reached for {path}")
            break

    return results


def parse_iso(dt_str: str) -> str:
    """Normalize GitHub's ISO 8601 timestamps to UTC."""
    if not dt_str:
        return ""
    try:
        # GitHub returns "2025-01-15T10:30:00Z" format
        s = dt_str.replace("Z", "+00:00")
        dt = datetime.fromisoformat(s)
        return dt.astimezone(timezone.utc).isoformat()
    except (ValueError, TypeError):
        return dt_str


def main():
    try:
        input_data = json.loads(sys.stdin.read())
    except json.JSONDecodeError as e:
        log("error", f"Invalid input JSON: {e}")
        sys.exit(2)

    config = input_data.get("config", {})
    cursor = input_data.get("cursor")

    token = config.get("token", "")
    if not token:
        log("error", "Missing required config: token")
        sys.exit(2)

    repos_str = config.get("repos", "")
    include_prs = config.get("include_prs", True)
    if isinstance(include_prs, str):
        include_prs = include_prs.lower() in ("true", "1", "yes")
    max_items = int(config.get("max_items", 500))

    # ── Determine repos to index ─────────────────────────────────────────
    if repos_str:
        repo_list = [r.strip() for r in repos_str.split(",") if r.strip()]
    else:
        # Fetch all repos the user has access to
        log("info", "No repos specified, fetching all accessible repos")
        try:
            all_repos = api_get("/user/repos", token, {"sort": "updated", "type": "all"})
            repo_list = [r["full_name"] for r in all_repos if r.get("full_name")]
        except Exception as e:
            log("error", f"Failed to fetch repos: {e}")
            sys.exit(1)

    log("info", f"Indexing {len(repo_list)} repositories")

    max_updated = cursor or ""
    total_issues = 0
    total_comments = 0

    for repo_full_name in repo_list:
        # ── Upsert repository ────────────────────────────────────────────
        try:
            repo_data = api_get(f"/repos/{repo_full_name}", token)
            if not repo_data:
                log("warn", f"Could not fetch repo: {repo_full_name}")
                continue
            repo = repo_data[0]
        except Exception as e:
            log("warn", f"Failed to fetch repo {repo_full_name}: {e}")
            continue

        repo_id = str(repo.get("id", repo_full_name))

        emit({
            "upsert": "repository",
            "external_id": repo_id,
            "fields": {
                "name": repo.get("name", ""),
                "full_name": repo.get("full_name", repo_full_name),
                "description": (repo.get("description") or "")[:5000],
                "language": repo.get("language") or "",
                "stars": repo.get("stargazers_count", 0),
                "url": repo.get("html_url", ""),
            }
        })

        # ── Fetch issues (and PRs if enabled) ────────────────────────────
        params = {
            "state": "all",
            "sort": "updated",
            "direction": "desc",
        }
        if cursor:
            params["since"] = cursor

        try:
            issues = api_get(f"/repos/{repo_full_name}/issues", token, params)
        except Exception as e:
            log("warn", f"Failed to fetch issues for {repo_full_name}: {e}")
            continue

        repo_issue_count = 0

        for item in issues:
            if repo_issue_count >= max_items:
                break

            is_pr = "pull_request" in item
            if is_pr and not include_prs:
                continue

            issue_number = item.get("number", 0)
            issue_id = f"{repo_full_name}#{issue_number}"

            title = item.get("title", "")
            body = item.get("body") or ""
            author = item.get("user", {}).get("login", "") if item.get("user") else ""
            state = item.get("state", "")
            url = item.get("html_url", "")
            created_at = parse_iso(item.get("created_at", ""))
            updated_at = parse_iso(item.get("updated_at", ""))
            closed_at = parse_iso(item.get("closed_at", ""))
            comments_count = item.get("comments", 0)

            # Labels
            labels = []
            for label in item.get("labels", []):
                if isinstance(label, dict):
                    labels.append(label.get("name", ""))
                elif isinstance(label, str):
                    labels.append(label)
            labels_str = ", ".join(labels)

            emit({
                "upsert": "issue",
                "external_id": issue_id,
                "fields": {
                    "title": title[:500],
                    "body": body[:50000],
                    "author": author,
                    "state": state,
                    "number": issue_number,
                    "url": url,
                    "is_pr": is_pr,
                    "labels": labels_str[:1000],
                    "comments_count": comments_count,
                    "created_at": created_at,
                    "updated_at": updated_at,
                    "closed_at": closed_at,
                    "repository_id": repo_id,
                }
            })
            total_issues += 1
            repo_issue_count += 1

            # Track latest update for cursor
            raw_updated = item.get("updated_at", "")
            if raw_updated > max_updated:
                max_updated = raw_updated

            # ── Fetch comments for this issue ────────────────────────────
            if comments_count > 0:
                try:
                    comments = api_get(
                        f"/repos/{repo_full_name}/issues/{issue_number}/comments",
                        token
                    )
                    for comment in comments:
                        comment_id = str(comment.get("id", ""))
                        if not comment_id:
                            continue

                        emit({
                            "upsert": "comment",
                            "external_id": comment_id,
                            "fields": {
                                "body": (comment.get("body") or "")[:50000],
                                "author": comment.get("user", {}).get("login", "") if comment.get("user") else "",
                                "created_at": parse_iso(comment.get("created_at", "")),
                                "url": comment.get("html_url", ""),
                                "issue_id": issue_id,
                            }
                        })
                        total_comments += 1
                except Exception as e:
                    log("warn", f"Failed to fetch comments for {issue_id}: {e}")

        log("info", f"{repo_full_name}: {repo_issue_count} issues/PRs")

    # Emit cursor
    if max_updated:
        emit({"cursor": max_updated})

    log("info", f"Synced {total_issues} issues/PRs and {total_comments} comments from {len(repo_list)} repos")


if __name__ == "__main__":
    main()
