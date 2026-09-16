# Search Refinement

> Status: planned, unowned
> Audited: 2026-09-15 against `fe4a100b3` and the current worktree
> Register: `PROJECT_STATUS.md`
> Companion history: the matching and duplicate-partition fixes in
> `fe4a100b3` made name search correct. This document owns everything above
> matching: scopes, filters, sort, pagination, and the refinement bar.

## Outcome

Search in the explorer becomes trustworthy end to end. Every control the bar
shows does what it says, every parameter the client sends is either honored by
the daemon or absent from the wire, and a search survives the navigation a
person does while reading its results.

Completion means: the three scope buttons select folder, source, and library
scope and stay selected while typing; the Filters button opens a panel whose
filters all take effect; the result list reports its true total; sort and
pagination requests change the response; and entering a folder from a result
leaves the search reachable through the back button.

## Current state

Matching is correct as of `fe4a100b3`. Everything above it is partly
theatrical:

- The Filters button is a styled element with no click handler. No filter UI
  exists. The filters object sent with every search is a hardcoded all-null
  literal in `useExplorerFiles.ts`.
- The "Location" scope has no wire representation and falls through to
  Library, so two of the three scope buttons do the same thing. The chosen
  scope is also reset to "This Folder" by the next keystroke, because the
  input handler re-enters search mode with the default scope.
- The daemon honors four filters (`file_types`, `size_range`, `date_range` on
  modified/created, `content_types`) and correctly advertises them in
  `available_filters`. It silently ignores tags, locations, hidden, archived,
  and all five redundancy filters. `include_hidden` is notable: the index
  computes `is_hidden` per entry and the filter never reads it.
- `sort` and `pagination` are validated, logged, and never applied. Results
  come back in score order, truncated at 200, with a synthetic pagination
  block whose `total_found` can never exceed 200. `SearchMode` is never read.
  `sorting.rs` and the `FacetBuilder` half of `facets.rs` are dead code.
- The redundancy routes (`at-risk.tsx`, `compare.tsx`) drive `search.files`
  with `at_risk`/`on_volumes` filters that the ephemeral path ignores, so
  those views very likely list unfiltered files today. Needs a live proof
  before Phase 5 decides their mechanism.
- The 300ms "debounce" returns a cleanup closure from an onChange handler
  that nothing invokes, so each keystroke dispatches its own search. Search
  mode lives in the global UI reducer rather than per tab, every navigation
  force-exits it, and no `NavigationTarget` variant exists for it, so a
  search cannot be returned to or linked to. Cmd+F is defined in the keybind
  registry and consumed by nothing.
- Unused assets: `total_found` and `SearchFacets` arrive on every response
  and are discarded; `packages/ts-client/src/hooks/useSearchFiles.ts` already
  maps every filter field and is used by mobile but not by the desktop
  explorer; `context.tsx` carries a UI `SearchFilters` interface with state
  and a setter that nothing reads or writes, in a shape that does not match
  the wire type.

## Decisions

These are settled here so the phases stay mechanical.

1. **Scopes are folder, source, library.** "Location" leaves the UI; a
   location is a pin and pins do not define search domains. Source scope is a
   client concern: resolve the containing source root through `paths.context`
   (which already reports it, alias-normalized) and send `SearchScope::Path`
   with that root. The wire enum stays `Library | Path`. On a replica the
   same rule applies with the owning device's slug, which the remote search
   branch already serves.
2. **The v1 filter set is what the daemon can answer from the index:** kind
   (`content_types`), extension (`file_types`), size range, date range on
   modified/created, and a hidden-files toggle. Tags follow in their own
   phase through a path-set join against the library database. Redundancy
   filters stay out of the bar; they belong to the redundancy views and their
   correct backend, and cross-source truth waits on `catalog.db`.
3. **Sort and pagination become honest rather than deleted.** The input shape
   is right; the implementation is missing. Sort applies after filtering,
   pagination applies after sorting, `total_found` reports the full match
   count, and the response page is `pagination.limit` capped at the validated
   maximum of 1000. Relevance stays the default sort. Equal scores tiebreak
   on name so results stop reshuffling between keystrokes. `SearchMode` stays
   accepted and logged; ranking depth is not a v1 concern.
4. **A search is a navigation target.** Mode and filters move into per-tab
   state, entering a folder from a result pushes history instead of
   destroying the search, back returns to the results, and the target
   round-trips through the URL so a search is linkable and restorable.
5. **The desktop explorer adopts `useSearchFiles`.** One mapper from UI
   filter state to `SearchFilters` lives in ts-client, shared with mobile.
   The hand-rolled input block in `useExplorerFiles.ts`, the dead UI
   `SearchFilters` scaffolding in `context.tsx`, and the duplicated
   `EMPTY_FILTERS` literals in the redundancy routes all fold into it.

## Preserve these boundaries

