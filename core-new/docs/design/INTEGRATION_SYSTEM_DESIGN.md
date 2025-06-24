# Integration System Design

## Overview

The Spacedrive Integration System enables third-party extensions to seamlessly integrate with Spacedrive's core functionality. The system supports cloud storage providers, custom file type handlers, search extensions, and content processors while maintaining security, performance, and reliability.

## Design Principles

### 1. Process Isolation
- Each integration runs as a separate process
- Core system remains stable if integrations crash
- Resource usage can be monitored and limited per integration
- Security boundaries prevent cross-integration data access

### 2. Language Agnostic
- Integrations can be written in any language
- Communication via standard protocols (IPC, HTTP, WebSocket)
- No dependency on Rust runtime or specific frameworks

### 3. Leverage Existing Architecture
- Build on proven patterns from job system, location manager, file type registry
- Reuse event bus for loose coupling
- Extend existing credential management via device manager

### 4. Zero-Configuration Discovery
- Automatic integration discovery and registration
- Schema-driven configuration validation
- Runtime capability negotiation

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                    Spacedrive Core                      │
│  ┌─────────────────┐  ┌──────────────────────────────┐  │
│  │ Integration     │  │         Core Systems         │  │
│  │ Manager         │  │  • Location Manager          │  │
│  │                 │  │  • Job System               │  │
│  │ • Registry      │  │  • File Type Registry       │  │
│  │ • Lifecycle     │  │  • Event Bus                │  │
│  │ • IPC Router    │  │  • Device Manager           │  │
│  │ • Sandbox       │  └──────────────────────────────┘  │
│  └─────────────────┘                                    │
└─────────────────────────────────────────────────────────┘
                              │
                ┌─────────────┼─────────────┐
                │             │             │
        ┌───────▼──────┐ ┌───▼────┐ ┌──────▼──────┐
        │ Cloud Storage│ │ Custom │ │   Search    │
        │ Integration  │ │ File   │ │ Integration │
        │              │ │ Types  │ │             │
        │ (Process)    │ │(Process│ │ (Process)   │
        └──────────────┘ └────────┘ └─────────────┘
```

## Core Components

### 1. Integration Manager

Central orchestrator managing integration lifecycle:

```rust
pub struct IntegrationManager {
    registry: Arc<IntegrationRegistry>,
    processes: Arc<RwLock<HashMap<String, IntegrationProcess>>>,
    ipc_router: Arc<IpcRouter>,
    credential_manager: Arc<CredentialManager>,
    event_bus: Arc<EventBus>,
    config: IntegrationConfig,
}

impl IntegrationManager {
    /// Discover and register all available integrations
    pub async fn discover_integrations(&self) -> Result<Vec<String>>;
    
    /// Start an integration process
    pub async fn start_integration(&self, id: &str) -> Result<()>;
    
    /// Stop an integration process
    pub async fn stop_integration(&self, id: &str) -> Result<()>;
    
    /// Route request to integration
    pub async fn handle_request(&self, request: IntegrationRequest) -> Result<IntegrationResponse>;
}
```

### 2. Integration Registry

Auto-discovery system for integration metadata:

```rust
inventory::collect!(IntegrationRegistration);

pub struct IntegrationRegistry {
    integrations: HashMap<String, IntegrationManifest>,
}

#[derive(Serialize, Deserialize)]
pub struct IntegrationManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub capabilities: Vec<IntegrationCapability>,
    pub executable_path: PathBuf,
    pub config_schema: JsonValue,
    pub permissions: IntegrationPermissions,
    pub author: String,
    pub homepage: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub enum IntegrationCapability {
    LocationProvider {
        supported_protocols: Vec<String>,
        auth_methods: Vec<AuthMethod>,
    },
    FileTypeHandler {
        extensions: Vec<String>,
        mime_types: Vec<String>,
        processing_modes: Vec<ProcessingMode>,
    },
    ContentProcessor {
        input_types: Vec<String>,
        output_formats: Vec<String>,
    },
    SearchProvider {
        query_languages: Vec<String>,
        result_types: Vec<String>,
    },
    ThumbnailGenerator {
        supported_formats: Vec<String>,
        output_formats: Vec<String>,
    },
}

#[derive(Serialize, Deserialize)]
pub struct IntegrationPermissions {
    pub network_access: Vec<String>,        // Allowed domains
    pub file_system_access: Vec<PathBuf>,   // Allowed paths
    pub max_memory_mb: u64,
    pub max_cpu_percent: u8,
    pub requires_credentials: bool,
}
```

### 3. IPC Communication System

High-performance communication layer:

```rust
pub struct IpcRouter {
    channels: HashMap<String, IpcChannel>,
    request_handlers: HashMap<String, Box<dyn RequestHandler>>,
}

#[derive(Serialize, Deserialize)]
pub struct IntegrationRequest {
    pub id: String,
    pub integration_id: String,
    pub method: String,
    pub params: JsonValue,
    pub timeout_ms: Option<u64>,
}

#[derive(Serialize, Deserialize)]
pub struct IntegrationResponse {
    pub request_id: String,
    pub success: bool,
    pub data: Option<JsonValue>,
    pub error: Option<IntegrationError>,
}

pub enum IpcChannel {
    UnixSocket(UnixStream),
    NamedPipe(NamedPipeClient),
    Tcp(TcpStream),
}
```

### 4. Credential Management

Secure credential storage leveraging existing device manager:

```rust
pub struct CredentialManager {
    device_manager: Arc<DeviceManager>,
    encrypted_store: EncryptedCredentialStore,
}

#[derive(Serialize, Deserialize)]
pub struct IntegrationCredential {
    pub integration_id: String,
    pub credential_type: CredentialType,
    pub data: EncryptedData,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Deserialize)]
