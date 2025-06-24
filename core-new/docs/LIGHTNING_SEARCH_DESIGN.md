# Lightning Search: Next-Generation File Discovery for Spacedrive

## Overview

Lightning Search is a revolutionary multi-modal file discovery system designed specifically for Spacedrive's VDFS (Virtual Distributed File System) architecture. The system combines blazing-fast temporal search with intelligent semantic understanding, delivering sub-100ms query responses across millions of files while maintaining complete user privacy through local processing.

## Architecture Philosophy

### Temporal-First, Vector-Enhanced Search

Lightning Search employs a sophisticated two-stage architecture:

1. **Temporal Engine** (SQLite FTS5) provides instant text-based discovery and acts as a high-performance filter
2. **Vector Engine** (Chroma-based) performs semantic analysis on temporal results for intelligent ranking and discovery

This approach ensures that vector search operations are performed only on pre-filtered, relevant datasets, dramatically improving performance while maintaining semantic intelligence.

```
User Query → Temporal Engine (FTS5) → Filtered Results → Vector Engine (Chroma) → Ranked Results
             ↑ <10ms                   ↑ 100-1000 items   ↑ +50ms        ↑ Final Results
```

## Core Components

### 1. Temporal Search Engine (SQLite FTS5)

The foundation layer providing instant text-based discovery integrated directly with the VDFS schema:

```sql
-- FTS5 Virtual Table integrated with entries
CREATE VIRTUAL TABLE search_index USING fts5(
    content='entries',
    content_rowid='id',
    name,
    extension, 
    relative_path,
    -- Computed searchable content for text files
    extracted_content,
    tokenize="unicode61 remove_diacritics 2 tokenchars '.@-'"
);

-- Real-time triggers for immediate index updates
CREATE TRIGGER entries_search_insert AFTER INSERT ON entries BEGIN
    INSERT INTO search_index(rowid, name, extension, relative_path) 
    VALUES (new.id, new.name, new.extension, new.relative_path);
END;

CREATE TRIGGER entries_search_update AFTER UPDATE ON entries BEGIN
    UPDATE search_index SET 
        name = new.name,
        extension = new.extension,
        relative_path = new.relative_path
    WHERE rowid = new.id;
END;
```

**Performance Characteristics:**
- **Query Speed**: <10ms for simple queries, <30ms for complex patterns
- **Index Size**: ~15% of total database size
- **Update Latency**: Real-time via triggers (<1ms)
- **Throughput**: >10,000 queries/second

### 2. Vector Content Engine (Chroma Integration)

Semantic search using lightweight embeddings stored in external vector database:

```rust
use chroma_rs::{ChromaClient, Collection, EmbeddingFunction};
use crate::file_type::FileTypeRegistry;

pub struct VectorSearchEngine {
    chroma_client: ChromaClient,
    collections: HashMap<ContentKind, Collection>,
    embedding_model: Arc<OnnxEmbeddingModel>,
    content_extractor: ContentExtractor,
    file_type_registry: Arc<FileTypeRegistry>,
}

impl VectorSearchEngine {
    async fn new(chroma_path: &Path) -> Result<Self> {
        let client = ChromaClient::new(&format!("file://{}", chroma_path.display()))?;
        
        let mut collections = HashMap::new();
        
        // Separate collections for different content types for optimized retrieval
        collections.insert(ContentKind::Document, 
            client.get_or_create_collection("documents", None).await?);
        collections.insert(ContentKind::Code, 
            client.get_or_create_collection("code", None).await?);
        collections.insert(ContentKind::Image, 
            client.get_or_create_collection("images", None).await?);
        collections.insert(ContentKind::Audio, 
            client.get_or_create_collection("audio", None).await?);
        collections.insert(ContentKind::Video, 
            client.get_or_create_collection("video", None).await?);
        
        let file_type_registry = Arc::new(FileTypeRegistry::new());
        
        Ok(Self {
            chroma_client: client,
            collections,
            embedding_model: Arc::new(OnnxEmbeddingModel::load("all-MiniLM-L6-v2")?),
            content_extractor: ContentExtractor::new(file_type_registry.clone()),
            file_type_registry,
        })
    }
    
    async fn search_semantic(&self, query: &str, pre_filtered_ids: &[i32]) -> Result<Vec<ScoredResult>> {
        let query_embedding = self.embedding_model.encode(query).await?;
        let mut all_results = Vec::new();
        
        // Search across relevant collections based on pre-filtered content
        for (content_type, collection) in &self.collections {
            let filtered_ids: Vec<String> = pre_filtered_ids.iter()
                .filter(|id| self.get_content_type(**id) == *content_type)
                .map(|id| id.to_string())
                .collect();
            
            if !filtered_ids.is_empty() {
                let results = collection.query()
                    .query_embeddings(vec![query_embedding.clone()])
                    .n_results(100)
                    .include(vec!["distances", "metadatas"])
                    .ids(filtered_ids)
                    .execute()
                    .await?;
                
                all_results.extend(self.process_chroma_results(results));
            }
        }
        
        // Sort by semantic similarity score
        all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        Ok(all_results)
    }
}
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
# core-new/src/file_type/definitions/documents.toml
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
# core-new/src/file_type/definitions/code.toml
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
# core-new/src/file_type/definitions/images.toml
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

### 5. Unified Search Orchestrator

The main search coordinator that manages the temporal-first, vector-enhanced workflow:

```rust
pub struct LightningSearchEngine {
    temporal_engine: TemporalSearchEngine,
    vector_engine: VectorSearchEngine,
    metadata_engine: MetadataSearchEngine,
    cache_manager: SearchCacheManager,
    query_optimizer: QueryOptimizer,
}

