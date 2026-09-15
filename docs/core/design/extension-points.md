# Extension Points

> **Status:** Design, inventory of seams
> **Captured:** 2026-08-19
> **Companions:** `docs/core/design/archive.md` (the schema and adapter boundary)

## Purpose

The platform doc decided that Spacedrive supports third-party code in three tiers and that no tier opens before its contract is frozen. It did not enumerate what can be extended. This document does that: every seam in the system where third-party code could plug in, what the seam looks like in the code today, which tier it belongs to, and whether it is ready.

An inventory is the useful artifact here because most of these seams already exist for first-party reasons. The work is rarely inventing an extension point. It is deciding which existing boundary becomes public, and paying for the freeze.

## The principle: data first

Custom software starts with data. Someone who wants to build on Spacedrive usually does not want to write a file manager; they want their own kind of record to exist, be indexed, be searchable, sync across their devices, and be visible to an agent. A filmmaker's asset manager is a schema for shots, reels, and rights, plus a few views over it. A person planning something is a schema and a list.

This orders the whole inventory. **The data model is the first extension point, and the rest exist to present and process what it defines.** An extension that can declare a record type and get indexing, search, sync, and agent access for free is a platform. One that can only add a preview renderer is a plugin API.

## The inventory

| Extension point | The seam today | Tier | State |
| --- | --- | --- | --- |
| **Custom data sources** | `crates/archive` schema TOML plus script adapters | 2 | Real, hardening |
| **Ops and events** | The ops registry, Specta-generated types, `registry.list` | 2 | Real, freeze scheduled |
| **UI contributions** | SpaceUI plus a manifest; windows, routes, menus, settings | 2 | Manifest not built |
| **Focused product surfaces** | App-owned source plus declared views | 2 | In design |
| **Agent skills** | CLI and future MCP operations described for an external agent | 2 | First skill planned |
| **Storage backends** | `VolumeBackend` trait | 2 or 3 | Trait exists, not public |
| **Byte providers** | `ByteProvider` in the mounts byte plane | 3 | Emerging |
| **Network protocols** | Iroh ALPNs, one per protocol | 3 | Informal registry |
| **Preview renderers** | `ContentRenderer.tsx` switch on content kind | 2 | Closed, needs a registry |
| **Tile producers** | `Producer` trait in `crates/bake` | 3 | Cleanest seam in the tree |
| **Facet extractors and rule lenses** | The sink pipeline | 3 | Host surface still moving |

## Data model

The most defensible point, and the one that is nearly finished.

`crates/archive` parses a TOML schema into models, fields, and relations, generates SQLite tables, FTS indexes, and vector embeddings, and ingests records through adapters written in any language. A data type declared this way gets per-source storage, hybrid search, typed edges to files and to other records, and agent access, without touching core.

The digital asset management case is the honest test. A DAM is a detailed schema (shot, reel, rights holder, delivery, review state), a handful of views, and edges from those records to the media files they describe. Every piece of that is expressible against this boundary today except the views. If a filmmaker cannot build a DAM on Spacedrive, the boundary is not yet what it claims to be, and finding that out is worth more than any amount of specification.

What remains: `Adapter::schema()` on the trait so Rust connectors are peers of scripted ones, batched transactional ingest, and the source pool cache. All three are already scheduled in the transplant plan.

**An app ships a source.** Installing an app registers its record types. Isolation is structural rather than promised, because a misbehaving source is one per-source store, which is a folder that can be deleted.

## Presentation

Where a record or a file becomes something a person looks at.

**Preview renderers.** `ContentRenderer.tsx` dispatches on content kind through a hardcoded switch covering image, video, audio, mesh, document, book, spreadsheet, presentation, text, code, and config. It is a closed set, and every kind the product will ever preview is compiled into it. The seam that has to exist is a renderer registry: a contribution declares which kinds or extensions it handles, and the switch becomes a lookup with the built-ins registered first. Streaming is part of the contract rather than an afterthought, since the interesting custom renderers are timeline-shaped and want byte ranges rather than whole files. The existing video path already reads proxies and sidecars this way.

**File kinds.** A custom renderer is not useful if the system cannot name the type it renders. Kind detection is core-side and closed. Opening it means a contribution can declare an extension and a kind, which then flows through the Explorer, search facets, and the renderer registry.

**Views.** The Explorer's views are lenses over the index. A custom view is a UI contribution that reads through the SDK and renders its own surface, which is what makes a DAM's shot board or a planner's board possible without core work.