- The ephemeral volume index is the search substrate for this plan's phases,
  and R6 of `2026-09-15-source-runtime-reliability.md` supersedes that as the
  final architecture: one primary backend per source per request, arena when
  suitable, direct store reads otherwise. Phase 1 therefore builds its
  filter, sort, dedup, and pagination stage as a module the arena path calls,
  not as arena internals, so R6 routes both backends through the same
  pipeline. No new durable state, no new columns on legacy tables, no `entry`
  FTS revival; a file candidate index belongs with the record store and is
  R6's decision to benchmark.
- `sources.search` remains the archive-record search. It is a model for a
  later content stage, never a dependency of this work.
- Filters must fail closed on the advertised set: a filter the daemon cannot
  answer is absent from `available_filters` and absent from the bar. No
  filter silently passes everything again.
- Remote replicas go through the same `collect_results` filter path as local
  partitions. Any new filter must hold for both or be scoped out explicitly.

## Phases

### Phase 1: backend honesty

All in `core/src/ops/search/`.

- Apply `sort` after filtering: Relevance (default), Name, Size, ModifiedAt,
  CreatedAt, with direction. IndexedAt sorts by nothing the index holds, so
  it maps to score order until an indexed-at signal exists; recents keeps
  working through its dedicated path.
- Apply `pagination` after sorting. `total_found` becomes the pre-pagination
  count. The library-scope merge collects per-partition candidates before a
  single global sort, truncating partitions only above the requested window.
  The filter/sort/paginate stage lives in its own module with a
  backend-neutral candidate type, because R6 later feeds it store-read
  candidates through the same pipeline.
- Case-fold `file_types` comparison and document that the daemon compares
  lowercase extensions.
- Wire `include_hidden` to the `is_hidden` the index already computes.
  Default excludes hidden files; the toggle includes them.
- A missing timestamp fails a date filter instead of passing it.
- Stable tiebreak in `rank` (score desc, then name asc).
- Delete dead code this phase makes unreachable for good: `sorting.rs`, the
  unused `FacetBuilder`/`SuggestionGenerator` duplicates, and the legacy
  entity imports in `query.rs`.
- Proof: focused Rust tests over one seeded index covering each filter, each
  sort field and direction, pagination windows, hidden defaults, and a
  `total_found` larger than the page.

### Phase 2: frontend state machine

All in `packages/interface`.

- One real debounce on the search input. Typing updates the query of the
  existing search mode without touching scope or filters. Falling under two
  characters clears results deterministically.
- Search mode and filters move into per-tab explorer state beside view
  preferences. A `search` `NavigationTarget` variant carries query, scope,
  and filters; navigation into a result pushes history; back restores the
  search; the URL round-trips it.
- Cmd+F focuses the search input through the already-defined keybind.
- The two-character gate lives in one place.
- Proof: type a word letter by letter with the network tab open and observe
  one request per settled query; pick Library scope, keep typing, scope
  holds; open a result folder, press back, results return; switch tabs, each
  tab keeps its own search.

### Phase 3: the refinement bar

- Scope control becomes folder / source / library. Source resolves through
  `paths.context` and disables itself when the current path has no containing
  source. Folder scope inside a replica keeps routing with the owning
  device's slug.
- The Filters button opens a panel with the v1 set: kind, extension, size
  range, date range (modified/created), hidden toggle. Active filters render
  as removable chips in the bar. The panel only offers what the response's
  `available_filters` advertises.
- The bar shows the true result count from `total_found`, and facet counts
  from `SearchFacets` annotate the kind and extension options.
- The desktop path consolidates onto `useSearchFiles` with one UI-to-wire
  filter mapper; the dead `SearchFilters` scaffolding in `context.tsx` and
  the hand-rolled input block are removed.
- Proof: every control changes the result set live against the daemon;
  a filter chip survives typing, scope changes, and tab switches; the
  desktop production build passes.

### Phase 4: tag filtering

- The daemon resolves `TagFilter.include`/`exclude` to a path set once per
  query through the existing tag scopes join (`files_by_tag` primitives) and
  intersects during collection. Applies to local partitions; replicas are
  excluded from tag filtering until assertions replicate, and tag filtering
  therefore removes replica hits rather than passing them unfiltered.
- `available_filters` advertises Tags once this lands; the panel gains a tag
  picker driven by `tags.search`.
- Proof: tag two files in different sources, search with include and exclude
  filters, and get exactly the right hits; a replica-only search with a tag
  filter returns empty rather than everything.

### Phase 5: redundancy views tell the truth

- Live-verify the suspicion that `at-risk.tsx` and `compare.tsx` currently
  receive unfiltered results.
- Decide their mechanism against what exists: either implement the
  redundancy filters in the ephemeral path over the store's content
  projections for sources the library holds, or move those views onto
  `files.duplicates` and its store-backed queries. Cross-source completeness
  remains bounded by the absent `catalog.db` and the views must say so
  rather than imply a global answer.
- Proof: the at-risk view's counts reconcile with `files.duplicates` numbers
  for the same store set.

## Out of scope

- Content and semantic search, highlights, and `matched_content`. The fields
  stay in the output for the stage that fills them.
- A persistent file FTS index. That decision belongs with the record table
  work after the entries drop.
- `catalog.db` and complete cross-source redundancy answers.
