<!--CREATED: 2025-06-24-->
# Lightning Search: Next-Generation File Discovery for Spacedrive

Note: in recent versions of the whitepaper we now refer to search as "Temporal-Sematic Search", or simply just "search".

## Overview

Lightning Search is a revolutionary multi-modal file discovery system designed specifically for Spacedrive's VDFS (Virtual Distributed File System) architecture. The system combines blazing-fast temporal search with intelligent semantic understanding, delivering sub-100ms query responses across millions of files while maintaining complete user privacy through local processing.

## Architecture Philosophy

### Temporal-First, Vector-Enhanced Search (VSS-Native)

Lightning Search employs a sophisticated two-stage architecture:

1. **Temporal Engine** (SQLite FTS5) provides instant text-based discovery and acts as a high-performance filter
2. **Semantic Engine** (VSS-managed embeddings) performs semantic analysis on temporal results for intelligent ranking and discovery

This approach ensures that vector search operations are performed only on pre-filtered, relevant datasets, dramatically improving performance while maintaining semantic intelligence.

```
User Query → Temporal Engine (FTS5) → Filtered Results → Semantic Engine (VSS embeddings) → Ranked Results
             ↑ <10ms                   ↑ 100-1000 items            ↑ +50ms                     ↑ Final Results
```

### Revised Search Architecture: A VSS-Native Approach

Search is a primary consumer of the Virtual Sidecar System (VSS). It remains a Hybrid Temporal–Semantic Search, with the semantic component powered directly by VSS-managed embedding sidecars. This eliminates any external vector database dependencies and makes the semantic index portable with the library.

## Core Components

### 1. Temporal Search Engine (SQLite FTS5)

The foundation layer providing instant text-based discovery integrated directly with the VDFS schema:

```sql
-- FTS5 Virtual Table integrated with entries (metadata-only)
CREATE VIRTUAL TABLE search_index USING fts5(
    content='entries',
    content_rowid='id',
    name,
    extension,
    tokenize="unicode61 remove_diacritics 2 tokenchars '.@-'"
);

-- Real-time triggers for immediate index updates
CREATE TRIGGER entries_search_insert AFTER INSERT ON entries BEGIN
    INSERT INTO search_index(rowid, name, extension)
    VALUES (new.id, new.name, new.extension);
END;

CREATE TRIGGER entries_search_update AFTER UPDATE ON entries BEGIN
    UPDATE search_index SET
        name = new.name,
        extension = new.extension
    WHERE rowid = new.id;
END;
```

> **Implementation Note:** These raw SQL statements for FTS5 will be executed using SeaORM's APIs (e.g., `db.execute()` and `Entity::find().from_raw_sql()`). This requires the underlying `rusqlite` database driver, which SeaORM uses, to be compiled with FTS5 support enabled via its feature flag in `Cargo.toml`.

**Performance Characteristics:**

- **Query Speed**: <10ms for simple queries, <30ms for complex patterns
- **Index Size**: ~15% of total database size
- **Update Latency**: Real-time via triggers (<1ms)
- **Throughput**: >10,000 queries/second

#### Path and Date Scoping with FTS5

FTS5 remains metadata-only (name/extension). Path and date constraints are applied via `entries` and the directory closure table, intersected with FTS candidates. Two execution patterns are supported and chosen dynamically based on selectivity:

- FTS-first (broad folder/date, selective text):

```sql
WITH fts AS (
  SELECT rowid, bm25(search_index) AS rank
  FROM search_index
  WHERE search_index MATCH :q
  ORDER BY rank
  LIMIT 5000
)
SELECT e.id, fts.rank
FROM fts
JOIN entries e ON e.id = fts.rowid
JOIN directory_closure dc ON dc.descendant_dir_id = e.directory_id
WHERE dc.ancestor_dir_id = :dir_id
  AND e.kind = 0
  AND e.modified_at BETWEEN :from AND :to
ORDER BY fts.rank
LIMIT 200;
```

- Filter-first (tight folder/date, broader text):

```sql
WITH cand AS (
  SELECT e.id
  FROM entries e
  JOIN directory_closure dc ON dc.descendant_dir_id = e.directory_id
  WHERE dc.ancestor_dir_id = :dir_id
    AND e.kind = 0
    AND e.modified_at BETWEEN :from AND :to
  LIMIT 100000
)
SELECT e.id, bm25(si) AS rank
FROM cand c
JOIN search_index si ON si.rowid = c.id
JOIN entries e ON e.id = c.id
WHERE si MATCH :q
ORDER BY rank
LIMIT 200;
```

Recommended supporting indexes:

- `CREATE INDEX IF NOT EXISTS idx_entries_recent ON entries(modified_at DESC) WHERE kind = 0;`
- `CREATE INDEX IF NOT EXISTS idx_entries_created ON entries(created_at DESC) WHERE kind = 0;`
- `CREATE INDEX IF NOT EXISTS idx_entries_dir_modified ON entries(directory_id, modified_at DESC) WHERE kind = 0;`
- `CREATE INDEX IF NOT EXISTS idx_entries_dir_created ON entries(directory_id, created_at DESC) WHERE kind = 0;`

### The `SearchRequest` API

The `SearchRequest` struct is the primary input for any search operation. It is designed to be expressive, type-safe, and extensible, capturing the full range of search capabilities envisioned for Spacedrive.