**UI contributions generally.** Windows, a nested router mount, context menu items scoped by kind, settings pages, menu items, and commands, declared in a manifest. The manifest does not exist yet and is specified in the applications document.

## Pipeline

Per-file work, in-process, on the hot path. This is Tier 3 and lands last, because an ABI frozen against a moving pipeline becomes a compatibility museum.

**Tile producers** are the best-shaped extension point in the tree today. `crates/bake` defines a `Producer` trait with one method that either returns a tile or declines, and `bake()` runs a chain in cost order and takes the first result. Adding a format handler is adding a producer to the chain. The design already demotes FFmpeg from a system dependency to one optional producer, which is the same move a third party would make. If any in-process extension ships first, it is this one.

**Facet extractors and rule lenses** run per file inside indexing, where an out-of-process round trip per file would destroy the throughput the living index depends on. This is the one place where Tier 2 is not an acceptable substitute, and it is also the place where the host surface is still under construction.

**Sidecar kinds** are a closed enum today (thumb, thumbstrip, proxy, embeddings, ocr, transcript, gaussian splat). A pipeline extension that derives something new needs a kind to store it under, so opening this enum is a prerequisite rather than a separate feature.

## Storage and transport

**Storage backends.** `VolumeBackend` is already a trait with read, ranged read, write, directory listing, metadata, existence, and delete. Cloud storage runs through it via OpenDAL. A third-party backend is a new implementation, which makes this one of the cheaper points to open. The caution recorded in the mounts design applies: the backend fallback must be provider-aware, because falling back to local I/O for a non-local volume means reading a literal URI as a path.

**Byte providers** serve mounted content, with local, cloud, peer, and replica implementations. `core/src/service/mounts/` already holds `peer.rs` and `webdav.rs`, so this seam is emerging in code rather than only in design.

**Network protocols.** Each protocol is an Iroh ALPN: file transfer, sync, job activity, byte range. Adding one is adding a string and a handler, which is an informal registry that works because every entrant so far has been first-party. Opening it needs the registry to become explicit, with namespacing and a capability gate, because a protocol handler is the most privileged thing on this list.

## Agent skills

Skills belong to the agent client. Spacedrive supplies structured operations,
stable identifiers, capability requirements, and examples. The first package
is a Spacedrive skill over `sd-cli`; an MCP transport follows after those tool
schemas have been exercised. An extension may ship skill instructions beside
its schema and views, but core does not own an agent runtime or skill lifecycle.

## Permissions

No extension gets ambient authority. Capability grants are specified in the applications document and apply here without modification: named grants, shown before they take effect, listed in the inspector, revocable there.

Three rules this inventory adds:

- **The grant names the seam.** Registering a data type, adding a renderer, and handling a network protocol are different grants with different risk, and a single "extend Spacedrive" permission would be meaningless.
- **In-process extensions carry more risk than out-of-process ones, and the model should say so.** A Tier 3 extension runs inside indexing. A Tier 2 app is a client that can be killed.
- **A declined capability degrades rather than fails.** An extension without the grant it wanted should still install and still do what it can, because the alternative teaches people to grant everything.

## Ordered phases

1. **Finish the source boundary.** `Adapter::schema()`, batched ingest, pool cache. The DAM test as the acceptance criterion.
2. **The manifest and the SDK.** Ops surface enumerated by `registry.list`, typed client, capability grants, UI contribution declarations. The analyzer as the dogfood, per the platform doc.
3. **The renderer registry and open file kinds.** The first presentation seam, and the one people ask for first.
4. **Storage backends.** Cheap, because the trait is already the right shape.
5. **Custom views.** Once the SDK and the manifest can carry them.
6. **Tile producers.** The first in-process tier, chosen because its chain is already a public-shaped contract.
7. **Protocols, facet extractors, and rule lenses.** Last, after the pipeline seam stops moving.

## Open questions

- Whether a schema alone can generate a usable default view, since a data type with no interface is not a feature for the person who declared it.
- How a record type declared by an app behaves when the app is uninstalled. The per-source store makes deletion clean and makes accidental data loss equally clean.
- Whether renderers are also the mount preview path, since a mounted remote file and a local one should preview identically.
- What a schema migration looks like when a third party ships version two of their data type against records a person already has.
- Whether skills and UI contributions from one app can target another app's record types, which is where an ecosystem starts and where the permission model gets hard.
