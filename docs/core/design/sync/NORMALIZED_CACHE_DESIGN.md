# Normalized Resource Cache Design

**Status**: RFC / Design Document
**Author**: AI Assistant with James Pine
**Date**: 2025-01-07
**Version**: 1.0
**Related**: INFRA_LAYER_SEPARATION.md

## Executive Summary

This document proposes a **normalized client-side cache** with **event-driven atomic updates** for Spacedrive. Instead of invalidating entire query results when a single resource changes, we:

1. **Normalize resources** by identity (UUID) in a client-side entity store
2. **Map queries to resources** they contain (query → [resource IDs])
3. **Listen to events** and perform atomic updates to cached resources
4. **Automatically update UI** when resources change

This enables:
- **Efficient search** - 1000 files returned, 1 file updated → update 1 entity, not re-fetch 1000
- **Real-time UI** - File renamed? Update visible immediately across all views
- **Bandwidth savings** - Only send deltas, not full result sets
- **Optimistic updates** - Update cache immediately, sync in background

## Core Concept: Resource Normalization

### The Problem

**Current approach** (query-based caching):

```swift
// Query returns full result
let searchResults = try await client.query("search:files.v1", input: searchInput)
// Cache: { "search:xyz": [File1, File2, File3, ...] }

// File2 gets renamed via event
event: .EntryModified { entry_id: file2_uuid }

// Problem: Have to invalidate entire search cache and re-fetch!
cache.invalidate("search:xyz")
let newResults = try await client.query("search:files.v1", input: searchInput) // 
```

**Normalized approach** (resource-based caching):

```swift
// Query returns full result
let searchResults = try await client.query("search:files.v1", input: searchInput)

// Cache structure:
// entities: {
//   "File:uuid1": File1,
//   "File:uuid2": File2,
//   "File:uuid3": File3
// }
// queries: {
//   "search:xyz": ["File:uuid1", "File:uuid2", "File:uuid3"]
// }

// File2 gets renamed via event
event: .EntryModified { entry_id: file2_uuid, updated_data: {...} }

// Atomic update: Update single entity, UI updates automatically!
cache.update(entity: "File:file2_uuid", delta: {...}) // ✅
```

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│ Client Application (Swift UI, React)                        │
│ ────────────────────────────────────────────────────────────│
│                       CLIENT-SIDE ONLY                       │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ UI Components                                         │  │
│  │  • Observe normalized cache via SwiftUI/React hooks  │  │
│  │  • Automatically re-render on cache updates          │  │
│  └───────────────────────┬──────────────────────────────┘  │
│                          │                                   │
│  ┌───────────────────────▼──────────────────────────────┐  │
│  │ Normalized Resource Cache (CLIENT ONLY)             │  │
│  │                                                       │  │
│  │  Entity Store:                                       │  │
│  │  ┌───────────────────────────────────────────────┐  │  │
│  │  │ "File:uuid1" → File { id, name, tags, ... }  │  │  │
│  │  │ "File:uuid2" → File { id, name, tags, ... }  │  │  │
│  │  │ "Tag:tag1"   → Tag { id, name, color, ... }  │  │  │
│  │  │ "Location:loc1" → Location { id, ... }       │  │  │
│  │  └───────────────────────────────────────────────┘  │  │
│  │                                                       │  │
│  │  Query Index:                                        │  │
│  │  ┌───────────────────────────────────────────────┐  │  │
│  │  │ "search:abc" → ["File:uuid1", "File:uuid2"]  │  │  │
│  │  │ "directory:/photos" → ["File:uuid3", ...]    │  │  │
│  │  │ "tags:list" → ["Tag:tag1", "Tag:tag2"]       │  │  │
│  │  └───────────────────────────────────────────────┘  │  │
│  └──────────────────────┬────────┬──────────────────────┘  │
│                         │        │                          │
│  ┌──────────────────────▼────┐  │                          │
│  │ Query Client             │  │                          │
│  │  • Execute queries        │  │                          │
│  │  • Normalize responses    │  │                          │
│  └──────────────────────┬────┘  │                          │
│                         │        │                          │
│  ┌──────────────────────▼────────▼──────────────────────┐  │
│  │ Event Stream Handler                                  │  │
│  │  • Subscribe to core events                           │  │
│  │  • Map events → cache updates                         │  │
│  │  • Apply atomic updates to entity store               │  │
│  └──────────────────────┬──────────────────────────────┘  │
│                         │                                   │
└─────────────────────────┼───────────────────────────────────┘
                          │ Unix Socket / JSON-RPC
                          │ (Events stream down)
                          │
┌─────────────────────────▼───────────────────────────────────┐
│ Spacedrive Core (Rust) - NO CACHE LAYER                     │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ Database (Source of Truth)                            │  │
│  │  • SeaORM entities                                    │  │
│  │  • Single source of truth                             │  │
│  │  • Already optimized with indexes                     │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ Event Bus (Broadcast Only)                            │  │
│  │  • FileUpdated { file: File {...} }                   │  │
│  │  • TagApplied { entry_ids, tag_id }                   │  │
│  │  • LocationUpdated { location: Location {...} }       │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ QueryManager (Stateless)                              │  │
│  │  • Returns data from database                         │  │
│  │  • Includes cache metadata for client                 │  │
│  │  • No caching layer - clients handle that            │  │
│  └──────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

## Layer 1: Rust Core Infrastructure

### 1.1 Identifiable Trait for Domain Models

```rust
// core/src/domain/identifiable.rs

use serde::{Deserialize, Serialize};
use specta::Type;
use std::hash::Hash;
use uuid::Uuid;

/// Marker trait for domain models that can be cached by identity
pub trait Identifiable: Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync + 'static {
    /// The type of ID used (usually Uuid, sometimes i32)
    type Id: Clone + Hash + Eq + Serialize + for<'de> Deserialize<'de> + std::fmt::Display;

    /// Get the primary key for this resource
    fn resource_id(&self) -> Self::Id;

    /// Get the resource type name for cache keys
    /// Returns something like "File", "Tag", "Location"
    fn resource_type() -> &'static str
    where
        Self: Sized;

    /// Get the full cache key: "ResourceType:id"
    fn cache_key(&self) -> String {
        format!("{}:{}", Self::resource_type(), self.resource_id())
    }

    /// Get cache key from just the ID
    fn cache_key_from_id(id: &Self::Id) -> String
    where
        Self: Sized,
    {
        format!("{}:{}", Self::resource_type(), id)
    }

    /// Extract relationships to other resources
    /// Returns map of: relationship_name → [resource_cache_keys]
    fn extract_relationships(&self) -> ResourceRelationships {
        ResourceRelationships::default()
    }
}

/// Relationships this resource has to other cached resources
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct ResourceRelationships {
    /// One-to-one relationships (e.g., File → Location)
    pub singular: HashMap<String, String>,

    /// One-to-many relationships (e.g., File → [Tags])
    pub plural: HashMap<String, Vec<String>>,
}

impl ResourceRelationships {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_singular(&mut self, name: impl Into<String>, cache_key: impl Into<String>) {
        self.singular.insert(name.into(), cache_key.into());
    }

    pub fn add_plural(&mut self, name: impl Into<String>, cache_keys: Vec<String>) {
        self.plural.insert(name.into(), cache_keys);
    }
}
```

### 1.2 Implement Identifiable for Domain Models

```rust
// core/src/domain/file.rs

use super::identifiable::{Identifiable, ResourceRelationships};

impl Identifiable for File {
    type Id = Uuid;

    fn resource_id(&self) -> Self::Id {
        self.id
    }

    fn resource_type() -> &'static str {
        "File"
    }

    fn extract_relationships(&self) -> ResourceRelationships {
        let mut rels = ResourceRelationships::new();

        // Tags relationship
        if !self.tags.is_empty() {
            let tag_keys: Vec<String> = self
                .tags
                .iter()
                .map(|t| Tag::cache_key_from_id(&t.id))
                .collect();
            rels.add_plural("tags", tag_keys);
        }

        // Content identity relationship
        if let Some(content) = &self.content_identity {
            rels.add_singular("content_identity", ContentIdentity::cache_key_from_id(&content.uuid));
        }

        // Location relationship (from sd_path)
        // Note: This requires parsing the location from sd_path context
        // For now, we'll handle this in query-specific logic

        rels
    }
}

impl Identifiable for Tag {
    type Id = Uuid;

    fn resource_id(&self) -> Self::Id {
        self.id
    }

    fn resource_type() -> &'static str {
        "Tag"
    }
}

impl Identifiable for Location {
    type Id = Uuid;

    fn resource_id(&self) -> Self::Id {
        self.id
    }

    fn resource_type() -> &'static str {
        "Location"
    }
}

// Note: Entry is a low-level database entity. For client-side caching,
// we use higher-level File domain objects instead. Entry → File conversion
// happens on the Rust side before sending to clients.

impl Identifiable for crate::infra::job::types::JobInfo {
    type Id = Uuid;

    fn resource_id(&self) -> Self::Id {
        self.id
    }

    fn resource_type() -> &'static str {
        "Job"
    }

    fn extract_relationships(&self) -> ResourceRelationships {
        let mut rels = ResourceRelationships::new();

        // Parent job relationship
        if let Some(parent_id) = self.parent_job_id {
            rels.add_singular("parent_job", Self::cache_key_from_id(&parent_id));
        }

        rels
    }
}

impl Identifiable for crate::library::Library {
    type Id = Uuid;

    fn resource_id(&self) -> Self::Id {
        self.id()
    }

    fn resource_type() -> &'static str {
        "Library"
    }
}

// Similarly for Volume, Device, etc.
```