```rust
/// The main entry point for all search operations.
/// It serves as the parameter object for a SearchJob.
pub struct SearchRequest {
    /// The primary text query. Can be a filename, content snippet, or natural language.
    pub query: String,

    /// The mode of search, determining the trade-off between speed and comprehensiveness.
    pub mode: SearchMode,

    /// The scope to which the search should be restricted.
    pub scope: SearchScope,

    /// Options that toggle specific search behaviors.
    pub options: SearchOptions,

    /// The desired sorting for the results.
    pub sort: Sort,

    /// Pagination for the result set.
    pub pagination: Pagination,

    /// A collection of structured filters to narrow down the search.
    pub filters: SearchFilters,
}

/// Defines the scope of the filesystem to search within.
#[derive(Default)]
pub enum SearchScope {
    /// Search the entire library (default).
    #[default]
    Library,
    /// Restrict search to a specific location by its ID.
    Location { location_id: Uuid },
    /// Restrict search to a specific directory path and all its descendants.
    Path { path: SdPath },
}

/// Defines boolean toggles and other options for the search.
pub struct SearchOptions {
    /// If true, results with the same `content_uuid` will be deduplicated,
    /// showing only the best-ranked instance of each unique content.
    pub unique_by_content: bool,
    /// If true, the text query will be case-sensitive.
    pub case_sensitive: bool,
    /// If true, the search result will include facet information (e.g., counts per file type).
    pub request_facets: bool,
}

/// Defines the sorting field and direction for the search results.
pub struct Sort {
    pub field: SortField,
    pub direction: SortDirection,
}

pub enum SortField {
    /// Sort by relevance score (default).
    Relevance,
    ModifiedAt,
    CreatedAt,
    Name,
    Size,
}

pub enum SortDirection { Desc, Asc }

/// Defines the pagination for the result set.
pub struct Pagination {
    /// The maximum number of results to return.
    pub limit: u32,
    /// The number of results to skip from the beginning.
    pub offset: u32,
}

/// A container for all structured filters.
/// Fields are optional, allowing for flexible query composition.
#[derive(Default)]
pub struct SearchFilters {
    pub time_range: Option<TimeFilter>,
    pub size_range: Option<SizeFilter>,
    pub content_types: Option<Vec<ContentType>>,
    pub tags: Option<TagFilter>,
    // Extensible list for other specific filters.
    pub other: Vec<OtherFilter>,
}

/// A filter for a time-based field.
pub struct TimeFilter {
    pub field: TimeField,
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
}
pub enum TimeField { CreatedAt, ModifiedAt }

/// A filter for file size in bytes.
pub struct SizeFilter {
    pub min: Option<u64>,
    pub max: Option<u64>,
}

/// A filter for tags, supporting complex boolean logic.
pub struct TagFilter {
    // e.g., (tag1 AND tag2) OR (tag3 AND NOT tag4)
    // This structure can be defined more concretely as needed.
    // For now, a simple list is proposed.
    pub include: Vec<Uuid>, // Must have all of these tag IDs.
    pub exclude: Vec<Uuid>, // Must not have any of these tag IDs.
}

/// An extensible enum for other types of filters.
pub enum OtherFilter {
    IsFavorite(bool),
    IsHidden(bool),
    Resolution { min_width: u32, min_height: u32 },
    Duration { min_seconds: u64, max_seconds: u64 },
}
```

### 2. Semantic Engine: VSS-Powered Vector Search

Semantic search using on-device embeddings stored as Virtual Sidecar artifacts (no external DB):

```rust
use crate::vss::{SidecarPathResolver, SidecarRepository};
use crate::file_type::FileTypeRegistry;

pub struct SemanticEngine {
    embedding_model: Arc<OnnxEmbeddingModel>,
    sidecars: Arc<SidecarRepository>,
}

impl SemanticEngine {
    async fn new(sidecars: Arc<SidecarRepository>) -> Result<Self> {
        Ok(Self {
            embedding_model: Arc::new(OnnxEmbeddingModel::load("all-MiniLM-L6-v2")?),
            sidecars,
        })
    }

    async fn rerank_with_embeddings(
        &self,
        query_text: &str,
        candidate_entry_ids: &[i32],
        model_name: &str,
    ) -> Result<Vec<ScoredResult>> {
        let query_vec = self.embedding_model.encode(query_text).await?;
        let mut results = Vec::new();

        for entry_id in candidate_entry_ids {
            if let Some((content_uuid, sidecar_path)) =
                self.sidecars.find_embedding_sidecar(*entry_id, model_name).await?
            {
                if let Some(file_vec) = self.sidecars.read_embedding_vector(&sidecar_path).await? {
                    let score = cosine_similarity(&query_vec, &file_vec);
                    results.push(ScoredResult { entry_id: *entry_id, content_uuid, score });
                }
            }
        }

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        Ok(results)
    }
}

// Sidecar file layout (deterministic):
// .sdlibrary/sidecars/content/{content_uuid}/embeddings/{model_name}.json
```

### 3. Content Extraction Pipeline

Intelligent content extraction leveraging the integrated file type system:

```rust
use crate::file_type::{FileTypeRegistry, IdentificationResult, ExtractionConfig};

pub struct ContentExtractor {
    file_type_registry: Arc<FileTypeRegistry>,
    text_extractors: HashMap<String, Box<dyn TextExtractor>>,
    image_analyzers: HashMap<String, Box<dyn ImageAnalyzer>>,
    metadata_extractors: HashMap<String, Box<dyn MetadataExtractor>>,
    cache: LruCache<ContentHash, ExtractedContent>,
}

impl ContentExtractor {
    async fn extract_searchable_content(&self, entry: &Entry) -> Result<ExtractedContent> {
        let cache_key = self.compute_content_hash(entry);

        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached.clone());
        }

        // Step 1: Identify file type using the integrated file type system
        let file_path = entry.full_path();
        let identification = self.file_type_registry.identify(&file_path).await?;

        // Step 2: Check if extraction is supported for this file type
        let extraction_config = identification.file_type.extraction_config
            .ok_or_else(|| ExtractionError::UnsupportedType(identification.file_type.id.clone()))?;

        // Step 3: Select appropriate extraction method based on file type configuration
        let content = self.extract_by_file_type(&identification, &extraction_config, entry).await?;

        self.cache.put(cache_key, content.clone());
        Ok(content)
    }

    async fn extract_by_file_type(
        &self,
        identification: &IdentificationResult,
        config: &ExtractionConfig,
        entry: &Entry
    ) -> Result<ExtractedContent> {
        let mut extracted = ExtractedContent::new(entry, identification.file_type.category);

        // Extract content based on configured methods
        for method in &config.methods {
            match method {
                ExtractionMethod::Text => {
                    extracted.text_content = self.extract_text_content(identification, entry).await?;
                },
                ExtractionMethod::Metadata => {
                    extracted.metadata = self.extract_file_metadata(identification, entry).await?;
                },
                ExtractionMethod::Structure => {
                    extracted.structure = self.extract_document_structure(identification, entry).await?;
                },
                ExtractionMethod::Thumbnails => {
                    extracted.thumbnail_path = self.generate_thumbnail(identification, entry).await?;
                },
            }
        }

        Ok(extracted)
    }

    async fn extract_text_content(
        &self,
        identification: &IdentificationResult,
        entry: &Entry
    ) -> Result<Option<String>> {
        match identification.file_type.category {
            ContentKind::Text | ContentKind::Code => {
                self.extract_plain_text(entry).await
            },
            ContentKind::Document => {
                match identification.file_type.id.as_str() {
                    "application/pdf" => self.extract_pdf_text(entry).await,
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
                        self.extract_docx_text(entry).await
                    },
                    _ => self.extract_plain_text(entry).await,
                }
            },
            ContentKind::Image => {
                // Use OCR for image text extraction
                self.extract_ocr_text(entry).await
            },
            _ => Ok(None),
        }
    }

    async fn extract_code_content(&self, entry: &Entry) -> Result<ExtractedContent> {
        let raw_content = fs::read_to_string(&entry.full_path()).await?;

        // Extract meaningful content for code files
        let mut searchable_parts = Vec::new();

        // Function/class names, comments, string literals
        searchable_parts.push(entry.name.clone());
        searchable_parts.extend(self.extract_code_symbols(&raw_content));
        searchable_parts.extend(self.extract_comments(&raw_content));
        searchable_parts.extend(self.extract_string_literals(&raw_content));

        Ok(ExtractedContent {
            primary_text: searchable_parts.join(" "),
            metadata: self.extract_code_metadata(&raw_content),
            content_type: ContentType::Code,
        })
    }

    /// Enhanced metadata extraction using file type system
    async fn extract_file_metadata(
        &self,
        identification: &IdentificationResult,
        entry: &Entry
    ) -> Result<HashMap<String, String>> {
        let mut metadata = HashMap::new();

        // Basic file information
        metadata.insert("file_type".to_string(), identification.file_type.id.clone());
        metadata.insert("category".to_string(), format!("{:?}", identification.file_type.category));
        metadata.insert("confidence".to_string(), identification.confidence.to_string());
        metadata.insert("identification_method".to_string(), format!("{:?}", identification.method));

        // Extract type-specific metadata
        match identification.file_type.category {
            ContentKind::Image => {
                if let Ok(exif_data) = self.extract_exif_metadata(entry).await {
                    metadata.extend(exif_data);
                }
            },
            ContentKind::Audio => {
                if let Ok(id3_data) = self.extract_id3_metadata(entry).await {
                    metadata.extend(id3_data);
                }
            },
            ContentKind::Video => {
                if let Ok(video_meta) = self.extract_video_metadata(entry).await {
                    metadata.extend(video_meta);
                }
            },
            ContentKind::Document => {
                if let Ok(doc_meta) = self.extract_document_metadata(entry).await {
                    metadata.extend(doc_meta);
                }
            },
            _ => {}
        }

        Ok(metadata)
    }
}
```

### 4. Enhanced File Type-Aware Extraction