impl LightningSearchEngine {
    pub async fn search(&self, query: SearchQuery) -> Result<SearchResults> {
        let search_id = Uuid::new_v4();
        let start_time = Instant::now();
        
        // Step 1: Optimize query
        let optimized_query = self.query_optimizer.optimize(query);
        
        // Step 2: Check cache
        if let Some(cached_results) = self.cache_manager.get(&optimized_query).await {
            return Ok(cached_results);
        }
        
        // Step 3: Temporal search (fast filtering)
        let temporal_results = self.temporal_engine
            .search(&optimized_query)
            .await?;
        
        let mut final_results = temporal_results;
        
        // Step 4: Semantic enhancement (if enabled and beneficial)
        if self.should_use_semantic_search(&optimized_query, &final_results) {
            let pre_filtered_ids: Vec<i32> = final_results.entries
                .iter()
                .map(|e| e.id)
                .collect();
            
            let semantic_scores = self.vector_engine
                .search_semantic(&optimized_query.text, &pre_filtered_ids)
                .await?;
            
            // Merge temporal and semantic results
            final_results = self.merge_search_results(final_results, semantic_scores);
        }
        
        // Step 5: Apply metadata filters and final ranking
        final_results = self.metadata_engine
            .apply_filters_and_rank(final_results, &optimized_query)
            .await?;
        
        // Step 6: Cache results
        self.cache_manager.store(&optimized_query, &final_results).await;
        
        final_results.execution_time = start_time.elapsed();
        final_results.search_id = search_id;
        
        Ok(final_results)
    }
    
    fn should_use_semantic_search(&self, query: &SearchQuery, temporal_results: &SearchResults) -> bool {
        // Use semantic search when:
        // 1. Query appears to be semantic in nature (not just filename search)
        // 2. Temporal results are ambiguous (many results with similar relevance)
        // 3. User explicitly requested comprehensive search
        // 4. Query contains natural language patterns
        
        query.mode == SearchMode::Comprehensive ||
        query.mode == SearchMode::Semantic ||
        (temporal_results.entries.len() > 10 && 
         temporal_results.relevance_variance() < 0.2) ||
        self.query_optimizer.is_semantic_query(&query.text)
    }
}
```

## Virtual Sidecar File System (Future Enhancement)

### Concept Overview

The Virtual Sidecar File System represents the next evolution of Spacedrive's file management, where the index maintains atomic links to files inline with the filesystem. This system will enable automatic, transparent vector embedding generation and search capabilities.

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
    pub vector_embeddings: Option<EmbeddingReference>,
    pub content_analysis: Option<ContentAnalysis>,
    pub user_annotations: UserAnnotations,
    pub sync_status: SyncStatus,
    pub last_updated: SystemTime,
}

pub struct EmbeddingReference {
    pub chroma_collection: String,
    pub chroma_id: String,
    pub model_version: String,
    pub embedding_hash: String,
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
                if let Some(embedding_ref) = sidecar.vector_embeddings {
                    // Get semantic similarity score
                    let semantic_score = self.vector_engine
                        .get_similarity_score(&query.text, &embedding_ref)
                        .await?;
                    
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

### Chroma Vector Database Integration

```rust
// Chroma configuration optimized for Spacedrive
pub struct ChromaConfig {
    pub persist_directory: PathBuf,
    pub collection_metadata: HashMap<String, CollectionMetadata>,
    pub embedding_model: EmbeddingModelConfig,
    pub index_params: IndexParams,
}