### 1.3 Cache Metadata in Query Results

```rust
// core/src/infra/query/cache_metadata.rs

use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;

/// Metadata about what resources are included in a query result
/// This enables the client to normalize and cache properly
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CacheMetadata {
    /// Map of resource type → list of IDs in this response
    /// e.g., { "File": ["uuid1", "uuid2"], "Tag": ["tag1", "tag2"] }
    pub resources: HashMap<String, Vec<String>>,

    /// Whether this query result should be cached
    pub cacheable: bool,

    /// Cache duration (in seconds, None = indefinite)
    pub cache_duration: Option<u64>,

    /// Cache invalidation strategy
    pub invalidation: InvalidationStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum InvalidationStrategy {
    /// Invalidate when any listed resource types change
    OnResourceChange { resource_types: Vec<String> },

    /// Invalidate when specific events occur
    OnEvents { event_types: Vec<String> },

    /// Manual invalidation only
    Manual,

    /// Never invalidate (static data)
    Never,
}

impl CacheMetadata {
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
            cacheable: true,
            cache_duration: None,
            invalidation: InvalidationStrategy::OnResourceChange {
                resource_types: Vec::new(),
            },
        }
    }

    /// Add a batch of identifiable resources
    pub fn add_resources<T: Identifiable>(&mut self, resources: &[T]) {
        let resource_type = T::resource_type();
        let ids: Vec<String> = resources
            .iter()
            .map(|r| r.resource_id().to_string())
            .collect();

        self.resources
            .entry(resource_type.to_string())
            .or_insert_with(Vec::new)
            .extend(ids);
    }

    /// Add a single resource
    pub fn add_resource<T: Identifiable>(&mut self, resource: &T) {
        let resource_type = T::resource_type();
        let id = resource.resource_id().to_string();

        self.resources
            .entry(resource_type.to_string())
            .or_insert_with(Vec::new)
            .push(id);
    }
}

/// Trait for queries to declare their cache behavior
pub trait CacheableQuery: LibraryQuery + Sized {
    /// Generate cache metadata for this query's result
    ///
    /// This is an instance method (not static) to allow the query to inspect
    /// its input parameters and customize metadata generation accordingly.
    fn generate_cache_metadata(&self, result: &Self::Output) -> CacheMetadata {
        let mut metadata = CacheMetadata::new();

        // Default implementation: try to extract identifiable resources
        // Queries should override this to handle complex result structures
        metadata.cacheable = Self::is_cacheable();
        metadata.cache_duration = Self::cache_duration();
        metadata.invalidation = Self::invalidation_strategy();

        metadata
    }

    /// Whether this query type should be cached
    fn is_cacheable() -> bool {
        true
    }

    /// Cache duration in seconds
    fn cache_duration() -> Option<u64> {
        None // Indefinite by default
    }

    /// Invalidation strategy
    fn invalidation_strategy() -> InvalidationStrategy {
        InvalidationStrategy::OnResourceChange {
            resource_types: Vec::new(),
        }
    }
}
```

### 1.4 Enhanced Query Response Wrapper

```rust
// core/src/infra/query/response.rs

use super::cache_metadata::CacheMetadata;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Wrapper for query responses that includes cache metadata
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct QueryResponse<T> {
    /// The actual query result data
    pub data: T,

    /// Cache metadata for normalization
    pub cache: CacheMetadata,

    /// Query execution metadata
    pub meta: QueryMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct QueryMeta {
    /// Query execution time in milliseconds
    pub execution_time_ms: u64,

    /// Query ID for debugging
    pub query_id: String,

    /// Timestamp of when this query was executed
    pub executed_at: chrono::DateTime<chrono::Utc>,
}

impl<T> QueryResponse<T> {
    pub fn new(data: T, cache: CacheMetadata, execution_time_ms: u64) -> Self {
        Self {
            data,
            cache,
            meta: QueryMeta {
                execution_time_ms,
                query_id: uuid::Uuid::new_v4().to_string(),
                executed_at: chrono::Utc::now(),
            },
        }
    }
}
```

### 1.5 Update QueryManager to Generate Cache Metadata

```rust
// core/src/infra/query/manager.rs (additions)

impl QueryManager {
    pub async fn dispatch_library_with_cache<Q: LibraryQuery + CacheableQuery>(
        &self,
        query: Q,
        library_id: Uuid,
        session: SessionContext,
    ) -> QueryResult<QueryResponse<Q::Output>>
    where
        Q::Output: Serialize,
    {
        let start = std::time::Instant::now();

        // Execute query normally
        let result = self.dispatch_library(query, library_id, session).await?;

        // Generate cache metadata based on query configuration
        let cache_metadata = Q::cache_metadata(&result); // Assumes result is &[Identifiable]

        let execution_time_ms = start.elapsed().as_millis() as u64;

        Ok(QueryResponse::new(result, cache_metadata, execution_time_ms))
    }
}
```

### 1.6 Enhanced Events with Resource Deltas

```rust
// core/src/infra/event/mod.rs (additions)

/// Enhanced entry event with full resource data for cache updates
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum Event {
    // ... existing events ...

    /// Entry was modified - includes delta for cache update
    EntryUpdated {
        library_id: Uuid,
        entry: Entry, // Full Entry data for cache update
    },

    /// File was updated - includes full File for cache
    FileUpdated {
        library_id: Uuid,
        file: File, // Full File domain object
    },

    /// Tag was modified
    TagUpdated {
        library_id: Uuid,
        tag: Tag,
    },

    /// Tag was applied to entries
    TagApplied {
        library_id: Uuid,
        tag_id: Uuid,
        entry_ids: Vec<Uuid>,
    },

    /// Tag was removed from entries
    TagRemoved {
        library_id: Uuid,
        tag_id: Uuid,
        entry_ids: Vec<Uuid>,
    },

    /// Location was updated
    LocationUpdated {
        library_id: Uuid,
        location: Location,
    },

    /// Job was updated
    JobUpdated {
        library_id: Uuid,
        job_id: Uuid,
        status: JobStatus,
        progress: Option<f32>,
    },

    // ... other events ...
}

/// Trait for events that contain resource updates
pub trait ResourceEvent {
    /// Extract the resource type and ID from this event
    fn resource_identity(&self) -> Option<(String, String)>;

    /// Extract the full resource data if available
    fn resource_data(&self) -> Option<serde_json::Value>;

    /// Get the resource type this event affects
    fn resource_types(&self) -> Vec<String>;
}

impl ResourceEvent for Event {
    fn resource_identity(&self) -> Option<(String, String)> {
        match self {
            Event::FileUpdated { file, .. } => {
                Some(("File".to_string(), file.id.to_string()))
            }
            Event::TagUpdated { tag, .. } => {
                Some(("Tag".to_string(), tag.id.to_string()))
            }
            Event::LocationUpdated { location, .. } => {
                Some(("Location".to_string(), location.id.to_string()))
            }
            Event::EntryUpdated { entry, .. } => {
                Some(("Entry".to_string(), entry.id.to_string()))
            }
            _ => None,
        }
    }

    fn resource_data(&self) -> Option<serde_json::Value> {
        match self {
            Event::FileUpdated { file, .. } => serde_json::to_value(file).ok(),
            Event::TagUpdated { tag, .. } => serde_json::to_value(tag).ok(),
            Event::LocationUpdated { location, .. } => serde_json::to_value(location).ok(),
            Event::EntryUpdated { entry, .. } => serde_json::to_value(entry).ok(),
            _ => None,
        }
    }

    fn resource_types(&self) -> Vec<String> {
        match self {
            Event::FileUpdated { .. } => vec!["File".to_string()],
            Event::TagApplied { .. } | Event::TagRemoved { .. } => {
                vec!["File".to_string(), "Tag".to_string()]
            }
            Event::LocationUpdated { .. } => vec!["Location".to_string()],
            _ => Vec::new(),
        }
    }
}
```

## Layer 2: Client-Side Implementation (Generic)

### 2.1 Normalized Cache Store (Swift)

