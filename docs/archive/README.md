# Adapter Sources

Spacedrive is a set of sources. Some are filesystems. Some are adapters — a
Gmail account, an Obsidian vault, a Slack workspace, browser history. Both kinds
are sources: an origin, an ingest, and a store. This page covers the adapter
ingest.

Design: `docs/core/design/archive.md`.
Direction: `docs/plans/2026-08-22-source-convergence.md`.

The store itself is `sd-store` (`crates/store`) — one file per source, the same
shape whatever wrote it. This crate, `sd-archive`, is the adapter ingest over
it, plus the registry and the cross-source search router.

## Storage layout

```text
<library>/archive/
  registry.db              # sources, data_types
  adapters/                # installed adapter directories
  sources/
    <source_id>/
      data.db              # record, facet_*, content, edge,
                           # record_overlay, search_index
```

`registry.db` and the `<library>/archive/` root are both scheduled to go: the
convergence plan folds the registry into `library.db` and moves source stores
under `SourceDirs` as `sources/<id>/source.db`.

## Bundled adapters

Eleven ship in `adapters/`, each a directory with `adapter.toml`, `icon.svg`
and `sync.py`:

`apple-notes` · `chrome-bookmarks` · `chrome-history` · `github` · `gmail` ·
`macos-calendar` · `macos-contacts` · `obsidian` · `opencode` ·
`safari-history` · `slack`

They are Python because Python is already on macOS. The protocol is not
language-specific — anything that reads stdin and writes JSONL works.

Adapters are discovered from the adapters directory at startup. In a dev tree
they are copied out of the workspace `adapters/` folder at engine init
(`core/src/data/manager.rs`), which resolves the workspace through
`CARGO_MANIFEST_DIR` at compile time and therefore only works from a cargo
build. Shipping needs this to read from app resources instead.

## Writing an adapter

### 1. The manifest

`adapters/my-adapter/adapter.toml` declares how to run it, what it needs
configured, and the shape of what it produces.

```toml
[adapter]
id = "my-adapter"
name = "My Adapter"
description = "What it indexes"
version = "0.1.0"
author = "you"
license = "MIT"
icon = "note"
trust_tier = "external"          # authored | collaborative | external

[adapter.runtime]
command = "python3 sync.py"
timeout = 300
schedule = "*/10 * * * *"        # optional
requires = ["python3 >= 3.9"]
env = []                         # host env vars to pass through

[[adapter.config]]
key = "api_token"
name = "API Token"
description = "Shown in the source setup form"
type = "string"
required = true

[data_type]
id = "my-records"
name = "My Record"
icon = "note"

[models.item]
fields.title = "string"
fields.body = "text"
fields.created = "datetime"

[search]
primary_model = "item"
title = "title"
preview = "body"
subtitle = "path"
search_fields = ["title", "body"]
date_field = "created"
```

`[models.*]` generates one facet table per model. `[search]` names the primary
model and which of its fields reach the FTS index. Editing either is a schema
migration and is applied on the next sync.

Models may declare relations:

```toml
[models.item.relations]
belongs_to = ["folder"]      # first entry becomes parent_uuid, rest become edges
self_referential = "reply_to"
many_to_many = ["item"]
```

### 2. The sync script

Config arrives as one JSON object on stdin. Operations go to stdout, one JSON
object per line. Config values are also exported as
`SPACEDRIVE_CONFIG_<KEY_UPPERCASED>`, alongside `SPACEDRIVE_ADAPTER_ID` and
`SPACEDRIVE_ADAPTER_VERSION`.

```python
#!/usr/bin/env python3
import json, sys

def main():
    config = json.load(sys.stdin)

    for item in fetch(config):
        print(json.dumps({
            "upsert": "item",                 # the model name
            "external_id": item["id"],        # stable at the source
            "fields": {
                "title": item["title"],
                "body": item["body"],
                "created": item["created_at"],
            },
        }), flush=True)

    print(json.dumps({"cursor": last_seen_timestamp}), flush=True)

main()
```

The operations:

| Operation | Shape |
|---|---|
| upsert | `{"upsert": model, "external_id": id, "fields": {...}}` |
| delete | `{"delete": model, "external_id": id}` |
| link | `{"link": model, "id": a, "to": model, "to_id": b}` |
| unlink | `{"unlink": model, "id": a, "to": model, "to_id": b}` |
| cursor | `{"cursor": "opaque resume token"}` |
| log | `{"log": "info", "message": "..."}` |

`external_id` is the source's own key and must be stable. `record_overlay`
addresses records by `(type, external_id)`, so an external id that changes
loses everything a person attached to that record.

stderr is drained into tracing at debug level. Print freely.

### 3. Install

Place the directory in `adapters/` and restart the daemon. There is no build
step and no registration list.

## Operations

| Op | Kind |
|---|---|
| `sources.list`, `sources.get` | query |
| `sources.list_items`, `sources.list_records` | query |
| `sources.search` | query |
| `sources.media_listing` | query |
| `sources.create`, `sources.sync`, `sources.delete` | action |

`sources.delete` removes the source's store and its registry row. The store
holds the source's assertions, so this discards them too. Re-indexing is the
operation that keeps them — replace what an ingest produced, leave
`record_overlay` alone — and it does not exist yet.

## Trust tier

Every manifest declares one: `authored` (the user wrote it), `collaborative`
(a shared space), `external` (arrived from elsewhere). It is stored on the
source row and returned with search results.

It has no consumer today. Screening indexed text before an agent can read it is
a position this codebase intends to hold, and trust tier is what that policy
will key on. A stub classifier that marked everything safe was removed on
2026-08-22 along with its verdict columns; they return with a real
implementation.

## Development

```bash
cargo test -p sd-store -p sd-archive
RUST_LOG=sd_store=debug,sd_archive=debug cargo run --bin sd-daemon
```

Inspect a source store directly:

```bash
sqlite3 "<library>/archive/sources/<source_id>/data.db"
.schema
SELECT uuid, type, external_id, title FROM record LIMIT 10;
```

`tests/adapters.rs` builds a store from every bundled manifest and round-trips
a probe record through ingest, listing and search. A manifest that does not
produce a usable store fails there. The store's own behaviour — identity,
facets, edges, overlays, search — is covered by `crates/store/tests/record.rs`.

## Known gaps

- Adapters run through `sh -c` with the daemon's privileges. There is no
  sandbox, and `[adapter.runtime] requires` is not enforced.
- Search is FTS5 per source, fanned out and merged by rank. There is no
  semantic search and no cross-source ranking model.
- Filesystem sources do not yet write a source store; that is P2 of the
  convergence plan.