pub enum CredentialType {
    OAuth2 {
        access_token: String,
        refresh_token: Option<String>,
        scopes: Vec<String>,
    },
    ApiKey {
        key: String,
        header_name: Option<String>,
    },
    Basic {
        username: String,
        password: String,
    },
    Custom(JsonValue),
}

impl CredentialManager {
    /// Store encrypted credential using device master key
    pub async fn store_credential(&self, integration_id: &str, credential: IntegrationCredential) -> Result<String>;
    
    /// Retrieve and decrypt credential
    pub async fn get_credential(&self, integration_id: &str, credential_id: &str) -> Result<IntegrationCredential>;
    
    /// Refresh OAuth2 tokens
    pub async fn refresh_oauth2_token(&self, credential_id: &str) -> Result<()>;
}
```

## Integration Types

### 1. Cloud Storage Provider

Extends location system for cloud storage mounting:

```rust
#[async_trait]
pub trait CloudStorageProvider {
    /// List available cloud locations for user
    async fn list_locations(&self, credentials: &IntegrationCredential) -> Result<Vec<CloudLocation>>;
    
    /// Create new location in cloud storage
    async fn create_location(&self, path: &str, credentials: &IntegrationCredential) -> Result<CloudLocation>;
    
    /// Sync local location with cloud
    async fn sync_location(&self, location: &CloudLocation, direction: SyncDirection) -> Result<SyncResult>;
    
    /// Watch for changes in cloud location
    async fn watch_location(&self, location: &CloudLocation) -> Result<ChangeStream>;
    
    /// Download file from cloud
    async fn download_file(&self, cloud_path: &str, local_path: &Path) -> Result<()>;
    
    /// Upload file to cloud
    async fn upload_file(&self, local_path: &Path, cloud_path: &str) -> Result<()>;
}

#[derive(Serialize, Deserialize)]
pub struct CloudLocation {
    pub id: String,
    pub name: String,
    pub path: String,
    pub total_space: Option<u64>,
    pub used_space: Option<u64>,
    pub device_id: Uuid,  // Virtual device ID for cloud
    pub last_sync: Option<DateTime<Utc>>,
}
```

### 2. File Type Handler

Extends file type registry with custom types:

```rust
#[async_trait]
pub trait FileTypeHandler {
    /// Get supported file extensions
    fn supported_extensions(&self) -> Vec<String>;
    
    /// Get supported MIME types
    fn supported_mime_types(&self) -> Vec<String>;
    
    /// Extract metadata from file
    async fn extract_metadata(&self, path: &Path) -> Result<FileMetadata>;
    
    /// Generate thumbnail for file
    async fn generate_thumbnail(&self, path: &Path, size: ThumbnailSize) -> Result<Vec<u8>>;
    
    /// Validate file integrity
    async fn validate_file(&self, path: &Path) -> Result<ValidationResult>;
}