impl ChromaConfig {
    pub fn spacedrive_optimized(data_dir: &Path) -> Self {
        Self {
            persist_directory: data_dir.join("chroma_db"),
            collection_metadata: hashmap! {
                "documents".to_string() => CollectionMetadata {
                    hnsw_space: "cosine",
                    embedding_dim: 384, // all-MiniLM-L6-v2
                    max_elements: 1_000_000,
                },
                "code".to_string() => CollectionMetadata {
                    hnsw_space: "cosine", 
                    embedding_dim: 384,
                    max_elements: 500_000,
                },
                "images".to_string() => CollectionMetadata {
                    hnsw_space: "cosine",
                    embedding_dim: 512, // CLIP embeddings
                    max_elements: 100_000,
                }
            },
            embedding_model: EmbeddingModelConfig {
                model_name: "all-MiniLM-L6-v2",
                batch_size: 32,
                max_sequence_length: 256,
            },
            index_params: IndexParams {
                ef_construction: 200,
                m: 16,
                max_m: 16,
                ml: 1.0 / 2.0_f32.ln(),
            }
        }
    }
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

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchMode {
    /// Ultra-fast filename and path search only
    Lightning,   // <5ms, FTS5 only
    
    /// Fast search with basic semantic enhancement
    Fast,        // <25ms, FTS5 + limited vector search
    
    /// Balanced speed and intelligence
    Balanced,    // <100ms, FTS5 + vector search + metadata
    
    /// Comprehensive semantic search
    Comprehensive, // <500ms, full vector search + facets
    
    /// Pure semantic/content search
    Semantic,    // variable, vector-first approach
}

impl SearchMode {
    pub fn execution_strategy(&self) -> ExecutionStrategy {
        match self {
            SearchMode::Lightning => ExecutionStrategy {
                use_fts5: true,
                use_vector_search: false,
                use_faceted_search: false,
                max_results: 100,
                timeout: Duration::from_millis(5),
            },
            
            SearchMode::Fast => ExecutionStrategy {
                use_fts5: true,
                use_vector_search: true,
                vector_candidate_limit: 50,
                use_faceted_search: false,
                max_results: 200,
                timeout: Duration::from_millis(25),
            },
            
            SearchMode::Balanced => ExecutionStrategy {
                use_fts5: true,
                use_vector_search: true,
                vector_candidate_limit: 500,
                use_faceted_search: true,
                max_results: 500,
                timeout: Duration::from_millis(100),
            },
            
            SearchMode::Comprehensive => ExecutionStrategy {
                use_fts5: true,
                use_vector_search: true,
                vector_candidate_limit: 2000,
                use_faceted_search: true,
                use_ml_ranking: true,
                max_results: 1000,
                timeout: Duration::from_millis(500),
            },
            
            SearchMode::Semantic => ExecutionStrategy {
                use_fts5: false, // Vector-first approach
                use_vector_search: true,
                vector_candidate_limit: 5000,
                use_semantic_expansion: true,
                use_faceted_search: true,
                max_results: 1000,
                timeout: Duration::from_secs(2),
            },
        }
    }
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
    relative_path,
    extracted_content,  -- Content extracted from files
    
    -- FTS5 configuration for optimal search
    tokenize="unicode61 remove_diacritics 2 tokenchars '.@-_'",
    
    -- Prefix indexing for autocomplete
    prefix='2,3'
);

-- Optimized triggers for real-time updates
CREATE TRIGGER IF NOT EXISTS entries_search_insert 
AFTER INSERT ON entries WHEN new.kind = 0  -- Only files
BEGIN
    INSERT INTO search_index(rowid, name, extension, relative_path, extracted_content) 
    VALUES (
        new.id, 
        new.name, 
        new.extension, 
        new.relative_path,
        CASE 
            WHEN new.extension IN ('txt', 'md', 'rs', 'js', 'py') 
            THEN (SELECT content FROM file_content_cache WHERE entry_id = new.id)
            ELSE ''
        END
    );
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

-- Content extraction cache
CREATE TABLE file_content_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entry_id INTEGER UNIQUE REFERENCES entries(id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    extracted_content TEXT,
    content_type TEXT NOT NULL,
    extraction_method TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Vector embedding metadata (references to Chroma)
CREATE TABLE embedding_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entry_id INTEGER UNIQUE REFERENCES entries(id) ON DELETE CASCADE,
    chroma_collection TEXT NOT NULL,
    chroma_id TEXT NOT NULL,
    model_version TEXT NOT NULL,
    embedding_hash TEXT NOT NULL,
    content_hash TEXT NOT NULL,  -- For invalidation
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    
    UNIQUE(chroma_collection, chroma_id)
);

-- Search facet cache for performance
CREATE TABLE search_facet_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    query_hash TEXT NOT NULL,
    facet_type TEXT NOT NULL,
    facet_data TEXT NOT NULL,  -- JSON
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT NOT NULL,
    