```swift
// packages/swift-client/Sources/SpacedriveCache/NormalizedCache.swift

import Foundation
import Combine

/// Normalized entity cache with automatic UI updates
@MainActor
public class NormalizedCache: ObservableObject {
    // MARK: - Entity Store

    /// Normalized entity storage: "ResourceType:id" → JSON data
    private var entities: [String: Any] = [:]

    /// Query result index: queryKey → [resource cache keys]
    private var queryIndex: [String: [String]] = [:]

    /// Reverse index: resource cache key → [query keys that contain it]
    private var resourceQueries: [String: Set<String>] = [:]

    /// Published to trigger UI updates
    @Published private var updateTrigger: Int = 0

    // MARK: - Public API

    /// Store query result with normalization
    public func storeQueryResult<T: Identifiable>(
        queryKey: String,
        data: [T],
        metadata: CacheMetadata
    ) {
        // 1. Store entities in normalized form
        for item in data {
            let cacheKey = item.cacheKey()
            entities[cacheKey] = item

            // Update reverse index
            resourceQueries[cacheKey, default: []].insert(queryKey)
        }

        // 2. Store query index
        let resourceKeys = data.map { $0.cacheKey() }
        queryIndex[queryKey] = resourceKeys

        triggerUpdate()
    }

    /// Get query result from cache (reconstructed from entities)
    public func getQueryResult<T: Identifiable>(queryKey: String) -> [T]? {
        guard let resourceKeys = queryIndex[queryKey] else {
            return nil
        }

        // Reconstruct result from entities
        return resourceKeys.compactMap { key in
            entities[key] as? T
        }
    }

    /// Update a single entity atomically
    public func updateEntity<T: Identifiable>(_ entity: T) {
        let cacheKey = entity.cacheKey()
        entities[cacheKey] = entity

        // Trigger updates for all queries containing this entity
        if let affectedQueries = resourceQueries[cacheKey] {
            print("Updated \(cacheKey) → \(affectedQueries.count) queries affected")
        }

        triggerUpdate()
    }

    /// Update entity by ID with partial data (merge)
    public func patchEntity(
        resourceType: String,
        id: String,
        patch: [String: Any]
    ) {
        let cacheKey = "\(resourceType):\(id)"

        guard var entity = entities[cacheKey] as? [String: Any] else {
            print("️  Entity \(cacheKey) not in cache, skipping patch")
            return
        }

        // Merge patch into entity
        for (key, value) in patch {
            entity[key] = value
        }

        entities[cacheKey] = entity
        triggerUpdate()
    }

    /// Remove entity from cache
    public func removeEntity(resourceType: String, id: String) {
        let cacheKey = "\(resourceType):\(id)"
        entities.removeValue(forKey: cacheKey)

        // Update affected queries
        if let affectedQueries = resourceQueries[cacheKey] {
            for queryKey in affectedQueries {
                // Remove from query index
                queryIndex[queryKey]?.removeAll { $0 == cacheKey }
            }
            resourceQueries.removeValue(forKey: cacheKey)
        }

        triggerUpdate()
    }

    /// Invalidate entire query (remove from index, keep entities)
    public func invalidateQuery(queryKey: String) {
        // Remove query index, but keep entities (might be used by other queries)
        if let resourceKeys = queryIndex[queryKey] {
            for resourceKey in resourceKeys {
                resourceQueries[resourceKey]?.remove(queryKey)
            }
        }
        queryIndex.removeValue(forKey: queryKey)
    }

    // MARK: - Observation Helpers

    /// Get observable query result for SwiftUI
    public func observeQuery<T: Identifiable>(queryKey: String) -> some Publisher {
        $updateTrigger
            .compactMap { [weak self] _ in
                self?.getQueryResult(queryKey) as [T]?
            }
            .eraseToAnyPublisher()
    }

    /// Get observable single entity
    public func observeEntity<T: Identifiable>(id: T.Id) -> some Publisher {
        let cacheKey = T.cacheKeyFromId(id)

        return $updateTrigger
            .compactMap { [weak self] _ in
                self?.entities[cacheKey] as? T
            }
            .eraseToAnyPublisher()
    }

    private func triggerUpdate() {
        updateTrigger += 1
    }
}
```

### 2.2 Event-Driven Cache Updater

```swift
// packages/swift-client/Sources/SpacedriveCache/EventCacheUpdater.swift

import Foundation

/// Handles event stream and applies atomic cache updates
public class EventCacheUpdater {
    private let cache: NormalizedCache
    private let eventStream: AsyncThrowingStream<Event, Error>
    private var task: Task<Void, Never>?

    public init(cache: NormalizedCache, eventStream: AsyncThrowingStream<Event, Error>) {
        self.cache = cache
        self.eventStream = eventStream
    }

    /// Start listening to events and updating cache
    public func start() {
        task = Task { [weak self] in
            guard let self = self else { return }

            do {
                for try await event in self.eventStream {
                    await self.handleEvent(event)
                }
            } catch {
                print("Event stream error: \(error)")
            }
        }
    }

    /// Stop listening to events
    public func stop() {
        task?.cancel()
        task = nil
    }

    @MainActor
    private func handleEvent(_ event: Event) {
        switch event {
        case .FileUpdated(let libraryId, let file):
            cache.updateEntity(file)

        case .TagUpdated(let libraryId, let tag):
            cache.updateEntity(tag)

        case .LocationUpdated(let libraryId, let location):
            cache.updateEntity(location)

        case .TagApplied(let libraryId, let tagId, let entryIds):
            // Update multiple File entities to include this tag
            for entryId in entryIds {
                // Fetch tag from cache
                guard let tag = cache.getEntity(Tag.self, id: tagId) else { continue }

                // Update file's tags array
                if let file = cache.getEntity(File.self, id: entryId) {
                    var updatedFile = file
                    updatedFile.tags.append(tag)
                    cache.updateEntity(updatedFile)
                }
            }

        case .TagRemoved(let libraryId, let tagId, let entryIds):
            // Remove tag from multiple files
            for entryId in entryIds {
                if let file = cache.getEntity(File.self, id: entryId) {
                    var updatedFile = file
                    updatedFile.tags.removeAll { $0.id == tagId }
                    cache.updateEntity(updatedFile)
                }
            }

        case .EntryModified(let libraryId, let entryId):
            // For lightweight events without full data, invalidate specific queries
            // that contain this entry
            cache.invalidateQueriesContaining(resourceType: "File", id: entryId)

        case .JobUpdated(let libraryId, let jobId, let status, let progress):
            // Patch job entity
            cache.patchEntity(
                resourceType: "Job",
                id: jobId.uuidString,
                patch: [
                    "status": status,
                    "progress": progress as Any
                ]
            )

        default:
            break
        }
    }
}
```

### 2.3 Query Client with Cache Integration

```swift
// packages/swift-client/Sources/SpacedriveCache/CachedQueryClient.swift

import Foundation

/// Query client with automatic normalization and caching
public class CachedQueryClient {
    private let client: SpacedriveClient
    private let cache: NormalizedCache

    public init(client: SpacedriveClient, cache: NormalizedCache = NormalizedCache()) {
        self.client = client
        self.cache = cache
    }

    /// Execute query with automatic caching
    public func query<Input: Encodable, Output: Decodable>(
        _ method: String,
        input: Input,
        cachePolicy: CachePolicy = .cacheFirst
    ) async throws -> Output {
        let queryKey = generateQueryKey(method: method, input: input)

        switch cachePolicy {
        case .cacheFirst:
            // Check cache first
            if let cached: Output = cache.getQueryResult(queryKey: queryKey) {
                print("Cache HIT: \(queryKey)")
                return cached
            }
            fallthrough

        case .networkOnly:
            print("Fetching from network: \(queryKey)")
            let response: QueryResponse<Output> = try await client.query(method, input: input)

            // Normalize and cache response
            if let identifiableArray = response.data as? [any Identifiable] {
                await cache.storeQueryResult(
                    queryKey: queryKey,
                    data: identifiableArray,
                    metadata: response.cache
                )
            }

            return response.data

        case .cacheOnly:
            guard let cached: Output = cache.getQueryResult(queryKey: queryKey) else {
                throw CacheError.cacheMiss(queryKey: queryKey)
            }
            return cached
        }
    }

    /// Observe query result with automatic updates from cache
    public func observeQuery<Output: Identifiable>(
        _ method: String,
        input: some Encodable
    ) -> AsyncThrowingStream<[Output], Error> {
        let queryKey = generateQueryKey(method: method, input: input)

        return AsyncThrowingStream { continuation in
            Task {
                // Initial fetch
                do {
                    let result: [Output] = try await self.query(method, input: input)
                    continuation.yield(result)
                } catch {
                    continuation.finish(throwing: error)
                    return
                }

                // Subscribe to cache updates
                let cancellable = cache.observeQuery(queryKey: queryKey)
                    .sink { (result: [Output]) in
                        continuation.yield(result)
                    }

                continuation.onTermination = { _ in
                    cancellable.cancel()
                }
            }
        }
    }

    private func generateQueryKey(method: String, input: some Encodable) -> String {
        // Hash input to create stable query key
        let inputData = try! JSONEncoder().encode(input)
        let inputHash = inputData.hashValue
        return "\(method):\(inputHash)"
    }
}

public enum CachePolicy {
    case cacheFirst  // Check cache, fallback to network
    case networkOnly // Always fetch from network, update cache
    case cacheOnly   // Only use cache, error if miss
}

public enum CacheError: Error {
    case cacheMiss(queryKey: String)
}
```

## Layer 3: Query Implementation Examples

### Example 1: File Search Query with Cache Metadata

