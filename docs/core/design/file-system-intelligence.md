# File System Intelligence

## Purpose

Define `File System Intelligence` as a first-class Spacedrive capability.

File System Intelligence is the intelligence layer that sits on top of the native filesystem and the VDFS. It turns files, directories, clouds, and devices into a machine-readable, agent-readable, human-readable system with derived knowledge, layered context, and universal policy.

This is one of the clearest explanations for why Spacedrive exists beyond being a file manager.

## Definition

`File System Intelligence` is Spacedrive's cross-platform intelligence layer for filesystems.

It includes:

1. derived knowledge about individual files
2. contextual knowledge attached to directories and subtrees
3. universal permissions and policies for agents and automation

Native operating systems expose paths, files, folders, metadata, and OS permissions.

Spacedrive adds:

- meaning
- structure-aware summaries
- derivative data
- context that evolves over time
- cross-device continuity
- agent-readable policy

## Why This Exists

Agents walking a filesystem through shell commands are effectively walking blind.

They can list directories, open files, and infer structure, but they do not naturally understand:

- why a folder exists
- how a user organizes work
- what a directory is for
- what files are important inside a subtree
- what workflows apply there
- what the agent is allowed to do there

File System Intelligence gives the filesystem a context layer that can be surfaced as the agent navigates.

The goal is to make a filesystem legible to AI without relying on fragile session memory or a monolithic root instruction file.

## Relationship to the VDFS

The VDFS remains the storage and identity substrate.

File System Intelligence is a layer on top of it.

The VDFS gives us:

- content identity
- path abstraction
- sidecars and derivatives
- cross-device addressing
- jobs
- sync
- permissions infrastructure

File System Intelligence uses that substrate to attach context and policy to files and subtrees in a way that is portable across devices and storage backends.

## Relationship to Spacebot

Spacedrive owns File System Intelligence.

Spacebot is the first major producer and consumer of it.

This is important because the intelligence layer should not be framed as only a Spacebot feature. It is a core Spacedrive capability that any agent or automation system can use.

Spacebot can:

- write user-informed context into the filesystem intelligence layer
- read that context while navigating files and directories
- update summaries and policies over time

## Product Framing

This is the short product framing:

- Finder and Explorer show you where files are.
- Spacedrive understands what they are, why they exist, how they relate, and what agents are allowed to do with them.

This is the platform framing:

- Spacedrive adds File System Intelligence: derived knowledge, contextual understanding, and universal permissions across every device and cloud.

## Core Pillars

File System Intelligence has three pillars.

### 1. File Intelligence

Per-file derived knowledge.

Examples:

- extracted metadata
- OCR text
- transcripts and subtitles
- thumbnails and previews
- classifications
- sidecars and derivative artifacts
- extracted structure from documents or media

This intelligence is usually deterministic or pipeline-driven.

### 2. Directory Intelligence

Contextual knowledge attached to directories and subtrees.

Examples:

- "This is where I keep active projects"
- "Archive contains dormant repositories"
- "This folder contains scanned personal records"
- "This area is client work, do not modify without approval"
- summaries of what a directory contains and how it is used

This intelligence can come from both users and agents and should be inherited through the subtree where appropriate.

### 3. Access Intelligence

Universal permissions and policy that sit above OS-native permissions.

Examples:

- which folders an agent may read
- which folders an agent may write to
- whether deletion is allowed
- whether a subtree is sensitive
- whether a cloud source is accessible to a given automation

This allows a user to grant access once through Spacedrive and have that policy apply consistently across devices, clouds, and operating systems.

## What It Is Not

File System Intelligence is not:

- a replacement for the native filesystem
- a monolithic prompt file like a giant `AGENTS.md`
- only vector embeddings
- only tags
- only sidecars
- only agent memory

It is a structured context and policy layer that can be queried, updated, inherited, and observed over time.

## Design Principles

### Context should be hierarchical

Context must attach at multiple levels of the filesystem and follow the tree.

If a user explains what `~/Projects` is for, that context should be available when an agent explores `~/Projects/foo/bar` unless something more specific overrides or narrows it.