// Integration with existing FileTypeRegistry
impl FileTypeRegistry {
    pub async fn register_integration_types(&mut self, integration_id: &str) -> Result<()> {
        let integration = IntegrationManager::get(integration_id).await?;
        
        if let Some(handler) = integration.as_file_type_handler() {
            for ext in handler.supported_extensions() {
                let file_type = FileType {
                    id: format!("{}:{}", integration_id, ext),
                    name: format!("{} File", ext.to_uppercase()),
                    extensions: vec![ext],
                    // ... other fields from integration
                    category: ContentKind::Custom,
                    metadata: json!({"integration_id": integration_id}),
                };
                
                self.register(file_type)?;
            }
        }
        
        Ok(())
    }
}
```

### 3. Search Provider

Extends search capabilities:

```rust
#[async_trait]
pub trait SearchProvider {
    /// Perform search query
    async fn search(&self, query: &SearchQuery, context: &SearchContext) -> Result<SearchResults>;
    
    /// Index content for search
    async fn index_content(&self, content: &ContentItem) -> Result<()>;
    
    /// Get search suggestions
    async fn get_suggestions(&self, partial_query: &str) -> Result<Vec<String>>;
}

#[derive(Serialize, Deserialize)]
pub struct SearchQuery {
    pub text: String,
    pub filters: HashMap<String, JsonValue>,
    pub sort_by: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Serialize, Deserialize)]
pub struct SearchContext {
    pub library_id: Uuid,
    pub location_ids: Option<Vec<Uuid>>,
    pub file_types: Option<Vec<String>>,
    pub date_range: Option<DateRange>,
}
```

## Job System Integration

Leverage existing job system for integration operations:

```rust
#[derive(Debug, Serialize, Deserialize, Job)]
pub struct IntegrationJob {
    pub integration_id: String,
    pub operation: IntegrationOperation,
    pub params: JsonValue,
    
    // State for resumability
    #[serde(skip)]
    pub progress: IntegrationProgress,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IntegrationOperation {
    CloudSync {
        location_id: Uuid,
        direction: SyncDirection,
    },
    ContentProcessing {
        file_paths: Vec<PathBuf>,
        processing_type: String,
    },
    SearchIndexing {
        content_batch: Vec<ContentItem>,
    },
    ThumbnailGeneration {
        file_paths: Vec<PathBuf>,
        sizes: Vec<ThumbnailSize>,
    },
}

impl Job for IntegrationJob {
    const NAME: &'static str = "integration_operation";
    const RESUMABLE: bool = true;
}

#[async_trait]
impl JobHandler for IntegrationJob {
    type Output = IntegrationJobOutput;
    
    async fn run(&mut self, ctx: JobContext<'_>) -> JobResult<Self::Output> {
        let integration = IntegrationManager::get(&self.integration_id).await?;
        
        match &self.operation {
            IntegrationOperation::CloudSync { location_id, direction } => {
                let provider = integration.as_cloud_provider()
                    .ok_or_else(|| JobError::ExecutionFailed("Not a cloud provider".into()))?;
                
                let location = ctx.library().get_location(*location_id).await?;
                let result = provider.sync_location(&location, *direction).await?;
                
                ctx.progress(Progress::structured(json!({
                    "files_synced": result.files_synced,
                    "bytes_transferred": result.bytes_transferred,
                    "operation": "cloud_sync"
                })));
                
                Ok(IntegrationJobOutput::CloudSync(result))
            }
            _ => todo!("Other operations")
        }
    }
}
```

## Location System Integration

Extend existing location system for cloud storage:

```rust
impl LocationManager {
    /// Add cloud storage location
    pub async fn add_cloud_location(
        &self,
        library: Arc<Library>,
        integration_id: &str,
        cloud_path: &str,
        name: Option<String>,
        credentials_id: &str,
    ) -> Result<(Uuid, Uuid)> {
        // Get integration
        let integration = IntegrationManager::get(integration_id).await?;
        let provider = integration.as_cloud_provider()
            .ok_or_else(|| LocationError::InvalidProvider)?;
        
        // Create cloud location
        let credentials = self.credential_manager.get_credential(integration_id, credentials_id).await?;
        let cloud_location = provider.create_location(cloud_path, &credentials).await?;
        
        // Create virtual device for cloud storage
        let virtual_device_id = self.device_manager.create_virtual_device(
            &format!("{}-{}", integration_id, cloud_location.id),
            &cloud_location.name,
        ).await?;
        
        // Create SdPath for cloud location
        let sd_path = SdPath::new(virtual_device_id, PathBuf::from(&cloud_location.path));
        
        // Add to location database
        let location_id = Uuid::new_v4();
        let location = ManagedLocation {
            id: location_id,
            name: name.unwrap_or(cloud_location.name),
            path: sd_path.path,
            device_id: virtual_device_id as i32,
            library_id: library.config.id,
            indexing_enabled: true,
            index_mode: IndexMode::Content,
            watch_enabled: true,
            integration_id: Some(integration_id.to_string()),
            cloud_location_id: Some(cloud_location.id),
        };
        
        // Save to database
        library.save_location(&location).await?;
        
        // Start initial sync job
        let sync_job = IntegrationJob {
            integration_id: integration_id.to_string(),
            operation: IntegrationOperation::CloudSync {
                location_id,
                direction: SyncDirection::Download,
            },
            params: json!({}),
            progress: IntegrationProgress::default(),
        };
        
        let job_id = library.jobs().dispatch(sync_job).await?;
        
        // Start file watching
        self.start_cloud_watching(&cloud_location, location_id).await?;
        
        Ok((location_id, job_id))
    }
    