```rust
// core/src/ops/search/query.rs (additions)

use crate::infra::query::{CacheableQuery, CacheMetadata, QueryResponse};

impl CacheableQuery for FileSearchQuery {
    fn cache_metadata<T: Identifiable>(files: &[T]) -> CacheMetadata {
        let mut metadata = CacheMetadata::new();

        // Add all file resources
        metadata.add_resources(files);

        // Configure invalidation
        metadata.invalidation = InvalidationStrategy::OnEvents {
            event_types: vec![
                "FileUpdated".to_string(),
                "TagApplied".to_string(),
                "TagRemoved".to_string(),
            ],
        };

        metadata.cacheable = true;
        metadata.cache_duration = Some(300); // 5 minutes

        metadata
    }

    fn is_cacheable() -> bool {
        true
    }

    fn invalidation_strategy() -> InvalidationStrategy {
        InvalidationStrategy::OnEvents {
            event_types: vec!["FileUpdated", "TagApplied", "TagRemoved"]
                .into_iter()
                .map(String::from)
                .collect(),
        }
    }
}

// Enhanced query execution
impl FileSearchQuery {
    pub async fn execute_with_cache_metadata(
        self,
        context: Arc<CoreContext>,
        session: SessionContext,
    ) -> QueryResult<QueryResponse<Vec<File>>> {
        let start = std::time::Instant::now();

        // Execute search normally
        let files = self.execute(context, session).await?;

        // Generate cache metadata
        let mut cache_metadata = Self::cache_metadata(&files);

        // Add tag entities too (extracted from files)
        let mut all_tags = Vec::new();
        for file in &files {
            all_tags.extend(file.tags.iter().cloned());
        }
        all_tags.dedup_by_key(|t| t.id);
        cache_metadata.add_resources(&all_tags);

        let execution_time = start.elapsed().as_millis() as u64;

        Ok(QueryResponse::new(files, cache_metadata, execution_time))
    }
}
```

### Example 2: Directory Listing with Cache

```rust
// core/src/ops/files/query/directory_listing.rs (additions)

impl CacheableQuery for DirectoryListingQuery {
    fn cache_metadata<T: Identifiable>(files: &[T]) -> CacheMetadata {
        let mut metadata = CacheMetadata::new();
        metadata.add_resources(files);

        // Directory listings should invalidate when entries are added/removed/moved
        metadata.invalidation = InvalidationStrategy::OnEvents {
            event_types: vec![
                "EntryCreated".to_string(),
                "EntryDeleted".to_string(),
                "EntryMoved".to_string(),
                "FileUpdated".to_string(),
            ],
        };

        // Cache for 60 seconds (directories change less frequently)
        metadata.cache_duration = Some(60);

        metadata
    }
}
```

### Example 3: Tag List Query

```rust
// core/src/ops/tags/list/query.rs (new)

pub struct ListTagsQuery;

impl LibraryQuery for ListTagsQuery {
    type Input = ();
    type Output = Vec<Tag>;

    fn from_input(_input: Self::Input) -> QueryResult<Self> {
        Ok(Self)
    }

    async fn execute(
        self,
        context: Arc<CoreContext>,
        session: SessionContext,
    ) -> QueryResult<Self::Output> {
        // Fetch all tags from database
        // ...
    }
}

impl CacheableQuery for ListTagsQuery {
    fn cache_metadata<T: Identifiable>(tags: &[T]) -> CacheMetadata {
        let mut metadata = CacheMetadata::new();
        metadata.add_resources(tags);

        // Tags change rarely, cache indefinitely
        metadata.invalidation = InvalidationStrategy::OnEvents {
            event_types: vec![
                "TagCreated".to_string(),
                "TagUpdated".to_string(),
                "TagDeleted".to_string(),
            ],
        };

        metadata.cache_duration = None; // Indefinite

        metadata
    }
}
```

## Layer 4: SwiftUI Integration

### Example: Self-Updating Search View

```swift
// apps/ios/Spacedrive/Views/Search/SearchView.swift

import SwiftUI
import SpacedriveCache

struct SearchView: View {
    @StateObject private var cache = NormalizedCache.shared
    @State private var searchQuery: String = ""
    @State private var files: [File] = []

    var body: some View {
        VStack {
            SearchBar(text: $searchQuery, onSubmit: executeSearch)

            List(files, id: \.id) { file in
                FileRow(file: file)
                    // Each file automatically updates when EntryModified event arrives!
                    .id(file.id) // SwiftUI tracks by ID
            }
        }
        .onAppear {
            // Subscribe to cache updates for this query
            subscribeToCache()
        }
    }

    private func executeSearch() {
        Task {
            do {
                // Query with cache
                files = try await cache.client.query(
                    "query:files.search.v1",
                    input: FileSearchInput(query: searchQuery),
                    cachePolicy: .cacheFirst
                )
            } catch {
                print("Search error: \(error)")
            }
        }
    }

    private func subscribeToCache() {
        let queryKey = "search:\(searchQuery.hashValue)"

        Task {
            // Observe cache updates
            for await updatedFiles in cache.observeQuery(queryKey) as AsyncThrowingStream<[File], Error> {
                await MainActor.run {
                    self.files = updatedFiles
                }
            }
        }
    }
}
```

## Layer 5: Event Emission from Core

### Update Actions to Emit Resource Events

```rust
// core/src/ops/files/rename/action.rs (example)

impl LibraryAction for FileRenameAction {
    async fn execute(
        self,
        library: Arc<Library>,
        context: Arc<CoreContext>,
    ) -> ActionResult<Self::Output> {
        // 1. Perform the rename
        let entry_id = self.entry_id;
        let new_name = self.new_name.clone();

        // Database update...
        let updated_entry = update_entry_name(library, entry_id, new_name).await?;

        // 2. Emit event with full resource data for cache update
        if let Some(file) = construct_full_file_from_entry(library, &updated_entry).await? {
            context.events.emit(Event::FileUpdated {
                library_id: library.id(),
                file, // Full File object for cache replacement
            });
        }

        Ok(RenameOutput { success: true })
    }
}
```

### Helper: Construct File from Entry

```rust
// core/src/domain/file.rs (additions)

impl File {
    /// Construct a complete File from an entry ID by fetching all related data
    /// This is used when emitting events that need full resource data
    pub async fn from_entry_id(
        library: Arc<Library>,
        entry_id: Uuid,
    ) -> QueryResult<Self> {
        let db = library.db().conn();

        // Fetch entry
        let entry_model = entry::Entity::find()
            .filter(entry::Column::Uuid.eq(entry_id))
            .one(db)
            .await?
            .ok_or(QueryError::Internal(format!("Entry {} not found", entry_id)))?;

        // Fetch content identity
        let content_identity = if let Some(content_id) = entry_model.content_id {
            ContentIdentity::from_id(db, content_id).await?
        } else {
            None
        };

        // Fetch tags
        let tags = Tag::for_entry(db, entry_model.id).await?;

        // Fetch sidecars
        let sidecars = Sidecar::for_entry(db, entry_model.id).await?;

        // Fetch alternate paths (duplicates)
        let alternate_paths = Entry::find_by_content_id(db, entry_model.content_id).await?;

        // Construct Entry domain object
        let entry = Entry::from_model(entry_model)?;

        Ok(File::from_data(FileConstructionData {
            entry,
            content_identity,
            tags,
            sidecars,
            alternate_paths,
        }))
    }
}
```

## Event Design Patterns

### Pattern 1: Full Resource Events (Recommended)

**Use when**: Resource is small enough to send in full

```rust
Event::TagUpdated {
    library_id: Uuid,
    tag: Tag { /* full data */ },
}
```

**Benefit**: Client can atomically replace cached entity without re-fetching

### Pattern 2: Lightweight Events with Delta

**Use when**: Resource is large, only specific fields changed

```rust
Event::FileMetadataUpdated {
    library_id: Uuid,
    entry_id: Uuid,
    delta: FileMetadataDelta {
        name: Some("new_name.txt"),
        modified_at: Some(timestamp),
        // Other fields: None (unchanged)
    },
}
```

**Benefit**: Lower bandwidth, client merges delta into cached entity

### Pattern 3: Relationship Events

**Use when**: Relationship changed but resources unchanged

```rust
Event::TagApplied {
    library_id: Uuid,
    tag_id: Uuid,
    entry_ids: Vec<Uuid>,
}
```

**Benefit**: Client can update relationships without re-fetching full resources

## Cache Consistency Guarantees

### Optimistic Updates

```swift
// Example: Tag a file optimistically
func tagFile(fileId: UUID, tagId: UUID) async throws {
    // 1. Update cache immediately (optimistic)
    var file = cache.getEntity(File.self, id: fileId)!
    let tag = cache.getEntity(Tag.self, id: tagId)!
    file.tags.append(tag)
    cache.updateEntity(file)
    // UI updates immediately! ✨

    // 2. Send action to server
    do {
        try await client.action("action:tags.apply.v1", input: ApplyTagInput(
            fileId: fileId,
            tagId: tagId
        ))
        // Server confirms, event arrives, cache updated again (same state)
    } catch {
        // 3. Rollback on error
        file.tags.removeLast()
        cache.updateEntity(file)
        throw error
    }
}
```

### Eventual Consistency

- Client cache is **eventually consistent** with server
- Events provide the synchronization mechanism
- Optimistic updates improve perceived performance
- Conflicts handled by "last write wins" or custom merge logic