    INDEX idx_facet_cache_query_type (query_hash, facet_type),
    INDEX idx_facet_cache_expires (expires_at)
);
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

CREATE INDEX IF NOT EXISTS idx_entries_code_files 
ON entries(extension, relative_path) 
WHERE extension IN ('rs', 'js', 'py', 'cpp', 'java', 'go');
```

## API Design

### Unified Search API

```rust
// Main search request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub mode: SearchMode,
    pub filters: Vec<SearchFilter>,
    pub sort: SortOptions,
    pub pagination: PaginationOptions,
    pub facets: FacetOptions,
    pub context: SearchContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFilter {
    pub field: FilterField,
    pub operation: FilterOperation,
    pub value: FilterValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterField {
    FileType,
    Size,
    ModifiedDate,
    CreatedDate,
    Location,
    Tags,
    ContentType,
    Resolution,
    Duration,
    Favorite,
    Hidden,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterOperation {
    Equals,
    NotEquals,
    Contains,
    StartsWith,
    EndsWith,
    GreaterThan,
    LessThan,
    Between,
    In,
    NotIn,
}

// Rich search results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub entries: Vec<SearchResultEntry>,
    pub facets: HashMap<FilterField, Vec<FacetValue>>,
    pub suggestions: Vec<SearchSuggestion>,
    pub analytics: SearchAnalytics,
    pub pagination: PaginationInfo,
    pub search_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultEntry {
    pub entry: Entry,
    pub score: f32,
    pub score_breakdown: ScoreBreakdown,
    pub highlights: Vec<TextHighlight>,
    pub context: SearchResultContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub temporal_score: f32,
    pub semantic_score: Option<f32>,
    pub metadata_score: f32,
    pub recency_boost: f32,
    pub user_preference_boost: f32,
    pub final_score: f32,
}
```

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
    LIGHTNING
    FAST  
    BALANCED
    COMPREHENSIVE
    SEMANTIC
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

### Phase 1: Foundation (Weeks 1-3)
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

### Phase 2: Vector Intelligence (Weeks 4-6)
**Chroma Integration**
- [ ] Chroma vector database setup and configuration
- [ ] Embedding generation pipeline
- [ ] Content type-specific collections
- [ ] Vector search integration with temporal results
- [ ] Background embedding processing

**Advanced Extraction Engines**
- [ ] PDF text extraction with poppler/tesseract
- [ ] Image metadata extraction (EXIF, XMP)
- [ ] Audio metadata extraction (ID3, FLAC)
- [ ] Code structure extraction with tree-sitter
- [ ] Document structure extraction

**Deliverables:**
- Semantic search capabilities
- Automatic embedding generation
- Vector-enhanced result ranking
- Multi-modal content understanding
- Comprehensive metadata extraction across all file types

### Phase 3: Advanced Features (Weeks 7-9)
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
- [ ] Transparent search integration
- [ ] Performance optimization

**Deliverables:**
- Transparent file-based search
- Automatic vector embedding generation
- Seamless filesystem integration
- Production-ready performance

## Performance Targets

### Search Performance Goals

| Search Mode | Target Latency | Max Results | Use Case |
|-------------|---------------|-------------|----------|
| Lightning | <5ms | 100 | Instant autocomplete |
| Fast | <25ms | 200 | Quick file finding |
| Balanced | <100ms | 500 | General purpose search |
| Comprehensive | <500ms | 1000 | Deep content discovery |
| Semantic | <2000ms | 1000 | Research and exploration |

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
            chroma_status: self.vector_engine.health_check().await,
            cache_status: self.cache_manager.health_check().await,
            index_freshness: self.check_index_freshness().await,
            performance_status: self.check_performance_status().await,
        }
    }
}
```

## Conclusion

Lightning Search represents a paradigm shift in file discovery technology, combining the speed of traditional search with the intelligence of modern AI. By leveraging Spacedrive's unique VDFS architecture and implementing a temporal-first, vector-enhanced approach, we create a search experience that is both lightning-fast and remarkably intelligent.

The virtual sidecar file system provides a path toward even more seamless integration, where search becomes an invisible, automatic capability that enhances every aspect of file management. This design positions Spacedrive as the most advanced file management platform available, with search capabilities that surpass even dedicated search engines.

The implementation roadmap provides a clear path from basic temporal search to advanced semantic understanding, ensuring that each phase delivers immediate value while building toward the ultimate vision of transparent, intelligent file discovery.