### Context should be scoped

Only the relevant context for the current subtree should be surfaced.

This avoids the context pollution problem of large root-level instruction files.

### Context should be observable

The system should preserve who said what, when it changed, and how the understanding of a subtree evolved over time.

### Context should be atomic

The source of truth should not be a single mutable paragraph.

Facts, policies, and notes should be stored as atomic records. Summaries should be generated views over those records.

### Context should be portable

The same model should work for:

- local filesystems
- removable volumes
- NAS storage
- cloud providers
- future repository-backed archival sources where relevant

## Recommended Data Model

Do not model File System Intelligence as tags alone.

Tags are useful, but they are too narrow to carry the full meaning of filesystem context.

Instead, use a richer context-layer model.

### Context Node

A `ContextNode` is the core primitive.

It attaches to a file, directory, subtree, or virtual filesystem object and stores one piece of meaning, policy, or generated understanding.

Suggested fields:

```text
id
library_id
target_kind           # file | directory | subtree | volume | cloud_location
target_id             # VDFS identity or location-scoped identifier
scope                 # exact | inherited
node_kind             # fact | summary | policy | note | tag
title
content
structured_payload
source_kind           # user | agent | job | system
source_id
confidence
visibility            # user_only | agent_visible | private | synced
created_at
updated_at
supersedes_id
archived_at
```

### Why This Shape

- atomic facts can accumulate over time
- generated summaries can be refreshed without destroying history
- policies can be stored separately from descriptive context
- tags can remain lightweight labels rather than carrying every semantic burden

## Facts vs Summaries

This distinction is critical.

### Atomic Facts

Examples:

- "User keeps active repositories in this directory"
- "Archive subfolder contains inactive projects"
- "This subtree contains financial documents"
- "Agent may edit files here but may not delete them"

Facts are durable, attributable, and versionable.

### Generated Summaries

Examples:

- "This directory mostly contains Rust and TypeScript repositories updated recently"
- "This subtree appears to be an archive of completed client projects"

Summaries are synthesized views over facts, file structure, and activity.

The source of truth is the atomic layer, not the summary text.

## Tags

Tags still matter.

They can be used as one expression of intelligence, especially when the system needs lightweight labels with rich metadata.

But they should not be the only model.

Recommended role for tags:

- lightweight labels on files or directories
- optional metadata carriers
- one output of the broader context system

Possible future direction:

- allow tags to carry rich text and version history
- allow tags to be generated from or backed by context nodes

## Permissions and Policy

Universal permissions are a major part of File System Intelligence.

These permissions should live above the OS layer and be enforced when agents access files through Spacedrive.

Examples:

- read-only subtree
- writable subtree
- safe workspace subtree
- no-delete policy
- user-confirmation-required policy
- hidden subtree for sensitive data

This gives the user one consistent interface for granting agent access across:

- macOS
- Windows
- Linux
- cloud providers
- remote devices

## Agent Experience

When an agent accesses a path through Spacedrive, it should not only receive the raw directory listing.

It should receive:

- the listing itself
- relevant inherited context
- relevant local context
- active permissions and policy
- important summaries of subtree contents
- optionally recent changes or historical notes

This turns navigation from blind traversal into informed traversal.

## Query Surface

At the VDFS and API layer, the system should support queries such as:

- get context for this path
- get inherited context for this subtree
- list context nodes attached here
- generate summary for this subtree
- add fact to this path
- add policy to this subtree
- resolve effective policy for this path
- show context history for this directory

The system should be able to answer both human-facing and agent-facing forms of the same question.

## Sources of Intelligence

There are multiple sources of intelligence.

### Deterministic Jobs

Best for:

- metadata extraction
- media derivatives
- content statistics
- directory composition summaries
- language and file type distribution

### Agent-Written Context

Best for:

- user workflow explanations
- organizational semantics
- safe workspace semantics
- intent captured during normal conversation

### User-Written Context

Best for:

- explicit corrections
- durable preferences
- policy decisions
- sensitive or authoritative context