## Query Annotation API

### Declarative Cache Configuration

```rust
// core/src/ops/search/query.rs

impl FileSearchQuery {
    /// Declare what resources this query returns
    pub fn declares_resources() -> Vec<ResourceTypeDeclaration> {
        vec![
            ResourceTypeDeclaration {
                resource_type: "File",
                extraction: ResourceExtraction::Direct, // Files are top-level result
                includes_relationships: vec!["tags", "content_identity"],
            },
            ResourceTypeDeclaration {
                resource_type: "Tag",
                extraction: ResourceExtraction::Nested { path: "tags" }, // Tags nested in files
                includes_relationships: vec![],
            },
        ]
    }

    /// Declare what events should invalidate this query
    pub fn invalidation_events() -> Vec<InvalidationRule> {
        vec![
            InvalidationRule::OnResourceChange {
                resource_type: "File",
                // Only invalidate if changed file is in our result set
                condition: InvalidationCondition::InResultSet,
            },
            InvalidationRule::OnEvent {
                event_type: "TagApplied",
                // Re-fetch if tag applied to file in our results
                condition: InvalidationCondition::InResultSet,
            },
        ]
    }
}

pub struct ResourceTypeDeclaration {
    pub resource_type: &'static str,
    pub extraction: ResourceExtraction,
    pub includes_relationships: Vec<&'static str>,
}

pub enum ResourceExtraction {
    /// Resources are top-level in result (e.g., Vec<File>)
    Direct,

    /// Resources are nested (e.g., file.tags)
    Nested { path: &'static str },

    /// Resources are in a map (e.g., HashMap<Uuid, Tag>)
    InMap { key_path: &'static str },
}

pub enum InvalidationCondition {
    /// Invalidate only if changed resource is in this query's result
    InResultSet,

    /// Always invalidate when this event occurs
    Always,

    /// Custom condition (library_id matches, etc.)
    Custom(fn(&Event) -> bool),
}
```

## Advanced: Relationship Updates

### Nested Resource Updates

When a File's Tag changes, we need to update:
1. The Tag entity itself
2. All File entities that reference this Tag

```rust
// Event with cascade information
Event::TagUpdated {
    library_id: Uuid,
    tag: Tag { /* updated tag */ },
    affects_entities: EntityAffectMap {
        "File": vec![uuid1, uuid2, uuid3], // Files that have this tag
    },
}
```

```swift
// Client handles cascading updates
case .TagUpdated(let libraryId, let tag, let affects):
    // 1. Update tag entity
    cache.updateEntity(tag)

    // 2. Update all files that reference this tag
    if let fileIds = affects["File"] {
        for fileId in fileIds {
            // Re-fetch file to get updated tag data
            // OR merge tag into file's tags array
            if var file = cache.getEntity(File.self, id: fileId) {
                if let tagIndex = file.tags.firstIndex(where: { $0.id == tag.id }) {
                    file.tags[tagIndex] = tag
                    cache.updateEntity(file)
                }
            }
        }
    }
```

## Performance Considerations

### Memory Management

**Problem**: Unbounded cache growth

**Solution**: LRU eviction with size limits

```swift
class NormalizedCache {
    private var lruOrder: [String] = [] // Cache keys in LRU order
    private let maxEntities: Int = 10_000
    private let maxMemoryMB: Int = 100

    private func evictIfNeeded() {
        while entities.count > maxEntities {
            // Remove oldest entity
            guard let oldestKey = lruOrder.first else { break }
            entities.removeValue(forKey: oldestKey)
            lruOrder.removeFirst()

            // Clean up query indexes
            cleanupQueryIndexes(for: oldestKey)
        }
    }

    private func touchEntity(_ cacheKey: String) {
        // Move to end of LRU
        lruOrder.removeAll { $0 == cacheKey }
        lruOrder.append(cacheKey)
    }
}
```

### Network Efficiency

**Batch Event Updates**: Instead of sending individual events, batch related updates:

```rust
Event::BatchUpdate {
    library_id: Uuid,
    updates: Vec<ResourceUpdate>,
    transaction_id: Uuid,
}

pub struct ResourceUpdate {
    pub resource_type: String,
    pub resource_id: String,
    pub update_type: UpdateType,
    pub data: serde_json::Value,
}

pub enum UpdateType {
    Create,
    Update,
    Delete,
    Patch { fields: Vec<String> },
}
```

## Implementation Roadmap

### Phase 1: Core Infrastructure (Week 1)
- [ ] Create `Identifiable` trait in `core/src/domain/identifiable.rs`
- [ ] Implement `Identifiable` for File, Tag, Location, Entry, Job, Library
- [ ] Create `CacheMetadata` and `QueryResponse<T>` wrapper types
- [ ] Add `CacheableQuery` trait to query infrastructure

### Phase 2: Event Enhancement (Week 1-2)
- [ ] Add `*Updated` events with full resource data to Event enum
- [ ] Add `ResourceEvent` trait for extracting resource identities
- [ ] Update key actions to emit resource events (rename, tag, move)
- [ ] Add relationship events (TagApplied, TagRemoved)

### Phase 3: Swift Cache Implementation (Week 2-3)
- [ ] Create `NormalizedCache.swift` with entity store
- [ ] Create `EventCacheUpdater.swift` for event handling
- [ ] Create `CachedQueryClient.swift` wrapper
- [ ] Implement LRU eviction and memory management

### Phase 4: SwiftUI Integration (Week 3-4)
- [ ] Create `@CachedQuery` property wrapper for views
- [ ] Create `ObservedEntity` for individual resource observation
- [ ] Update existing views to use cached queries
- [ ] Add loading states and error handling

### Phase 5: TypeScript Implementation (Week 4-5)
- [ ] Port NormalizedCache to TypeScript
- [ ] Create React hooks: `useCachedQuery`, `useEntity`
- [ ] Update web app to use normalized cache

### Phase 6: Optimization (Ongoing)
- [ ] Add query deduplication (merge concurrent queries)
- [ ] Add prefetching strategies
- [ ] Add cache persistence (SQLite for offline)
- [ ] Add cache statistics and monitoring

## Benefits

### For Users
- **Instant updates** - UI updates immediately when data changes
- **Works offline** - Cached data available when disconnected
- **Lower battery usage** - Fewer network requests

### For Developers
- **Simple API** - Just use `@CachedQuery`, updates happen automatically
- **Type-safe** - Identifiable trait ensures consistency
- **Testable** - Mock cache for UI tests

### For System
- **Lower bandwidth** - Atomic updates instead of full re-fetches
- **Better performance** - Client-side joins eliminate network roundtrips
- **Real-time sync** - Event bus provides immediate updates

## Example: Complete Flow

```swift
// 1. USER SEARCHES FOR FILES
let files = try await cache.client.query(
    "query:files.search.v1",
    input: FileSearchInput(query: "photos")
)
// Returns 1000 files, all normalized in cache

// 2. USER RENAMES ONE FILE (on another device or in another view)
// Action executes → Core emits event
Event::FileUpdated {
    library_id: lib_uuid,
    file: File { id: file_123, name: "new_name.jpg", ... }
}

// 3. EVENT ARRIVES AT CLIENT
// EventCacheUpdater handles it:
cache.updateEntity(file) // Atomic update of 1 entity

// 4. UI AUTOMATICALLY UPDATES
// All views displaying this file re-render with new name
// Search results update
// Directory listings update
// Inspector panel updates
// All without re-fetching! ✨
```

## Comparison to Other Systems

| Feature | Apollo Client | React Query | Spacedrive Cache |
|---------|---------------|-------------|------------------|
| Normalization | GraphQL IDs | Query-based | UUID-based |
| Event-driven | Subscriptions | Manual invalidation | Event bus |
| Optimistic updates | Yes | Yes | Yes |
| Offline support | ️ Apollo Persist | ️ Manual | Planned |
| Cross-platform | JS only | JS only | Swift + TS + Rust |
| Type safety | ️ Codegen | ️ Generics | Derive-based |

## Critical Implementation Concerns

### 1. Concurrency Safety in Client Cache

**Problem**: Multiple threads updating cache simultaneously can cause race conditions

**Solution**: Thread-safe client-side cache implementation

**Swift Implementation**:

```swift
// For SwiftUI apps: Use @MainActor for UI thread safety
@MainActor
public class NormalizedCache: ObservableObject {
    // All mutations happen on main thread - simple and safe
    private var entities: [String: Any] = [:]
    private var queryIndex: [String: [String]] = [:]

    func updateEntity<T: Identifiable>(_ entity: T) {
        entities[entity.cacheKey()] = entity
        objectWillChange.send() // Trigger SwiftUI updates
    }
}

// For background processing (network, event handling):
// Use actor isolation for concurrent access
actor BackgroundCacheUpdater {
    private let mainCache: NormalizedCache

    func processEvent(_ event: Event) async {
        // Parse event
        // ...

        // Apply to main cache on main thread
        await MainActor.run {
            mainCache.updateEntity(updatedFile)
        }
    }
}
```

**TypeScript Implementation**:

```typescript
// For React/web: Use immutable updates with locks
export class NormalizedCache {
    private entities: Map<string, any> = new Map();
    private queryIndex: Map<string, string[]> = new Map();
    private updateLock: Promise<void> = Promise.resolve();

    async updateEntity<T extends Identifiable>(entity: T): Promise<void> {
        // Serialize updates to prevent race conditions
        this.updateLock = this.updateLock.then(async () => {
            const key = entity.cacheKey();
            this.entities.set(key, entity);
            this.notifySubscribers(key);
        });

        await this.updateLock;
    }
}
```

**Note**: The Rust core does **not** need a cache - it already has the database as the source of truth. The cache is purely client-side.

### 2. Event Ordering and Consistency

**Problem**: Events can arrive out of order, especially during network issues

**Solution**: Event versioning with reconciliation

```rust
// Add version numbers to all events
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct EventEnvelope {
    /// Sequential event number per library
    pub sequence: u64,

    /// Library this event belongs to
    pub library_id: Uuid,

    /// Timestamp when event was created
    pub timestamp: DateTime<Utc>,

    /// The actual event
    pub event: Event,
}

// Track sequence numbers
pub struct EventSequenceTracker {
    /// Last seen sequence per library
    last_sequence: HashMap<Uuid, u64>,
}

impl EventSequenceTracker {
    pub fn check_for_gaps(&mut self, envelope: &EventEnvelope) -> EventGapStatus {
        let last_seen = self.last_sequence
            .get(&envelope.library_id)
            .copied()
            .unwrap_or(0);

        if envelope.sequence == last_seen + 1 {
            // Expected sequence, no gap
            self.last_sequence.insert(envelope.library_id, envelope.sequence);
            EventGapStatus::Ok
        } else if envelope.sequence > last_seen + 1 {
            // Gap detected! Missed events
            EventGapStatus::Gap {
                expected: last_seen + 1,
                received: envelope.sequence,
                missing_count: (envelope.sequence - last_seen - 1) as usize,
            }
        } else {
            // Duplicate or old event
            EventGapStatus::Duplicate
        }
    }
}

pub enum EventGapStatus {
    Ok,
    Gap { expected: u64, received: u64, missing_count: usize },
    Duplicate,
}
```

**Client-side gap handling**:
```swift
class EventCacheUpdater {
    private var sequenceTracker = EventSequenceTracker()

    private func handleEvent(_ envelope: EventEnvelope) async {
        let gapStatus = sequenceTracker.checkForGaps(envelope)

        switch gapStatus {
        case .ok:
            // Process event normally
            await applyEventToCache(envelope.event)

        case .gap(let expected, let received, let missing):
            print("️  Event gap detected: expected \(expected), got \(received)")

            // Invalidate affected queries to force refetch
            await invalidateAffectedQueries(envelope.event)

            // Background reconciliation: fetch missing state
            Task.detached {
                await self.reconcileState(libraryId: envelope.libraryId)
            }

        case .duplicate:
            // Ignore duplicate events
            break
        }
    }

    private func reconcileState(libraryId: UUID) async {
        // Re-fetch critical queries to ensure consistency
        // This is a "catch-up" mechanism after missed events
        print("Reconciling state for library \(libraryId)")

        // Invalidate all queries for this library
        cache.invalidateLibrary(libraryId)
    }
}
```

### 3. Centralized Event Emission

**Problem**: Events emitted from multiple places can be inconsistent

**Solution**: EventEmitter service with transactional guarantees

```rust
// core/src/infra/event/emitter.rs

use super::EventBus;
use crate::domain::{File, Location, Tag};
use uuid::Uuid;

/// Centralized service for emitting cache-update events
/// Ensures events are created consistently and include proper resource data
pub struct CacheEventEmitter {
    event_bus: Arc<EventBus>,
    sequence_generator: Arc<Mutex<HashMap<Uuid, u64>>>, // library_id → sequence
}

impl CacheEventEmitter {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            event_bus,
            sequence_generator: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Emit a file update event with full resource data
    pub fn emit_file_updated(&self, library_id: Uuid, file: File) {
        let sequence = self.next_sequence(library_id);

        let envelope = EventEnvelope {
            sequence,
            library_id,
            timestamp: Utc::now(),
            event: Event::FileUpdated { library_id, file },
        };

        self.event_bus.emit(Event::Envelope(Box::new(envelope)));

        tracing::debug!(
            library_id = %library_id,
            sequence = sequence,
            resource = "File",
            "Emitted cache update event"
        );
    }

    /// Emit a tag update event
    pub fn emit_tag_updated(&self, library_id: Uuid, tag: Tag) {
        let sequence = self.next_sequence(library_id);

        let envelope = EventEnvelope {
            sequence,
            library_id,
            timestamp: Utc::now(),
            event: Event::TagUpdated { library_id, tag },
        };

        self.event_bus.emit(Event::Envelope(Box::new(envelope)));
    }

    /// Emit a relationship change event
    pub fn emit_tag_applied(&self, library_id: Uuid, tag_id: Uuid, entry_ids: Vec<Uuid>) {
        let sequence = self.next_sequence(library_id);

        let envelope = EventEnvelope {
            sequence,
            library_id,
            timestamp: Utc::now(),
            event: Event::TagApplied { library_id, tag_id, entry_ids },
        };

        self.event_bus.emit(Event::Envelope(Box::new(envelope)));
    }

    /// Emit multiple events in a transaction (atomic batch)
    pub fn emit_transaction(&self, library_id: Uuid, events: Vec<Event>) {
        let sequence = self.next_sequence(library_id);

        let envelope = EventEnvelope {
            sequence,
            library_id,
            timestamp: Utc::now(),
            event: Event::BatchUpdate {
                library_id,
                updates: events,
                transaction_id: Uuid::new_v4(),
            },
        };

        self.event_bus.emit(Event::Envelope(Box::new(envelope)));
    }

    fn next_sequence(&self, library_id: Uuid) -> u64 {
        let mut sequences = self.sequence_generator.lock().unwrap();
        let sequence = sequences.entry(library_id).or_insert(0);
        *sequence += 1;
        *sequence
    }
}

// Add to CoreContext
impl CoreContext {
    pub fn cache_events(&self) -> &CacheEventEmitter {
        &self.cache_event_emitter
    }
}
```

**Usage in Actions**:
```rust
// core/src/ops/files/rename/action.rs

impl LibraryAction for FileRenameAction {
    async fn execute(
        self,
        library: Arc<Library>,
        context: Arc<CoreContext>,
    ) -> ActionResult<Self::Output> {
        let entry_id = self.entry_id;

        // Perform rename in database
        let updated_entry = rename_entry(&library, entry_id, &self.new_name).await?;

        // Construct full File domain object
        let file = File::from_entry_id(library.clone(), entry_id).await?;

        // Emit through centralized emitter
        context.cache_events().emit_file_updated(library.id(), file);

        Ok(RenameOutput { success: true })
    }
}
```

### 4. Resource Versioning for Conflict Resolution

**Problem**: Optimistic updates need conflict detection

**Solution**: Add version field to domain models

```rust
// core/src/domain/file.rs (additions)

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct File {
    pub id: Uuid,

    /// Resource version - incremented on each update
    /// Used for optimistic concurrency control
    pub version: u64,

    // ... rest of fields
}

// Update strategy
pub enum MergeStrategy {
    /// Always use server version (default)
    ServerWins,

    /// Keep client version, reject server update
    ClientWins,

    /// Merge fields if both changed different things
    FieldLevelMerge,

    /// Use version with higher timestamp
    LastWriteWins,
}
```

**Client-side conflict handling**:
```swift
func handleFileUpdate(_ event: Event.FileUpdated) {
    let incomingFile = event.file

    guard let cachedFile = cache.getEntity(File.self, id: incomingFile.id) else {
        // Not in cache, just add it
        cache.updateEntity(incomingFile)
        return
    }

    // Check for conflicts
    if cachedFile.version > incomingFile.version {
        // Client has newer version - possible if optimistic update happened
        print("️  Version conflict: client=\(cachedFile.version) server=\(incomingFile.version)")

        // Strategy: Server wins (safest), but log the conflict
        cache.updateEntity(incomingFile)

        // Could implement more sophisticated merging here
    } else {
        // Normal case: server has newer or same version
        cache.updateEntity(incomingFile)
    }
}
```

### 5. Memory Management and GC

**Problem**: Unbounded cache growth consumes memory

**Solution**: Multi-tiered eviction strategy

