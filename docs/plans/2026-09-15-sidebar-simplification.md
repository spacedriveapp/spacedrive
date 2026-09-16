# Sidebar Simplification

> **Status:** In execution
> **Captured:** 2026-09-15
> **Product boundary:** `docs/core/product-direction.mdx`

The first slice landed in the worktree on 2026-09-15. S0 through S2 are built.
S3 has the new names, routes, Activity surface, and computed current-device
folders under Places. Those folders use the Spacedrive Folder and Home assets.
Explorer destinations also load route-scoped view preferences, so a Media
default cannot leak from Photos or Screenshots into normal folders. Home and
Storage still need a live composition pass. S4 remains open.

## Outcome

The sidebar is a list of destinations a person returns to. It is not an
inventory of every device and volume, a setup checklist, or a second settings
window.

The default surface has one library scope, flat daily destinations, one Places
section for known folders, source roots, and pinned paths, and a small footer
for activity and settings. Storage topology and setup live on the Storage
surface. Extensions may contribute one removable home for a repeated activity.

## Product contract

- A new user learns Home, Places, Storage, and Protection before source,
  volume, or space terminology.
- Home, Recents, Favorites, Screenshots, Storage, and Protection each appear
  once.
- Places contains known system folders, browsable source roots, pinned
  source-relative paths, and relevant extension contributions.
- Detected devices and volumes never appear just because they exist.
- Empty groups and unavailable setup placeholders do not render.
- Jobs and sync share one Activity entry.
- One canonical row owns active, disabled, badge, keyboard, and context-menu
  behavior.
- Spaces remain compatible, but a single default space is not presented as a
  second scope selector.

## Vocabulary boundary

Places is a client presentation category, not a third storage abstraction.

| Term | Meaning | Where it belongs |
| --- | --- | --- |
| Source | A data origin with an ingest, store, identity, state, and lifecycle | Storage, diagnostics, CLI, and API |
| Location | An internal pin over a source-relative path and a stable target for explicit policy | Core persistence and navigation intent |
| Place | Somewhere a person can navigate to | Sidebar and other client navigation |

The Places section may project current-device folders such as Desktop and
Downloads, browsable source roots, user pins, offline indexed drives, and
extension contributions. Those rows keep the identity and behavior of the
thing they represent. There is no `Place` database model, `places.*` operation
namespace, or second indexing lifecycle.

Persist only underlying user intent, such as a pin, order, visibility, or
extension contribution. Clients compute Places from that intent and the
currently available sources and system folders.

The product rule is: **Places are where you go. Sources are how Spacedrive
knows about the data.**

## Execution

### S0. Canonical surface

Add typed destination, place, and utility nodes. Render them through one row
and one section component. Remove custom buttons and fake `SpaceItem` values
from the active sidebar.

Acceptance: every visible navigation row uses the canonical row; the active
sidebar contains no `any` casts.

### S1. Default information architecture

Render one library selector, the flat daily destinations, Places, and the
Activity/Settings footer. Move imports and setup recommendations out of the
sidebar. Hide workspace switching until more than one space exists.

Acceptance: there is no duplicate Sources destination, no empty group, and no
automatic Devices, Volumes, Tags, or import section.

### S2. Seed convergence

Rename the deterministic default space without changing its UUID. Seed the
daily destinations and retire the old Analyzer, Sources, Devices, Volumes,
Tags, and duplicate Sources defaults by their deterministic UUIDs. Preserve
all non-default spaces, groups, and items.

Acceptance: opening an existing library converges only seeded defaults and
keeps user-created navigation intact.

### S3. Destination homes

Rename Overview to Home, Sources to Storage, and Redundancy to Protection.
Storage owns sources, devices, drives, capacity, and analysis. Home owns setup
recommendations. A Duplicates destination lands only with a working workflow.

Acceptance: navigation labels match their page titles and every setup action
has one contextual home.

### S4. Shared contribution model

Expose stable navigation contributions to React and GPUI. Persist only user
intent: pins, order, visibility, custom sections, and optional workspaces.

Acceptance: an extension can add or remove one home without client-specific
sidebar JSX, and no shared contribution introduces a persisted Place entity.

## Verification states

Exercise a new empty library, the production-sized NAS library, an offline
source, multiple libraries, multiple spaces, a custom group, a running job,
and an installed adapter. Type checking and focused backend tests gate the
first slice.