## Jobs vs Agent Interaction

The first implementation should not rely only on a background job that tries to infer the meaning of the whole filesystem from structure alone.

That approach risks weak summaries and invented semantics.

Instead:

- jobs produce deterministic observations and refresh generated summaries
- agents and users add meaning over time

This lets the intelligence layer evolve incrementally and honestly.

## Example

Given a home directory with:

```text
~/Projects
~/Projects/Archive
~/Documents
```

The user tells Spacebot:

- "I keep active repositories in Projects"
- "Archive contains repos I'm not actively working on"

The system stores these as atomic context nodes.

Later, a summary job produces:

- `~/Projects`: "Primary software workspace containing active repositories, mostly Rust and TypeScript"
- `~/Projects/Archive`: "Inactive or historical repositories, lower write priority"

Then when an agent enters `~/Projects/foo`, it inherits:

- that it is inside the active projects subtree
- that agent write access may be allowed there
- that archive semantics do not apply yet

This is the intended user and agent experience.

## Storage Strategy

The exact persistence model is open, but the design should support:

- attachment to VDFS identities and locations
- revision history
- sync across devices when appropriate
- efficient subtree lookup
- policy inheritance resolution

Possible implementation shapes:

1. dedicated context tables in the library database
2. sidecar-style storage indexed into the library
3. tag-backed records with richer metadata and versioning

Recommended direction:

- use dedicated context records as the real model
- integrate tags as one expression layer, not the underlying substrate

## Observability and History

This system should preserve how understanding changes over time.

That means:

- revision history for facts and policies
- superseded summaries rather than silent overwrite
- attribution to user, job, or agent
- optional inspection of context evolution

This is important both for trust and for future agent behavior.

## Search and Retrieval

Vector embeddings may help in some cases, but they are not the primary abstraction for File System Intelligence.

The first retrieval model should be structure-aware and direct.

Examples:

- retrieve context by exact path
- retrieve inherited context by walking ancestors
- retrieve effective policy by path
- retrieve summaries for the current subtree

Embeddings can be added later for semantic recall over large bodies of context, but they should not replace the explicit hierarchical model.

## MVP Recommendation

Start with four primitives.

### 1. Folder Context

Attach rich context to a directory or subtree.

### 2. Atomic Facts

Store user or agent assertions as discrete records.

### 3. Agent Policy

Store subtree-level read/write/modify rules.

### 4. Generated Summary

Generate refreshable summaries from file structure and facts.

This is enough to demonstrate the full value of File System Intelligence without solving every future problem first.

## Integration Path

### Phase 1: Product Language and UI Surface

- adopt `File System Intelligence` as the product term
- expose a basic UI for enabling it per location or subtree
- allow users to add and inspect context

### Phase 2: Context Data Model

- add context node storage
- add effective-context queries
- add policy resolution

### Phase 3: Agent Integration

- Spacebot reads context while navigating via Spacedrive
- Spacebot can write facts and notes with attribution

### Phase 4: Summary Jobs

- generate structure-aware summaries
- refresh them on indexing or change events where appropriate

### Phase 5: Cross-Device Policy and Sync

- sync context and policy across devices at the library level
- apply universal permissions through the VDFS

## Open Questions

1. Should the first storage implementation use dedicated context records or evolve the existing tag model first?
2. How should effective-context inheritance be surfaced in the UI so it is understandable?
3. Which parts of the context layer should sync automatically and which should stay local?
4. How should user-authored policy interact with existing OS-level permission failures?
5. How much agent-written context should require confirmation before becoming durable?

## Recommendation

Adopt `File System Intelligence` as the name for Spacedrive's filesystem context and policy layer.

Implement it as:

- atomic context records
- generated summaries built over those records
- subtree-aware policy and permission resolution
- agent-readable context surfaced during navigation

This gives Spacedrive a clear answer to a fundamental product question:

- why should an agent use Spacedrive instead of raw shell access?

Because Spacedrive does not just expose files. It exposes file systems with intelligence.
