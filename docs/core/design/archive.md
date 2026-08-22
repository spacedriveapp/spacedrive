# Adapter Sources

> **Status:** Describes `crates/archive` as it stands on 2026-08-22.
> **Direction:** `docs/plans/2026-08-22-source-convergence.md`. This document
> describes one ingest path; the convergence plan describes the store all
> ingest paths are converging on.

## What this is

An adapter source is a registered source whose records come from somewhere
other than a filesystem walk: a Gmail account, an Obsidian vault, a Slack
workspace, browser history. It is not a second data plane. A source is a source
— origin, ingest, store — and an adapter is one kind of ingest.

The prior version of this document described archive as a system sitting
*beside* the VDFS with its own storage world. That framing is retired. Files
and adapter records converge on one store shape; what differs is how rows get
written.

## The store

Every source owns a database with the same shape, whatever wrote it:

- **`record`** — one row per indexed thing. `uuid`, `external_id`, `type`,
  `title`, timestamps, `parent_uuid`, `content_id`. `(type, external_id)` is
  unique, which is what makes re-ingest idempotent.
- **`facet_<model>`** — the type's own columns, keyed by `record_uuid`,
  generated from the model's TOML declaration.
- **`content`** — the identity of bytes a record points at.
- **`edge`** — relationships between records in this source.
- **`search_index`** — an FTS5 virtual table over the fields the search
  contract names.

`record` and its facets is the shape cross-source search and cross-source edges
join on. Two shapes would mean two of everything downstream.

Durable assertions — overlays, curated groupings, cross-source edges — live in
`registry.db` today and key on `(source_id, type, external_id)` so they survive
a source being deleted and re-added. Moving them into each source's own file is
decision 1 of the architecture previs; see the convergence plan for the open
question that decision leaves.

## The adapter protocol

An adapter is a directory containing `adapter.toml`, an icon, and an
executable. The manifest declares three things: how to run it, what it needs
configured, and the shape of what it produces.

```toml
[adapter]
id = "obsidian"
name = "Obsidian Vault"
trust_tier = "authored"

[adapter.runtime]
command = "python3 sync.py"
timeout = 300
schedule = "*/10 * * * *"

[[adapter.config]]
key = "vault_path"
type = "string"
required = true

[data_type]
id = "markdown"
name = "Markdown Note"

[models.note]
fields.title = "string"
fields.body = "text"
fields.modified = "datetime"

[search]
primary_model = "note"
title = "title"
preview = "body"
search_fields = ["title", "body"]
date_field = "modified"
```

`[models.*]` generates the facet table DDL. `[search]` names which model is the
primary one and which of its fields reach the FTS index. Changing either is a
schema migration, applied on the next sync.

At sync time the process receives its resolved config as JSON on stdin and
writes newline-delimited JSON operations to stdout:

```jsonl
{"upsert": "note", "external_id": "vault/notes/q3.md", "fields": {"title": "Q3", "body": "..."}}
{"delete": "note", "external_id": "vault/notes/old.md"}
{"link": "note", "id": "a", "to": "note", "to_id": "b"}
{"cursor": "2026-08-22T10:00:00Z"}
{"log": "info", "message": "412 notes scanned"}
```

`cursor` is the resume point for incremental syncs. stderr is drained into
tracing at debug level, so an adapter can print freely without blocking.

Nothing about this protocol is language-specific. The eleven bundled adapters
are Python because Python is already on macOS; anything that reads stdin and
writes JSONL works.

## Trust tier

Each manifest declares whether its content is `authored` (the user wrote it),
`collaborative` (a shared space), or `external` (arrived from elsewhere). It is
carried on the source row and surfaced with search results.

It has no consumer today. Screening — classifying indexed text before an agent
can read it — is a design position this codebase intends to hold, and trust
tier is the input that policy will key on. What was removed on 2026-08-22 was a
stub classifier that marked everything safe and a set of verdict columns that
read as a guarantee nothing was enforcing. The columns come back with a real
implementation, not before.

## File-backed adapters

When an adapter's subjects are files that already exist on disk, the filesystem
source owns the record and the adapter contributes knowledge about it. It does
not mint a parallel record for the same bytes. See
`docs/core/design/file-backed-sources.md` — the Apple Photos adapter predates
that rule and is the reason it is written down.

## Where it is wired

`Library` holds a `SourceManager` (`core/src/data/manager.rs`) wrapping the
engine. Operations live in `core/src/ops/sources/`:

| Op | Kind |
|---|---|
| `sources.list`, `sources.get` | query |
| `sources.list_items`, `sources.list_records` | query |
| `sources.search` | query |
| `sources.media_listing` | query |
| `sources.create`, `sources.sync`, `sources.delete` | action |

The crate itself holds no job system and no operation layer; long-running sync
is a core job that calls into it.