```swift
class NormalizedCache {
    // Configuration
    private let maxEntities: Int = 10_000
    private let maxMemoryMB: Int = 100
    private let entityTTL: TimeInterval = 3600 // 1 hour

    // Tracking
    private var lruOrder: [String] = []
    private var accessTimestamps: [String: Date] = [:]
    private var referenceCount: [String: Int] = [:] // How many queries reference this

    /// Update entity with automatic GC
    func updateEntity<T: Identifiable>(_ entity: T) {
        let cacheKey = entity.cacheKey()

        // Store entity
        entities[cacheKey] = entity
        accessTimestamps[cacheKey] = Date()

        // Update LRU
        touchEntity(cacheKey)

        // Check if eviction needed
        if entities.count > maxEntities {
            evictLRU()
        }

        triggerUpdate()
    }

    private func evictLRU() {
        // Sort by: refCount (0 first) → lastAccess (oldest first)
        let candidates = entities.keys.sorted { key1, key2 in
            let ref1 = referenceCount[key1] ?? 0
            let ref2 = referenceCount[key2] ?? 0

            if ref1 != ref2 {
                return ref1 < ref2 // Unreferenced first
            }

            let time1 = accessTimestamps[key1] ?? Date.distantPast
            let time2 = accessTimestamps[key2] ?? Date.distantPast
            return time1 < time2 // Older first
        }

        // Evict until under limit
        let toEvict = entities.count - (maxEntities * 90 / 100) // Evict to 90%

        for i in 0..<min(toEvict, candidates.count) {
            let key = candidates[i]

            // Don't evict if still referenced by active queries
            if let refCount = referenceCount[key], refCount > 0 {
                continue
            }

            entities.removeValue(forKey: key)
            accessTimestamps.removeValue(forKey: key)
            referenceCount.removeValue(forKey: key)

            print("️  Evicted: \(key)")
        }
    }

    /// Increment reference count when query adds entity
    func incrementRefCount(_ cacheKey: String) {
        referenceCount[cacheKey, default: 0] += 1
    }

    /// Decrement reference count when query is invalidated
    func decrementRefCount(_ cacheKey: String) {
        if let count = referenceCount[cacheKey], count > 0 {
            referenceCount[cacheKey] = count - 1
        }
    }
}
```

### 6. Background Reconciliation for Missed Events

**Problem**: Client disconnects, misses events, cache becomes stale

**Solution**: State reconciliation on reconnect

```rust
// core/src/infra/sync/reconciliation.rs

pub struct StateReconciliationService;

impl StateReconciliationService {
    /// Get all changes since a specific event sequence
    pub async fn get_changes_since(
        &self,
        library_id: Uuid,
        since_sequence: u64,
    ) -> QueryResult<Vec<ResourceChange>> {
        // Query audit log / event log for changes
        // Return list of resources that changed
        todo!()
    }

    /// Full state snapshot for complete cache rebuild
    pub async fn get_full_state_snapshot(
        &self,
        library_id: Uuid,
        resource_types: Vec<String>,
    ) -> QueryResult<StateSnapshot> {
        // Return all entities of requested types
        todo!()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceChange {
    pub resource_type: String,
    pub resource_id: Uuid,
    pub change_type: ChangeType,
    pub data: Option<serde_json::Value>,
    pub sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Created,
    Updated,
    Deleted,
}
```

**Client usage**:
```swift
class CacheReconciliationService {
    func reconcileOnReconnect(libraryId: UUID) async throws {
        let lastSequence = cache.getLastSequence(libraryId: libraryId)

        print("Reconciling from sequence \(lastSequence)")

        // Fetch all changes since last known sequence
        let changes = try await client.query(
            "query:sync.changes_since.v1",
            input: ChangesSinceInput(
                libraryId: libraryId,
                sinceSequence: lastSequence
            )
        )

        // Apply changes in order
        for change in changes.sorted(by: { $0.sequence < $1.sequence }) {
            switch change.changeType {
            case .created, .updated:
                if let data = change.data {
                    await cache.updateFromJSON(
                        resourceType: change.resourceType,
                        id: change.resourceId,
                        json: data
                    )
                }
            case .deleted:
                await cache.removeEntity(
                    resourceType: change.resourceType,
                    id: change.resourceId
                )
            }
        }

        print("Reconciliation complete: applied \(changes.count) changes")
    }
}
```

## Implementation Strategy Refinements

### Refinement 1: Instance Method for cache_metadata

**Original**: `fn cache_metadata<T: Identifiable>(result: &[T]) -> CacheMetadata`
**Improved**: `fn generate_cache_metadata(&self, result: &Self::Output) -> CacheMetadata`

```rust
impl CacheableQuery for FileSearchQuery {
    fn generate_cache_metadata(&self, result: &Self::Output) -> CacheMetadata {
        let mut metadata = CacheMetadata::new();

        // Access query input to customize caching
        if self.input.query.len() < 3 {
            // Don't cache very short searches (too dynamic)
            metadata.cacheable = false;
            return metadata;
        }

        // Extract files from search output
        for search_result in &result.results {
            // Handle the actual result structure
            metadata.add_resource(&search_result.file);

            // Add nested resources (tags)
            for tag in &search_result.file.tags {
                metadata.add_resource(tag);
            }
        }

        // Configure based on search mode
        metadata.cache_duration = match self.input.mode {
            SearchMode::Fast => Some(300),  // 5 minutes
            SearchMode::Normal => Some(60), // 1 minute (less stable)
            SearchMode::Full => Some(600),  // 10 minutes (expensive to recompute)
        };

        metadata
    }
}
```

### Refinement 2: Centralized Event Creation in Actions

**Pattern**: All events emitted at end of action execution

```rust
// core/src/infra/action/manager.rs (additions)

impl ActionManager {
    pub async fn dispatch_library<A: LibraryAction>(
        &self,
        library_id: Option<Uuid>,
        action: A,
    ) -> Result<A::Output, ActionError> {
        let library_id = library_id.ok_or(/*...*/)?;
        let library = self.context.get_library(library_id).await?;

        // Execute action
        let result = action.execute(library.clone(), self.context.clone()).await;

        // Emit cache events AFTER successful execution
        if let Ok(ref output) = result {
            // Actions can optionally implement CacheEventEmitter trait
            if let Some(events) = action.generate_cache_events(library_id, output) {
                for event in events {
                    self.context.cache_events().emit(library_id, event);
                }
            }
        }

        result
    }
}

/// Optional trait for actions to declare what cache events they generate
pub trait CacheEventEmitter {
    type Output;

    /// Generate cache events after successful execution
    fn generate_cache_events(
        &self,
        library_id: Uuid,
        output: &Self::Output,
    ) -> Option<Vec<CacheableEvent>> {
        None // Default: no special cache events
    }
}

pub enum CacheableEvent {
    FileUpdated(File),
    TagUpdated(Tag),
    LocationUpdated(Location),
    RelationshipChanged {
        resource_type: String,
        resource_id: Uuid,
        relationship: String,
        added: Vec<String>,
        removed: Vec<String>,
    },
}
```

### Refinement 3: Use File Instead of Entry for Clients

**Rationale**: File is richer, Entry is database-level

```rust
// Don't implement Identifiable for Entry (keep it internal)
// Only expose File to clients

impl Event {
    // Don't emit Entry events to clients
    // EntryModified { entry_id: Uuid }

    // Emit File events with full data
    FileUpdated {
        library_id: Uuid,
        file: File, // Complete File domain object
    },

    // For lightweight updates, use delta pattern
    FileMetadataChanged {
        library_id: Uuid,
        file_id: Uuid,
        changes: FileMetadataDelta,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FileMetadataDelta {
    pub name: Option<String>,
    pub size: Option<u64>,
    pub modified_at: Option<DateTime<Utc>>,
    // Only include fields that changed
}
```

**Entry → File conversion happens server-side**:
```rust
// When indexer updates an entry, emit File event
impl IndexingJob {
    async fn process_entry(&mut self, entry: entry::Model) {
        // Update database...

        // Construct File domain object
        let file = File::from_entry_id(self.library.clone(), entry.uuid?).await?;

        // Emit to clients (not Entry, but File!)
        self.context.cache_events().emit_file_updated(self.library.id(), file);
    }
}
```

## Open Questions (Revised)

1. **Partial events**: Should we always send full resources, or support delta updates?
   - **Decision**: Start with full resources for File/Tag/Location (< 10KB typically)
   - Add `FileMetadataDelta` for large objects with many relationships
   - Client merges deltas into cached entities

2. **Cache persistence**: Should cache survive app restarts?
   - **Decision**: Phase 2 feature - persist to SQLite for offline access
   - Use sequence numbers to validate cache on startup
   - Implement "stale while revalidate" pattern

3. **Cache invalidation**: What if event is missed (network drop)?
   - **Solved**: Event versioning with sequence numbers
   - Gap detection triggers background reconciliation
   - Fallback: invalidate affected queries, force refetch

4. **Resource versions**: Should resources have version numbers for conflict resolution?
   - **Solved**: Add `version: u64` field to all Identifiable resources
   - Increment on each update
   - Client checks version before applying optimistic updates

5. **Garbage collection**: When to remove entities no longer in any query?
   - **Solved**: Reference counting + LRU eviction
   - Evict entities with refCount = 0 and not accessed recently
   - Configurable limits: maxEntities, maxMemoryMB, entityTTL

## Handling Complex Relationships

### The Challenge

The `extract_relationships()` method can become complex for deeply nested domain models. Consider `File`:

```rust
pub struct File {
    pub id: Uuid,
    pub sd_path: SdPath,              // Contains device_id (relationship!)
    pub tags: Vec<Tag>,               // Many-to-many relationship
    pub sidecars: Vec<Sidecar>,       // One-to-many relationship
    pub content_identity: Option<ContentIdentity>, // One-to-one relationship
    pub alternate_paths: Vec<SdPath>, // Implicit relationship to other Files
    // ...
}
```