    /// Start watching cloud location for changes
    async fn start_cloud_watching(&self, cloud_location: &CloudLocation, location_id: Uuid) -> Result<()> {
        // This would integrate with the existing location watcher service
        // to poll cloud storage for changes
        todo!("Implement cloud watching")
    }
}
```

## Security Model

### 1. Process Sandboxing

```rust
pub struct IntegrationSandbox {
    process_limits: ProcessLimits,
    file_system_jail: FileSystemJail,
    network_filter: NetworkFilter,
}

#[derive(Debug)]
pub struct ProcessLimits {
    pub max_memory_mb: u64,
    pub max_cpu_percent: u8,
    pub max_file_descriptors: u32,
    pub max_execution_time: Duration,
}

#[derive(Debug)]
pub struct FileSystemJail {
    pub allowed_read_paths: Vec<PathBuf>,
    pub allowed_write_paths: Vec<PathBuf>,
    pub temp_directory: PathBuf,
}

#[derive(Debug)]
pub struct NetworkFilter {
    pub allowed_domains: Vec<String>,
    pub allowed_ports: Vec<u16>,
    pub require_https: bool,
}
```

### 2. Permission System

```rust
#[derive(Serialize, Deserialize)]
pub struct IntegrationPermissions {
    pub file_system: FileSystemPermissions,
    pub network: NetworkPermissions,
    pub credentials: CredentialPermissions,
    pub core_apis: Vec<CoreApiPermission>,
}

#[derive(Serialize, Deserialize)]
pub enum CoreApiPermission {
    ReadLocations,
    WriteLocations,
    ReadFiles,
    WriteFiles,
    CreateJobs,
    AccessEvents,
    ManageCredentials,
}
```

## Installation & Distribution

### 1. Integration Package Format

```
integration-package.tar.gz
├── manifest.json          # Integration metadata
├── executable             # Main integration binary
├── config-schema.json     # Configuration schema
├── permissions.json       # Required permissions
├── assets/               # Icons, documentation
│   ├── icon.png
│   └── README.md
└── examples/             # Example configurations
    └── config.example.json
```

### 2. CLI Commands

```bash
# Install integration
spacedrive integration install ./google-drive-integration.tar.gz

# List available integrations
spacedrive integration list

# Enable integration with configuration
spacedrive integration enable google-drive --config ./config.json

# Disable integration
spacedrive integration disable google-drive

# Show integration status
spacedrive integration status google-drive

# Update integration
spacedrive integration update google-drive

# Remove integration
spacedrive integration remove google-drive
```

## Implementation Phases

### Phase 1: Foundation (3-4 weeks)
- [ ] Integration manager core structure
- [ ] IPC communication system
- [ ] Basic process lifecycle management
- [ ] Integration registry and discovery
- [ ] Credential management foundation

### Phase 2: Cloud Storage Integration (3-4 weeks)
- [ ] Cloud location provider interface
- [ ] Virtual device system for cloud storage
- [ ] SdPath extension for cloud paths
- [ ] Basic sync job implementation
- [ ] Cloud file watcher integration

### Phase 3: File Type Extensions (2-3 weeks)
- [ ] File type handler interface
- [ ] Custom file type loading
- [ ] Metadata extraction jobs
- [ ] Thumbnail generation hooks
- [ ] Integration with existing file type registry

### Phase 4: Advanced Features (3-4 weeks)
- [ ] Search provider integration
- [ ] Content processing jobs
- [ ] Performance optimization
- [ ] Security hardening
- [ ] Comprehensive testing

### Phase 5: Developer Experience (2-3 weeks)
- [ ] Integration SDK/template
- [ ] Documentation and examples
- [ ] CLI tooling improvements
- [ ] Integration marketplace preparation

## Example Integration: Google Drive

```rust
pub struct GoogleDriveIntegration {
    client: GoogleDriveClient,
    config: GoogleDriveConfig,
}

#[async_trait]
impl Integration for GoogleDriveIntegration {
    async fn initialize(&mut self, config: IntegrationConfig) -> IntegrationResult<()> {
        self.config = serde_json::from_value(config.params)?;
        self.client = GoogleDriveClient::new(&self.config.client_id, &self.config.client_secret);
        Ok(())
    }
    