The integration with the file type system enables sophisticated, type-aware content extraction:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionConfig {
    /// Supported extraction methods for this file type
    pub methods: Vec<ExtractionMethod>,

    /// Required external dependencies
    pub dependencies: Vec<String>,

    /// Extraction priority (higher = more important for search)
    pub priority: u8,

    /// Maximum file size to process (bytes)
    pub max_file_size: Option<u64>,

    /// Specific configuration per extraction method
    pub method_configs: HashMap<ExtractionMethod, MethodConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtractionMethod {
    /// Extract readable text content
    Text,

    /// Extract file metadata (EXIF, ID3, etc.)
    Metadata,

    /// Extract document structure (headings, tables, etc.)
    Structure,

    /// Generate thumbnails and previews
    Thumbnails,

    /// Extract semantic embeddings
    Embeddings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodConfig {
    /// Engine to use for extraction (e.g., "poppler", "tesseract", "exifread")
    pub engine: String,

    /// Engine-specific configuration
    pub settings: HashMap<String, serde_json::Value>,

    /// Fallback engines if primary fails
    pub fallbacks: Vec<String>,
}
```

#### File Type-Specific Extraction Examples

**PDF Documents:**

```toml
# core/src/file_type/definitions/documents.toml
[[file_types]]
id = "application/pdf"
name = "PDF Document"
extensions = ["pdf"]
mime_types = ["application/pdf"]
category = "document"
priority = 100

[file_types.extraction]
methods = ["text", "metadata", "structure", "thumbnails"]
priority = 95
max_file_size = 104857600  # 100MB

[file_types.extraction.method_configs.text]
engine = "poppler"
fallbacks = ["tesseract"]
settings = { preserve_layout = true, ocr_fallback = true }

[file_types.extraction.method_configs.thumbnails]
engine = "pdf2image"
settings = { page = 1, dpi = 150, format = "webp" }
```

**Source Code Files:**

```toml
# core/src/file_type/definitions/code.toml
[[file_types]]
id = "text/rust"
name = "Rust Source Code"
extensions = ["rs"]
mime_types = ["text/rust", "text/x-rust"]
category = "code"
priority = 90

[file_types.extraction]
methods = ["text", "structure", "embeddings"]
priority = 88

[file_types.extraction.method_configs.structure]
engine = "tree-sitter"
settings = { language = "rust", extract_symbols = true, extract_comments = true }

[file_types.extraction.method_configs.embeddings]
engine = "code-bert"
settings = { model = "microsoft/codebert-base", chunk_size = 512 }
```

**Image Files:**

```toml
# core/src/file_type/definitions/images.toml
[[file_types]]
id = "image/jpeg"
name = "JPEG Image"
extensions = ["jpg", "jpeg"]
mime_types = ["image/jpeg"]
category = "image"
priority = 95

[file_types.extraction]
methods = ["metadata", "text", "thumbnails", "embeddings"]
priority = 85

[file_types.extraction.method_configs.metadata]
engine = "exifread"
settings = { include_thumbnails = false, include_maker_notes = true }

[file_types.extraction.method_configs.text]
engine = "tesseract"
settings = { languages = ["eng"], confidence_threshold = 60 }

[file_types.extraction.method_configs.embeddings]
engine = "clip"
settings = { model = "openai/clip-vit-base-patch32" }
```

#### Intelligent Extraction Scheduling

```rust
pub struct ExtractionScheduler {
    file_type_registry: Arc<FileTypeRegistry>,
    priority_queue: PriorityQueue<ExtractionTask>,
    worker_pool: ThreadPool,
    system_monitor: SystemResourceMonitor,
}

impl ExtractionScheduler {
    pub async fn schedule_extraction(&self, entry: &Entry) -> Result<()> {
        // Identify file type and extraction capabilities
        let identification = self.file_type_registry.identify(&entry.full_path()).await?;

        if let Some(config) = &identification.file_type.extraction_config {
            let task_priority = self.calculate_extraction_priority(&identification, entry, config);

            let task = ExtractionTask {
                entry: entry.clone(),
                file_type: identification.file_type.clone(),
                config: config.clone(),
                priority: task_priority,
                scheduled_at: Utc::now(),
            };

            // Schedule based on system resources and task priority
            if self.system_monitor.can_handle_immediate_extraction() && task_priority > 80 {
                self.schedule_immediate(task).await?;
            } else {
                self.schedule_background(task).await?;
            }
        }

        Ok(())
    }

    fn calculate_extraction_priority(
        &self,
        identification: &IdentificationResult,
        entry: &Entry,
        config: &ExtractionConfig
    ) -> u8 {
        let mut priority = config.priority;

        // Boost priority for recently accessed files
        if entry.last_accessed_within(Duration::from_days(7)) {
            priority = (priority + 10).min(100);
        }

        // Boost priority for files in active directories
        if self.is_active_directory(&entry.parent_path()) {
            priority = (priority + 15).min(100);
        }

        // Lower priority for very large files
        if entry.size > 100_000_000 { // 100MB
            priority = priority.saturating_sub(20);
        }

        // High priority for text/code files (fast to process)
        match identification.file_type.category {
            ContentKind::Text | ContentKind::Code => priority + 5,
            ContentKind::Image if entry.size < 10_000_000 => priority, // 10MB
            ContentKind::Document if entry.size < 50_000_000 => priority, // 50MB
            _ => priority.saturating_sub(10),
        }
    }
}
```

### 5. Unified Search Orchestrator & The Progressive Search Lifecycle

The `LightningSearchEngine` acts as a lightweight orchestrator. Its primary role is to dispatch a `SearchJob` that follows a **Progressive Enhancement Lifecycle**. This model ensures users receive instant results which are then intelligently refined in the background.

A single user query triggers a multi-stage job that can progress through several power levels, emitting updates as more relevant results are found.

#### The `SearchMode` Enum

The `SearchMode` now represents the internal power level or stage of a search.

```rust
pub enum SearchMode {
    /// Fast, metadata-only FTS5 search on filenames and extensions.
    Fast,
    /// Adds VSS-based semantic re-ranking to the fast results.
    Normal,
    /// A comprehensive search that may include more expensive operations
    /// like on-demand content analysis or expanded candidate sets.
    Full,
}
```

#### The Phased Search Lifecycle

1.  **Dispatch:** A `SearchRequest` is received. The `LightningSearchEngine` creates and dispatches a `SearchJob`.

2.  **Phase 1: `Fast` Search (Instant Results)**

    - The job immediately runs the `Fast` search (FTS5 on metadata).
    - Within ~50ms, the initial results are cached and a `SearchResultsReady(result_id)` event is sent to the UI.
    - **The user sees instant results for any matching filenames.**

3.  **Phase 2: `Normal` Search (Background Enhancement)**

    - After Phase 1 completes, the job analyzes the query and initial results.
    - If the query appears semantic or the `Fast` results are ambiguous, the job automatically promotes itself to the `Normal` stage.
    - It re-ranks the results using VSS embedding sidecars.
    - When complete, it **updates the existing cached result set** for the same `result_id` and sends a `SearchResultsUpdated(result_id)` event.
    - **The UI seamlessly re-sorts the results list, bringing more relevant files to the top.**

4.  **Phase 3: `Full` Search (Optional Deep Dive)**
    - This phase can be triggered by explicit user action (e.g., a "search deeper" button) or by an AI agent.
    - It may perform more expensive operations, like expanding the candidate pool for semantic search.
    - Like Phase 2, it updates the cached results when complete.

### 6. Search Result Caching: A Device-Local Filesystem Approach

To ensure a fast experience and avoid re-computing searches, Spacedrive uses a scalable, device-local caching strategy. The cache is ephemeral and is **never synced between devices**.

This solution has three components:

1.  **Cache Directory (Non-Syncing)**

    - The cache lives outside the portable `.sdlibrary` directory, in a standard system cache location, ensuring it is never synced or backed up.
    - Example: `~/.cache/spacedrive/libraries/{library_id}/search/`

2.  **Result Files (Binary)**

    - The ordered list of `entry_id`s for a search is stored in a compact binary file (e.g., a raw array of `i64`s).
    - The filename is the unique `query_hash` of the search request (e.g., `.../search/a1b2c3d4.../results.bin`).
    - This scales to millions of results and allows for extremely efficient pagination by seeking to the required offset in the file without loading the entire list into memory.

3.  **Cache Index (Local Database)**
    - A tiny, separate SQLite database (`cache_index.db`) is kept in the cache directory to manage the result files.
    - This database is also local and never synced. It contains a single table to provide fast lookups for cached results.
    - **Schema:**
      ```sql
      -- In: cache_index.db
      CREATE TABLE cached_searches (
          query_hash TEXT PRIMARY KEY,
          result_count INTEGER NOT NULL,
          created_at TEXT NOT NULL DEFAULT (datetime('now')),
          expires_at TEXT NOT NULL
      );
      ```

This architecture strictly separates durable, syncable library data from ephemeral, device-local cache data, providing a robust and scalable caching solution.

## Virtual Sidecar File System (Core)

### Concept Overview

The Virtual Sidecar File System is the source of truth for derived intelligence. It maintains atomic links to files inline with the filesystem and enables automatic, transparent embedding generation and search capabilities. Search consumes VSS-managed artifacts directly.

### Architecture Design

```rust
pub struct VirtualSidecarSystem {
    sidecar_manager: SidecarManager,
    atomic_linker: AtomicLinker,
    embedding_scheduler: EmbeddingScheduler,
    filesystem_watcher: FilesystemWatcher,
}

// Virtual sidecar structure
pub struct VirtualSidecar {
    pub file_path: PathBuf,
    pub spacedrive_metadata: SpacedriveMetadata,
    pub embeddings: HashMap<String, EmbeddingSidecar>, // key: model_name
    pub content_analysis: Option<ContentAnalysis>,
    pub user_annotations: UserAnnotations,
    pub sync_status: SyncStatus,
    pub last_updated: SystemTime,
}

pub struct EmbeddingSidecar {
    pub model_name: String,
    pub embedding_hash: String,
    pub vector_len: usize,
    pub sidecar_path: PathBuf, // .sdlibrary/sidecars/content/{content_uuid}/embeddings/{model}.json
    pub created_at: SystemTime,
}
```

### Automatic Embedding Workflow

```rust
impl VirtualSidecarSystem {
    async fn on_file_change(&self, file_path: &Path, change_type: ChangeType) -> Result<()> {
        match change_type {
            ChangeType::Created | ChangeType::Modified => {
                // Update sidecar metadata
                self.sidecar_manager.update_metadata(file_path).await?;

                // Schedule embedding generation based on file type and priority
                if self.should_generate_embedding(file_path) {
                    self.embedding_scheduler.schedule_embedding(
                        file_path.to_path_buf(),
                        self.determine_priority(file_path)
                    ).await?;
                }
            },
            ChangeType::Deleted => {
                // Clean up sidecar and vector embeddings
                self.cleanup_file_references(file_path).await?;
            },
            ChangeType::Moved(old_path, new_path) => {
                // Update sidecar location and references
                self.move_sidecar_references(old_path, new_path).await?;
            }
        }

        Ok(())
    }

    fn should_generate_embedding(&self, file_path: &Path) -> bool {
        // Smart decisions based on file type, size, and user patterns
        let extension = file_path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        match extension {
            // Always embed text content
            "txt" | "md" | "rst" | "doc" | "docx" | "pdf" => true,

            // Embed code files in active projects
            "rs" | "js" | "py" | "cpp" | "java" => {
                self.is_active_project_file(file_path)
            },

            // Embed images if they're in photo directories
            "jpg" | "jpeg" | "png" | "webp" => {
                self.is_photo_directory(file_path.parent().unwrap())
            },

            // Skip large binaries and system files
            _ => false
        }
    }
}
```

### Transparent Search Integration

With the sidecar system, search becomes completely transparent:

```rust
impl LightningSearchEngine {
    async fn search_with_sidecar(&self, query: SearchQuery) -> Result<SearchResults> {
        // Step 1: Temporal search as usual
        let temporal_results = self.temporal_engine.search(&query).await?;

        // Step 2: Automatic semantic enhancement via sidecar embeddings
        let enhanced_results = self.enhance_with_sidecar_embeddings(
            temporal_results,
            &query
        ).await?;

        Ok(enhanced_results)
    }

    async fn enhance_with_sidecar_embeddings(
        &self,
        temporal_results: SearchResults,
        query: &SearchQuery
    ) -> Result<SearchResults> {
        let mut enhanced_entries = Vec::new();

        for entry in temporal_results.entries {
            // Check if this entry has vector embeddings available
            if let Some(sidecar) = self.sidecar_system.get_sidecar(&entry.full_path()).await? {
                if let Some(emb) = sidecar.embeddings.get("all-MiniLM-L6-v2") {
                    let semantic_score = self.semantic_engine
                        .rerank_with_embeddings(&query.text, &[entry.id], &emb.model_name)
                        .await?
                        .into_iter()
                        .next()
                        .map(|r| r.score)
                        .unwrap_or(0.0);

                    enhanced_entries.push(SearchResultEntry {
                        temporal_score: entry.temporal_score,
                        semantic_score: Some(semantic_score),
                        combined_score: self.compute_combined_score(
                            entry.temporal_score,
                            semantic_score
                        ),
                        ..entry
                    });
                } else {
                    // No embeddings available, use temporal score only
                    enhanced_entries.push(entry);
                }
            }
        }

        // Re-sort by combined score
        enhanced_entries.sort_by(|a, b|
            b.combined_score.partial_cmp(&a.combined_score).unwrap()
        );

        Ok(SearchResults {
            entries: enhanced_entries,
            ..temporal_results
        })
    }
}
```

## Performance Architecture

### VSS Embedding Storage and Access

```rust
// Deterministic on-disk layout owned by VSS
// .sdlibrary/sidecars/content/{content_uuid}/embeddings/{model_name}.json

#[derive(Serialize, Deserialize)]
pub struct EmbeddingFileV1 {
    pub model_name: String,
    pub model_version: String,
    pub vector: Vec<f32>,
    pub vector_len: usize,
    pub embedding_hash: String,
    pub content_hash: String,
    pub created_at: SystemTime,
}

pub struct SidecarRepository {
    root: PathBuf,
}

impl SidecarRepository {
    pub async fn find_embedding_sidecar(&self, entry_id: i32, model_name: &str)
        -> Result<Option<(Uuid, PathBuf)>> { /* lookup via sidecars table */ }

    pub async fn read_embedding_vector(&self, path: &Path)
        -> Result<Option<Vec<f32>>> { /* mmap or buffered read */ }
}
```

### Adaptive Performance Management

```rust
pub struct AdaptivePerformanceManager {
    system_monitor: SystemResourceMonitor,
    search_analytics: SearchAnalytics,
    embedding_scheduler: EmbeddingScheduler,
    cache_manager: CacheManager,
}

impl AdaptivePerformanceManager {
    async fn optimize_search_strategy(&self, query: &SearchQuery) -> SearchStrategy {
        let system_load = self.system_monitor.current_load();
        let query_complexity = self.analyze_query_complexity(query);

        match (system_load, query_complexity) {
            (SystemLoad::Low, QueryComplexity::Simple) => SearchStrategy::FastTrack {
                use_temporal_only: true,
                cache_ttl: Duration::from_secs(300),
            },

            (SystemLoad::Low, QueryComplexity::Complex) => SearchStrategy::Comprehensive {
                use_vector_search: true,
                max_vector_candidates: 1000,
                enable_faceted_search: true,
            },

            (SystemLoad::High, _) => SearchStrategy::Conservative {
                use_temporal_only: true,
                limit_results: 50,
                skip_faceted_search: true,
            },

            (SystemLoad::Medium, QueryComplexity::Semantic) => SearchStrategy::Balanced {
                use_vector_search: true,
                max_vector_candidates: 500,
                enable_result_caching: true,
            },
        }
    }

    async fn schedule_background_embedding(&self, entry: &Entry) -> Result<()> {
        let priority = self.calculate_embedding_priority(entry);
        let system_resources = self.system_monitor.available_resources();

        if system_resources.cpu_available > 0.3 && system_resources.memory_available > 0.5 {
            self.embedding_scheduler.schedule_immediate(entry.clone(), priority).await?;
        } else {
            self.embedding_scheduler.schedule_deferred(entry.clone(), priority).await?;
        }

        Ok(())
    }
}
```

## Search Modes and Query Types

### Search Mode Optimization

The system operates on three primary modes, which a search job can transition through to progressively enhance results.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchMode {
    /// Fast, metadata-only FTS5 search on filenames and extensions.
    Fast,
    /// Adds VSS-based semantic re-ranking to the fast results.
    Normal,
    /// A comprehensive search that may include more expensive operations
    /// like on-demand content analysis or expanded candidate sets.
    Full,
}
```

### Query Intelligence and Optimization

```rust
pub struct QueryIntelligence {
    nlp_processor: NlpProcessor,
    pattern_matcher: PatternMatcher,
    query_history: QueryHistoryAnalyzer,
}

impl QueryIntelligence {
    pub fn analyze_query(&self, query_text: &str) -> QueryAnalysis {
        let mut analysis = QueryAnalysis::default();

        // Detect query patterns
        analysis.query_type = self.detect_query_type(query_text);
        analysis.intent = self.extract_intent(query_text);
        analysis.entities = self.extract_entities(query_text);
        analysis.temporal_context = self.extract_temporal_context(query_text);

        // Suggest optimizations
        analysis.optimizations = self.suggest_optimizations(&analysis);

        analysis
    }

    fn detect_query_type(&self, query: &str) -> QueryType {
        // File extension search
        if query.ends_with(|c: char| c == '.' || c.is_alphanumeric()) &&
           query.contains('.') {
            return QueryType::FileExtension;
        }

        // Path-like search
        if query.contains('/') || query.contains('\\') {
            return QueryType::PathSearch;
        }

        // Natural language questions
        if query.starts_with("show me") || query.starts_with("find") ||
           query.contains('?') {
            return QueryType::NaturalLanguage;
        }

        // Content search indicators
        if query.len() > 20 || query.split_whitespace().count() > 3 {
            return QueryType::ContentSearch;
        }

        QueryType::SimpleFilename
    }

    fn extract_temporal_context(&self, query: &str) -> Option<TemporalContext> {
        let temporal_patterns = [
            (r"today", TemporalContext::Today),
            (r"yesterday", TemporalContext::Yesterday),
            (r"last week", TemporalContext::LastWeek),
            (r"last month", TemporalContext::LastMonth),
            (r"recent", TemporalContext::Recent),
            (r"old", TemporalContext::Old),
        ];

        for (pattern, context) in temporal_patterns {
            if query.to_lowercase().contains(pattern) {
                return Some(context);
            }
        }

        None
    }
}
```

## Database Schema Integration

### FTS5 Integration with VDFS

```sql
-- Enhanced FTS5 configuration for optimal performance
CREATE VIRTUAL TABLE search_index USING fts5(
    content='entries',
    content_rowid='id',
    name,
    extension,

    -- FTS5 configuration for optimal search
    tokenize="unicode61 remove_diacritics 2 tokenchars '.@-_'",

    -- Prefix indexing for autocomplete
    prefix='2,3'
);

-- Optimized triggers for real-time updates
CREATE TRIGGER IF NOT EXISTS entries_search_insert
AFTER INSERT ON entries WHEN new.kind = 0  -- Only files
BEGIN
    INSERT INTO search_index(rowid, name, extension)
    VALUES (new.id, new.name, new.extension);
END;

-- Search analytics for query optimization
CREATE TABLE search_analytics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    query_text TEXT NOT NULL,
    query_hash TEXT NOT NULL,
    search_mode TEXT NOT NULL,
    execution_time_ms INTEGER NOT NULL,
    result_count INTEGER NOT NULL,
    vector_search_used BOOLEAN DEFAULT FALSE,
    user_clicked_result BOOLEAN DEFAULT FALSE,
    clicked_result_position INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Note: The schema for the Virtual Sidecar System (`sidecars`, `sidecar_availability`)
-- is defined in the VSS design document and is the source of truth.
-- The schema for search result caching (`cached_searches`) is defined in a separate,
-- non-synced, device-local database.
```

### Optimized Indexes for Search Performance

```sql
-- Critical indexes for search performance
CREATE INDEX IF NOT EXISTS idx_entries_search_composite
ON entries(location_id, kind, extension, modified_at DESC);

CREATE INDEX IF NOT EXISTS idx_entries_size_range
ON entries(size) WHERE kind = 0;

CREATE INDEX IF NOT EXISTS idx_entries_recent
ON entries(modified_at DESC) WHERE kind = 0;

CREATE INDEX IF NOT EXISTS idx_user_metadata_search
ON user_metadata(favorite, hidden);

-- Specialized indexes for common search patterns
CREATE INDEX IF NOT EXISTS idx_entries_media_files
ON entries(extension, size DESC)
WHERE extension IN ('jpg', 'jpeg', 'png', 'mp4', 'mov', 'avi');

CREATE INDEX IF NOT EXISTS idx_entries_documents
ON entries(extension, modified_at DESC)
WHERE extension IN ('pdf', 'doc', 'docx', 'txt', 'md');

-- Path filtering is handled via the closure table; prefer joins over storing path in FTS
CREATE INDEX IF NOT EXISTS idx_entries_code_files
ON entries(extension)
WHERE extension IN ('rs', 'js', 'py', 'cpp', 'java', 'go');
```

## API Design

### GraphQL Integration

```graphql
extend type Query {
	"""
	Primary search endpoint with full Lightning Search capabilities
	"""
	search(
		query: String!
		mode: SearchMode = BALANCED
		filters: [SearchFilterInput!] = []
		sort: SortOptionsInput
		pagination: PaginationInput
		facets: FacetOptionsInput
	): SearchResponse!

	"""
	Fast autocomplete suggestions
	"""
	searchSuggestions(
		partial: String!
		limit: Int = 10
		context: SearchContextInput
	): [SearchSuggestion!]!

	"""
	Get available facets for a query
	"""
	searchFacets(
		query: String!
		filters: [SearchFilterInput!] = []
	): SearchFacetsResponse!

	"""
	Search analytics and insights
	"""
	searchAnalytics(
		timeRange: TimeRangeInput
		groupBy: AnalyticsGroupBy
	): SearchAnalyticsResponse!
}

enum SearchMode {
	FAST
	NORMAL
	FULL
}

type SearchResponse {
	entries: [SearchResultEntry!]!
	facets: [SearchFacet!]!
	suggestions: [SearchSuggestion!]!
	analytics: SearchAnalytics!
	pagination: PaginationInfo!
	searchId: UUID!
}

type SearchResultEntry {
	entry: Entry!
	score: Float!
	scoreBreakdown: ScoreBreakdown!
	highlights: [TextHighlight!]!
	context: SearchResultContext!
}

type ScoreBreakdown {
	temporalScore: Float!
	semanticScore: Float
	metadataScore: Float!
	recencyBoost: Float!
	userPreferenceBoost: Float!
	finalScore: Float!
}
```

## Implementation Roadmap

### Phase 1: VSS & Temporal Search Foundation (Weeks 1-3)

**VSS Core**

- [ ] Implement `sidecars` and `sidecar_availability` tables
- [ ] Implement deterministic filesystem layout under `.sdlibrary/sidecars/...`

**Temporal Search Engine**

- [ ] FTS5 integration with existing VDFS schema
- [ ] Real-time indexing triggers
- [ ] Basic search API infrastructure
- [ ] Query optimization and caching
- [ ] File type system integration
- [ ] Content extraction pipeline for text files

**File Type Integration**

- [ ] Extend file type definitions with extraction configurations
- [ ] Implement extraction method framework
- [ ] Add magic byte-based content identification
- [ ] Create type-aware extraction scheduling

**Deliverables:**

- Lightning-fast filename and content search (<10ms)
- Real-time index updates
- Basic REST and GraphQL APIs
- Type-aware content extraction for text, code, and documents
- Extensible file type system with extraction capabilities

### Phase 2: Embedding Generation (Weeks 4-6)

**VSS Embeddings**

- [ ] Integrate a lightweight embedding model (e.g., all-MiniLM-L6-v2)
- [ ] Implement `EmbeddingJob` producing VSS-compliant sidecars
- [ ] Write sidecar records to `sidecars` and `sidecar_availability`
- [ ] Hook job dispatch into the indexer Intelligence Queueing
- [ ] Background batching and throttling

**Advanced Extraction Engines**

- [ ] PDF text extraction with poppler/tesseract
- [ ] Image metadata extraction (EXIF, XMP)
- [ ] Audio metadata extraction (ID3, FLAC)
- [ ] Code structure extraction with tree-sitter
- [ ] Document structure extraction

**Deliverables:**

- Semantic reranking via VSS embeddings
- Automatic embedding generation and storage
- Portable, self-contained semantic artifacts per library
- Multi-modal extraction and metadata

### Phase 3: Semantic Search Integration (Weeks 7-9)

**Semantic Reranking**

- [ ] Update `SearchJob` to perform Stage 2 reranking using VSS embedding sidecars
- [ ] Model selection and fallbacks
- [ ] Result caching keyed by query + model + candidates

**Search Intelligence**

- [ ] Query analysis and optimization
- [ ] Faceted search implementation
- [ ] Search result personalization
- [ ] Advanced filtering and sorting
- [ ] Search analytics and learning

**Content-Aware Search Enhancement**

- [ ] File type-specific search strategies
- [ ] Context-aware extraction scheduling
- [ ] Intelligent thumbnail generation
- [ ] OCR integration for image text search
- [ ] Content similarity clustering

**Deliverables:**

- Intelligent query understanding
- Dynamic faceted search
- Personalized search results
- Comprehensive search analytics
- Context-aware content processing

### Phase 4: Virtual Sidecar System (Weeks 10-12)

**Sidecar Architecture**

- [ ] Virtual sidecar file system design
- [ ] Atomic file linking system
- [ ] Automatic embedding scheduling
- [ ] Transparent search integration (Search consumes VSS sidecars directly)
- [ ] Performance optimization

**Deliverables:**

- Transparent file-based search
- Automatic vector embedding generation
- Seamless filesystem integration
- Production-ready performance

## Performance Targets

### Search Performance Goals

| Search Mode   | Target Latency | Max Results | Use Case                 |
| ------------- | -------------- | ----------- | ------------------------ |
| Lightning     | <5ms           | 100         | Instant autocomplete     |
| Fast          | <25ms          | 200         | Quick file finding       |
| Balanced      | <100ms         | 500         | General purpose search   |
| Comprehensive | <500ms         | 1000        | Deep content discovery   |
| Semantic      | <2000ms        | 1000        | Research and exploration |

### Scalability Targets

- **Files Indexed**: 10M+ files per library
- **Concurrent Users**: 100+ simultaneous searches
- **Index Size**: <20% of original data size
- **Memory Usage**: <1GB RAM for 1M files
- **Disk Usage**: <500MB vector embeddings for 100K files

### Quality Metrics

- **Relevance**: >95% top-3 accuracy for filename searches
- **Semantic Accuracy**: >85% relevance for content searches
- **Freshness**: <100ms index update latency
- **Availability**: 99.9% search uptime

## Security and Privacy

### Data Protection

- **Local Processing**: All embeddings generated locally
- **No Cloud Dependencies**: Complete offline operation
- **Encryption**: Vector embeddings encrypted at rest
- **Access Control**: Search respects file permissions

### Privacy Guarantees

- **No Telemetry**: Search queries never leave the device
- **Content Privacy**: File content never transmitted
- **Metadata Protection**: User annotations remain local
- **Audit Trail**: Optional search analytics for performance

## Monitoring and Observability

### Search Analytics

```rust
pub struct SearchAnalytics {
    pub query_patterns: QueryPatternAnalyzer,
    pub performance_metrics: PerformanceTracker,
    pub user_behavior: UserBehaviorAnalyzer,
    pub system_health: SystemHealthMonitor,
}

pub struct QueryPatternAnalyzer {
    pub popular_queries: TopQueries,
    pub query_types: HashMap<QueryType, u64>,
    pub temporal_patterns: TemporalUsagePattern,
    pub failure_patterns: Vec<FailedQuery>,
}

pub struct PerformanceTracker {
    pub avg_query_time: Duration,
    pub p95_query_time: Duration,
    pub cache_hit_rate: f32,
    pub vector_search_utilization: f32,
    pub index_update_latency: Duration,
}
```

### Health Checks

```rust
impl LightningSearchEngine {
    pub async fn health_check(&self) -> SearchHealthStatus {
        SearchHealthStatus {
            fts5_status: self.temporal_engine.health_check().await,
            vss_status: self.semantic_engine.health_check().await,
            cache_status: self.cache_manager.health_check().await,
            index_freshness: self.check_index_freshness().await,
            performance_status: self.check_performance_status().await,
        }
    }
}
```

## Updated Search Workflow

1.  **Indexing & Intelligence Queueing**

    - A file is discovered and its `content_uuid` is determined.
    - The indexer dispatches jobs to generate sidecars: `OcrJob`, `TextExtractionJob`, `EmbeddingJob`, etc.

2.  **Sidecar Generation**

    - Jobs run asynchronously, creating text and embedding sidecars and populating the necessary database tables (`sidecars`, `sidecar_availability`).

3.  **Progressive Search Execution (`SearchJob`)**
    - The job starts with a `Fast` search (FTS5) and immediately returns results to the UI.
    - It then automatically enhances the results in the background by progressing to a `Normal` search (semantic re-ranking), issuing updates to the UI as better results are found.
    - An optional `Full` search can be triggered for the most comprehensive results.
    - All results are managed in the device-local cache.

## Conclusion

Lightning Search represents a paradigm shift in file discovery technology, combining the speed of traditional search with the intelligence of modern AI. By leveraging Spacedrive's unique VDFS architecture and implementing a temporal-first, vector-enhanced approach, we create a search experience that is both lightning-fast and remarkably intelligent.

The virtual sidecar file system provides a path toward even more seamless integration, where search becomes an invisible, automatic capability that enhances every aspect of file management. This design positions Spacedrive as the most advanced file management platform available, with search capabilities that surpass even dedicated search engines.

The implementation roadmap provides a clear path from basic temporal search to advanced semantic understanding, ensuring that each phase delivers immediate value while building toward the ultimate vision of transparent, intelligent file discovery.