### Solution: Layered Relationship Extraction

```rust
impl Identifiable for File {
    fn extract_relationships(&self) -> ResourceRelationships {
        let mut rels = ResourceRelationships::new();

        // Layer 1: Direct relationships (IDs are explicit)
        for tag in &self.tags {
            rels.add_to_collection("tags", Tag::cache_key_from_id(&tag.id));
        }

        if let Some(content) = &self.content_identity {
            rels.add_singular("content_identity", ContentIdentity::cache_key_from_id(&content.uuid));
        }

        // Layer 2: Derived relationships (require parsing)
        // Extract location from sd_path
        if let Some(location_id) = self.infer_location_id() {
            rels.add_singular("location", Location::cache_key_from_id(&location_id));
        }

        // Extract device from sd_path
        if let SdPath::Physical { device_id, .. } = &self.sd_path {
            rels.add_singular("device", Device::cache_key_from_id(device_id));
        }

        // Layer 3: Implicit relationships (duplicates)
        // Note: alternate_paths represent other Files with same content
        // We don't extract these as explicit relationships to avoid circular deps
        // The client can query for duplicates when needed

        rels
    }

    /// Helper: Infer location ID from sd_path
    /// This requires looking up which location contains this path
    fn infer_location_id(&self) -> Option<Uuid> {
        // Implementation would query location registry
        // For now, we can include location_id explicitly in File struct
        // See improvement below
        None
    }
}

// IMPROVEMENT: Add explicit location_id to File
// This avoids complex inference logic
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct File {
    pub id: Uuid,
    pub location_id: Option<Uuid>, // Explicit relationship
    pub sd_path: SdPath,
    pub tags: Vec<Tag>,
    pub content_identity: Option<ContentIdentity>,
    // ...
}

impl Identifiable for File {
    fn extract_relationships(&self) -> ResourceRelationships {
        let mut rels = ResourceRelationships::new();

        // Much simpler now!
        if let Some(loc_id) = self.location_id {
            rels.add_singular("location", Location::cache_key_from_id(&loc_id));
        }

        for tag in &self.tags {
            rels.add_to_collection("tags", Tag::cache_key_from_id(&tag.id));
        }

        if let Some(content) = &self.content_identity {
            rels.add_singular("content_identity", ContentIdentity::cache_key_from_id(&content.uuid));
        }

        rels
    }
}
```

### Circular Relationship Handling

**Problem**: File references Tag, Tag might reference Files (via search)

**Solution**: One-directional relationships in cache graph

```rust
// File → Tag (stored)
// Tag → Files (not stored, computed via reverse lookup)

impl NormalizedCache {
    /// Get all files that have a specific tag (reverse lookup)
    fn files_with_tag(&self, tag_id: Uuid) -> Vec<File> {
        let tag_cache_key = Tag::cache_key_from_id(&tag_id);

        self.entities
            .values()
            .filter_map(|entity| entity as? File)
            .filter(|file| {
                file.tags.iter().any(|t| t.id == tag_id)
            })
            .collect()
    }
}
```

### Relationship Update Patterns

**Pattern 1**: Many-to-many (Tag File)

```rust
// When tag is applied to file
Event::TagApplied {
    library_id: Uuid,
    tag_id: Uuid,
    entry_ids: Vec<Uuid>, // Files affected
}

// Client handler:
// 1. Fetch tag entity from cache
// 2. For each entry_id, update that File's tags array
// 3. Don't update Tag entity (it doesn't store reverse refs)
```

**Pattern 2**: One-to-many (Location → Files)

```rust
// When location is updated
Event::LocationUpdated {
    library_id: Uuid,
    location: Location, // Full location data
}

// Client handler:
// 1. Update Location entity
// 2. Don't need to update Files (they reference location_id, not vice versa)
// 3. UI will see new location data automatically via relationships
```

**Pattern 3**: Cascading updates (rename Location → all Files in it)

```rust
// When location is renamed
Event::LocationRenamed {
    library_id: Uuid,
    location: Location, // Updated location
    affected_file_count: usize, // For UI feedback
}

// Client handler:
// 1. Update Location entity
// 2. All Files with this location_id will show new location name
//    automatically via join (no need to update each File!)
```

## Phased Rollout Strategy

### Phase 1A: Core Infrastructure (Week 1)
- Create `Identifiable` trait
- Implement for File, Tag, Location, Job
- Add `version` field to domain models
- Create `CacheMetadata` and `QueryResponse<T>`
- Add `CacheableQuery` trait with instance method

### Phase 1B: Event Infrastructure (Week 1-2)
- Create `EventEnvelope` with sequence numbers
- Create `CacheEventEmitter` service
- Add to `CoreContext`
- Create new event types: `FileUpdated`, `TagUpdated`, etc.

### Phase 2A: Swift Prototype (Week 2-3)
- Implement `NormalizedCache` for File only (narrow scope)
- Test with file search query
- Implement `EventCacheUpdater` for File events
- Measure performance vs query-based approach

### Phase 2B: Expand to More Resources (Week 3-4)
- Add Tag, Location, Job to cache
- Test relationship updates
- Implement reference counting and GC

### Phase 3: Production Hardening (Week 4-6)
- Add event versioning and gap detection
- Implement reconciliation service
- Add conflict resolution for optimistic updates
- Performance testing and optimization
- Memory profiling and tuning

### Phase 4: TypeScript Port (Week 6-8)
- Port NormalizedCache to TypeScript
- Create React hooks
- Update web app

### Phase 5: Advanced Features (Ongoing)
- Cache persistence (SQLite)
- Prefetching strategies
- Query deduplication
- Analytics and monitoring

## Risk Mitigation

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Complexity overwhelms team | Medium | High | Start with File only, iterate |
| Cache becomes stale | Medium | High | Event versioning + reconciliation |
| Memory issues on mobile | High | Medium | Aggressive LRU eviction, configurable limits |
| Relationship logic bugs | High | Medium | Comprehensive tests, start simple |
| Event order issues | Medium | High | Sequence numbers + gap detection |
| Performance regression | Low | High | Benchmark before/after, A/B test |

## Success Metrics

### Performance Targets
- **UI responsiveness**: < 16ms for cache hits (60fps)
- **Network reduction**: 80% fewer queries after initial load
- **Memory usage**: < 100MB for 10k cached entities
- **Event latency**: < 100ms from action → cache update → UI

### User Experience Goals
- Instant UI updates when data changes
- App works offline with cached data
- 50% reduction in battery usage from fewer network calls
- Real-time sync across devices

## Why Client-Side Only?

### Server-Side Cache is Redundant

The Rust core **should not** have a cache layer because:

1. **Database IS the cache** - SeaORM with PostgreSQL/SQLite is already highly optimized
   - Indexes provide fast lookups
   - Query planner optimizes joins
   - Connection pooling handles concurrency
   - Adding another cache layer would just duplicate data

2. **Different problems being solved**:
   - **Database**: Persistent storage, ACID guarantees, query optimization
   - **Client cache**: Network latency, offline access, instant UI updates
   - These are orthogonal concerns!

3. **Complexity without benefit**:
   - Server cache needs invalidation logic (when DB updates)
   - Cache coherency between cache and DB
   - More memory usage on server
   - More code to maintain
   - Minimal performance gain (DB queries are already fast locally)

4. **Queries should be fast enough**:
   - Core is local (same machine or local network)
   - Database queries are microseconds to milliseconds
   - The bottleneck is network latency (client → core), not DB queries

### The Client-Side Cache Solves Real Problems

The normalized cache on **clients** makes sense because:

- **Network latency**: 100ms+ round trip vs 0ms cache hit
- **Bandwidth**: Don't re-fetch unchanged data
- **Offline**: App works when disconnected
- **Real-time UI**: Atomic updates instead of full refreshes
- **Battery life**: Fewer network operations on mobile

### Architecture Clarity

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ Swift Client │     │  Web Client  │     │  CLI Client  │
│              │     │              │     │              │
│ Cache     │     │ Cache     │     │ No Cache  │
│ (Memory)     │     │ (Memory)     │     │ (Stateless)  │
└──────┬───────┘     └──────┬───────┘     └──────┬───────┘
       │                    │                    │
       └────────────────────┼────────────────────┘
                           │ Network (bottleneck!)
                           │
                    ┌──────▼───────┐
                    │ Rust Core    │
                    │              │
                    │ No Cache  │
                    │ Database  │ ← Single source of truth
                    └──────────────┘
```

**Takeaway**: Cache at the network boundary (clients), not at the data source (core).

## Next Steps

1. **Design approved** - Incorporate review feedback (DONE)
2. **Start Phase 1A** - Implement `Identifiable` trait in Rust
3. **Prototype Phase 2A** - Build Swift NormalizedCache for File
4. **Measure and iterate** - Compare performance metrics
5. **Expand gradually** - Add more resource types based on learnings

---

This design provides a **foundation for instant, real-time UI updates** across all Spacedrive clients while minimizing network overhead and enabling offline functionality. The phased approach mitigates risk while delivering value incrementally.

**Critical Design Principle**: Cache where the latency is (client core), not where the data is (core database).