    async fn register_capabilities(&self) -> Vec<IntegrationCapability> {
        vec![
            IntegrationCapability::LocationProvider {
                supported_protocols: vec!["gdrive".to_string()],
                auth_methods: vec![AuthMethod::OAuth2],
            }
        ]
    }
    
    async fn handle_request(&mut self, request: IntegrationRequest) -> IntegrationResult<IntegrationResponse> {
        match request.method.as_str() {
            "list_locations" => {
                let credentials: IntegrationCredential = serde_json::from_value(request.params)?;
                let locations = self.list_locations(&credentials).await?;
                Ok(IntegrationResponse {
                    request_id: request.id,
                    success: true,
                    data: Some(serde_json::to_value(locations)?),
                    error: None,
                })
            }
            "sync_location" => {
                // Handle sync request
                todo!()
            }
            _ => Err(IntegrationError::UnknownMethod(request.method))
        }
    }
}

#[async_trait]
impl CloudStorageProvider for GoogleDriveIntegration {
    async fn list_locations(&self, credentials: &IntegrationCredential) -> Result<Vec<CloudLocation>> {
        let access_token = self.extract_oauth2_token(credentials)?;
        let drives = self.client.list_drives(&access_token).await?;
        
        Ok(drives.into_iter().map(|drive| CloudLocation {
            id: drive.id,
            name: drive.name,
            path: format!("gdrive:///{}", drive.id),
            total_space: drive.quota.total,
            used_space: drive.quota.used,
            device_id: Uuid::new_v4(), // Generated virtual device ID
            last_sync: None,
        }).collect())
    }
    
    // ... other methods
}
```

## Performance Considerations

### 1. Process Management
- **Lazy Loading**: Start integrations only when needed
- **Process Pooling**: Reuse processes for multiple operations
- **Resource Monitoring**: Track CPU, memory, network usage per integration
- **Graceful Degradation**: Continue core functionality if integrations fail

### 2. Communication Optimization
- **Batched Requests**: Group multiple operations into single IPC calls
- **Streaming**: Support streaming for large data transfers
- **Compression**: Compress large payloads
- **Caching**: Cache frequently accessed integration data

### 3. Storage Efficiency
- **Incremental Sync**: Only sync changed files
- **Deduplication**: Use existing CAS system for cloud files
- **Lazy Indexing**: Index cloud files on-demand
- **Metadata Caching**: Cache cloud metadata locally

## Error Handling & Monitoring

### 1. Error Categories
```rust
#[derive(Error, Debug)]
pub enum IntegrationError {
    #[error("Integration not found: {0}")]
    NotFound(String),
    
    #[error("Integration process crashed: {0}")]
    ProcessCrashed(String),
    
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    
    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}
```

### 2. Health Monitoring
- **Heartbeat System**: Regular health checks for integration processes
- **Performance Metrics**: Track response times, success rates, resource usage
- **Error Reporting**: Structured error logging with integration context
- **Automatic Recovery**: Restart failed integrations with exponential backoff

## Future Extensions

### 1. Plugin Marketplace
- **Discovery**: Browse and install integrations from marketplace
- **Reviews**: User ratings and feedback system
- **Updates**: Automatic update notifications and installation
- **Revenue Sharing**: Support for paid integrations

### 2. AI/ML Integrations
- **Content Analysis**: Image recognition, document classification
- **Smart Organization**: AI-powered file organization suggestions
- **Predictive Caching**: ML-based file access prediction
- **Natural Language Search**: Query files using natural language

### 3. Workflow Automation
- **Rule Engine**: Define automated workflows based on file events
- **Integration Chains**: Connect multiple integrations in workflows
- **Scheduling**: Time-based automation triggers
- **Conditional Logic**: Complex rule-based automation

This integration system provides a robust foundation for extending Spacedrive's capabilities while maintaining security, performance, and ease of development.