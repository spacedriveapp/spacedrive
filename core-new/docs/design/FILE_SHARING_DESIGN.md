# Spacedrive File Sharing Design

## Overview

Spacedrive's file sharing system provides a unified, relationship-aware approach to transferring files between devices. The system automatically adapts its behavior based on the relationship between devices: fast, efficient syncing between paired devices, and secure, privacy-focused sharing with unknown devices.

## Design Principles

### 1. **Relationship-Aware Adaptation**
- Leverage existing device relationships when available
- Fall back to secure ephemeral sharing for unknown devices
- Single unified interface regardless of underlying protocol

### 2. **Protocol Agnostic Pairing**
- Device pairing establishes trust relationships only
- File sharing consumes but doesn't create device relationships
- Clean separation between trust establishment and data transfer

### 3. **Security Appropriate to Context**
- Paired devices: Fast, efficient, leverages established trust
- Unknown devices: Maximum security with ephemeral keys and explicit consent
- No security compromises for convenience

### 4. **Seamless User Experience**
- Users see one "Share" interface across all scenarios
- System chooses optimal method transparently
- Consistent progress and status reporting

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                   Application Layer                     │
│  Core.share_files(), UI file sharing, drag & drop      │
└─────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────┐
│                File Share Manager                       │
│  Orchestrates between persistent + ephemeral sharing   │
└─────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              │                               │
┌─────────────────────────┐    ┌─────────────────────────┐
│  Persistent File Sync   │    │   Ephemeral Spacedrop  │
│  (Paired devices)       │    │   (Unknown devices)     │
│  - Reuses session keys  │    │   - Perfect forward     │
│  - Auto-accept trusted  │    │     secrecy per file    │
│  - Resumable transfers  │    │   - Explicit consent    │
└─────────────────────────┘    └─────────────────────────┘
              │                               │
┌─────────────────────────┐    ┌─────────────────────────┐
│   Persistent Network    │    │   Spacedrop Discovery  │
│   (Device relationships)│    │   (mDNS, DHT, QR)      │
└─────────────────────────┘    └─────────────────────────┘
              │                               │
┌─────────────────────────────────────────────────────────┐
│                    LibP2P Foundation                   │
│        Shared networking, transports, encryption       │
└─────────────────────────────────────────────────────────┘
```

## Core Components

### 1. File Share Manager

Central orchestrator that determines the appropriate sharing mechanism:

```rust
/// Main file sharing service
pub struct FileShareManager {
    /// Persistent networking for paired devices
    persistent_networking: Arc<NetworkingService>,
    
    /// Ephemeral Spacedrop for unknown devices
    spacedrop_service: Arc<SpacedropService>,
    
    /// Device relationship resolver
    device_resolver: Arc<DeviceResolver>,
    
    /// Active transfer sessions
    active_sessions: Arc<RwLock<HashMap<Uuid, FileShareSession>>>,
    
    /// Transfer configuration
    config: FileShareConfig,
}

impl FileShareManager {
    /// Universal file sharing entry point
    pub async fn share_files(
        &self,
        request: FileShareRequest,
    ) -> Result<FileShareHandle> {
        // 1. Resolve device relationship
        let relationship = self.device_resolver
            .resolve_device_relationship(&request.target).await?;
        
        // 2. Choose appropriate sharing mechanism
        let session = match relationship {
            DeviceRelationship::Paired { device_id, trust_level } => {
                self.create_persistent_session(device_id, request, trust_level).await?
            }
            
            DeviceRelationship::Unknown { advertisement } => {
                self.create_ephemeral_session(advertisement, request).await?
            }
            
            DeviceRelationship::Blocked { reason } => {
                return Err(FileShareError::DeviceBlocked(reason));
            }
        };
        
        // 3. Track session and return handle
        let session_id = session.id();
        self.active_sessions.write().await.insert(session_id, session);
        
        Ok(FileShareHandle::new(session_id, self.clone()))
    }
}
```

### 2. Device Relationship Resolver

Determines the relationship between local device and target:

```rust
/// Resolves device relationships for file sharing decisions
pub struct DeviceResolver {
    persistent_identity: Arc<RwLock<PersistentNetworkIdentity>>,
    spacedrop_discovery: Arc<SpacedropDiscovery>,
    device_cache: Arc<RwLock<HashMap<DeviceFingerprint, CachedDevice>>>,
}

#[derive(Debug, Clone)]
pub enum DeviceRelationship {
    /// Device is paired and trusted
    Paired { 
        device_id: Uuid, 
        trust_level: TrustLevel,
        connection_status: ConnectionStatus,
        capabilities: DeviceCapabilities,
    },
    
    /// Device discovered but not paired
    Unknown { 
        advertisement: SpacedropAdvertisement,
        proximity: NetworkProximity,
        security_context: SecurityContext,
    },
    
    /// Device is explicitly blocked
    Blocked { 
        device_id: Uuid, 
        reason: String,
        blocked_at: DateTime<Utc>,
    },
}

impl DeviceResolver {
    /// Resolve device relationship from various target types
    pub async fn resolve_device_relationship(
        &self, 
        target: &ShareTarget
    ) -> Result<DeviceRelationship> {
        match target {
            ShareTarget::PairedDevice(device_id) => {
                self.resolve_paired_device(*device_id).await
            }
            
            ShareTarget::DiscoveredDevice(advertisement) => {
                // Check if discovered device is actually paired
                if let Some(paired_id) = self.find_paired_device_by_fingerprint(
                    &advertisement.network_fingerprint
                ).await? {
                    self.resolve_paired_device(paired_id).await
                } else {
                    self.resolve_unknown_device(advertisement.clone()).await
                }
            }
            
            ShareTarget::DeviceIdentifier(identifier) => {
                self.resolve_by_identifier(identifier).await
            }
        }
    }
}
```

### 3. File Share Sessions

Unified session abstraction that adapts to relationship type:

```rust
/// Universal file sharing session
pub enum FileShareSession {
    /// Using persistent connection (paired devices)
    Persistent {
        session_id: Uuid,
        device_id: Uuid,
        connection: Arc<DeviceConnection>,
        transfer_state: PersistentTransferState,
        config: PersistentShareConfig,
    },
    
    /// Using ephemeral Spacedrop (unknown devices)
    Ephemeral {
        session_id: Uuid,
        peer_id: PeerId,
        ephemeral_keys: EphemeralKeyPair,
        transfer_state: EphemeralTransferState,
        config: EphemeralShareConfig,
    },
}

impl FileShareSession {
    /// Send file data (adapts to session type)
    pub async fn send_chunk(&mut self, chunk: FileChunk) -> Result<ChunkReceipt> {
        match self {
            FileShareSession::Persistent { connection, device_id, .. } => {
                // Use persistent connection with existing session keys
                let message = DeviceMessage::FileChunk {
                    transfer_id: self.id(),
                    chunk_index: chunk.index,
                    data: chunk.data,
                    is_final: chunk.is_final,
                    checksum: Some(chunk.checksum),
                };
                
                connection.send_message(message).await?;
                Ok(ChunkReceipt::Persistent { device_id: *device_id })
            }
            
            FileShareSession::Ephemeral { ephemeral_keys, peer_id, .. } => {
                // Use ephemeral encryption with perfect forward secrecy
                let encrypted_chunk = ephemeral_keys.encrypt_chunk(&chunk)?;
                let message = SpacedropMessage::FileChunk {
                    transfer_id: self.id(),
                    chunk_index: chunk.index,
                    chunk_data: encrypted_chunk,
                    is_final: chunk.is_final,
                    checksum: chunk.checksum,
                };
                
                self.spacedrop_connection.send_message(*peer_id, message).await?;
                Ok(ChunkReceipt::Ephemeral { peer_id: *peer_id })
            }
        }
    }
}
```

## File Sharing Request Types

### Unified Request Structure

```rust
/// Universal file sharing request
#[derive(Debug, Clone)]
pub struct FileShareRequest {
    /// Files to share
    pub files: Vec<FileItem>,
    
    /// Information about sender
    pub sender_info: SenderInfo,
    
    /// Optional message to recipient
    pub message: Option<String>,
    
    /// Target device specification
    pub target: ShareTarget,
    
    /// Sharing preferences
    pub options: ShareOptions,
}

/// Target device specification
#[derive(Debug, Clone)]
pub enum ShareTarget {
    /// Known paired device
    PairedDevice(Uuid),
    
    /// Discovered nearby device
    DiscoveredDevice(SpacedropAdvertisement),
    
    /// Device identifier to resolve
    DeviceIdentifier(DeviceIdentifier),
    
    /// Auto-detect best target
    Auto { 
        preference: TargetPreference,
        filters: Vec<DeviceFilter>,
    },
}

/// File item specification
#[derive(Debug, Clone)]
pub struct FileItem {
    pub path: PathBuf,
    pub metadata: FileMetadata,
    pub share_type: FileShareType,
}

#[derive(Debug, Clone)]
pub enum FileShareType {
    /// Copy file to destination
    Copy { preserve_metadata: bool },
    
    /// Move file to destination
    Move { preserve_metadata: bool },
    
    /// Create reference/link
    Reference { access_level: AccessLevel },
    
    /// Sync with version tracking
    Sync { 
        sync_mode: SyncMode,
        conflict_resolution: ConflictResolution,
    },
}
```

### Share Options and Configuration

```rust
/// Sharing configuration options
#[derive(Debug, Clone)]
pub struct ShareOptions {
    /// Transfer priority
    pub priority: TransferPriority,
    
    /// Compression settings
    pub compression: CompressionConfig,
    
    /// Encryption requirements
    pub encryption: EncryptionRequirements,
    
    /// Progress reporting
    pub progress_reporting: ProgressConfig,
    
    /// Resumability
    pub resumable: bool,
    
    /// Timeout settings
    pub timeouts: TimeoutConfig,
    
    /// Privacy settings for ephemeral shares
    pub privacy: PrivacyConfig,
}

#[derive(Debug, Clone)]
pub enum TransferPriority {
    /// Background transfer
    Low,
    /// Normal priority
    Normal,
    /// High priority (user-initiated)
    High,
    /// Critical (system files)
    Critical,
}

#[derive(Debug, Clone)]
pub struct PrivacyConfig {
    /// Level of sender information shared
    pub sender_visibility: SenderVisibility,
    
    /// Whether to show file previews
    pub show_previews: bool,
    
    /// Metadata sharing level
    pub metadata_level: MetadataLevel,
    
    /// Auto-delete after transfer
    pub ephemeral_keys: bool,
}
```

## Persistent vs Ephemeral Sharing

### Persistent Sharing (Paired Devices)

**Use Case**: Syncing between user's own devices, trusted family/work devices

**Characteristics**:
- Leverages existing persistent connections
- Can auto-accept based on trust level
- Supports resumable transfers
- Efficient for frequent sharing
- Uses established session keys

```rust
/// Configuration for persistent sharing
#[derive(Debug, Clone)]
pub struct PersistentShareConfig {
    /// Auto-accept from trusted devices
    pub auto_accept_trusted: bool,
    
    /// Enable transfer resume
    pub resumable_transfers: bool,
    
    /// Compression for large files
    pub compression_threshold: u64,
    
    /// Sync integration
    pub sync_integration: bool,
    
    /// Conflict resolution strategy
    pub conflict_resolution: ConflictResolution,
}

impl PersistentShareConfig {
    pub fn for_trust_level(trust_level: TrustLevel) -> Self {
        match trust_level {
            TrustLevel::Trusted => Self {
                auto_accept_trusted: true,
                resumable_transfers: true,
                compression_threshold: 100 * 1024 * 1024, // 100MB
                sync_integration: true,
                conflict_resolution: ConflictResolution::AutoMerge,
            },
            
            TrustLevel::Verified => Self {
                auto_accept_trusted: false, // Require confirmation
                resumable_transfers: true,
                compression_threshold: 50 * 1024 * 1024, // 50MB
                sync_integration: false,
                conflict_resolution: ConflictResolution::PromptUser,
            },
            
            _ => Self::secure_defaults(),
        }
    }
}
```

### Ephemeral Sharing (Unknown Devices)

**Use Case**: Sharing with strangers, one-time transfers, maximum privacy

**Characteristics**:
- Perfect forward secrecy per transfer
- Explicit consent required
- No transfer history stored
- Maximum privacy protection
- Discovery-based targeting

```rust
/// Configuration for ephemeral sharing
#[derive(Debug, Clone)]
pub struct EphemeralShareConfig {
    /// Require explicit acceptance
    pub require_explicit_consent: bool,
    
    /// Perfect forward secrecy
    pub perfect_forward_secrecy: bool,
    
    /// Auto-delete keys after transfer
    pub auto_delete_keys: bool,
    
    /// Maximum file size
    pub max_file_size: u64,
    
    /// Transfer timeout
    pub transfer_timeout: Duration,
    
    /// Metadata privacy level
    pub metadata_privacy: MetadataPrivacy,
}

impl Default for EphemeralShareConfig {
    fn default() -> Self {
        Self {
            require_explicit_consent: true,
            perfect_forward_secrecy: true,
            auto_delete_keys: true,
            max_file_size: 5 * 1024 * 1024 * 1024, // 5GB
            transfer_timeout: Duration::from_secs(3600), // 1 hour
            metadata_privacy: MetadataPrivacy::Minimal,
        }
    }
}
```

## Security Model

### Trust-Based Security Levels

```rust
/// Security configuration based on device relationship
#[derive(Debug, Clone)]
pub enum SecurityProfile {
    /// Trusted paired devices
    TrustedDevice {
        /// Reuse persistent session keys
        reuse_session_keys: bool,
        /// Allow auto-accept
        auto_accept: bool,
        /// Enable advanced features
        advanced_features: bool,
    },
    
    /// Verified paired devices
    VerifiedDevice {
        /// Require confirmation for transfers
        require_confirmation: bool,
        /// Limited feature set
        limited_features: bool,
    },
    
    /// Unknown devices (Spacedrop)
    UnknownDevice {
        /// Perfect forward secrecy required
        perfect_forward_secrecy: bool,
        /// Explicit consent required
        explicit_consent: bool,
        /// Minimal metadata sharing
        minimal_metadata: bool,
        /// No transfer history
        no_history: bool,
    },
}
```

### Encryption Strategies

```rust
/// Encryption approach based on relationship
impl FileShareSession {
    fn encryption_strategy(&self) -> EncryptionStrategy {
        match self {
            FileShareSession::Persistent { device_id, trust_level, .. } => {
                EncryptionStrategy::SessionBased {
                    // Reuse established session keys from pairing
                    session_keys: self.get_session_keys(*device_id),
                    key_rotation: trust_level.key_rotation_interval(),
                }
            }
            
            FileShareSession::Ephemeral { .. } => {
                EncryptionStrategy::EphemeralBased {
                    // Generate new ephemeral keys for each transfer
                    ephemeral_keypair: EphemeralKeyPair::generate(),
                    perfect_forward_secrecy: true,
                    auto_delete_keys: true,
                }
            }
        }
    }
}
```

## Discovery and Targeting

### Device Discovery System

```rust
/// Multi-modal device discovery
pub struct DeviceDiscovery {
    /// mDNS for local network
    mdns_discovery: MdnsDiscovery,
    
    /// DHT for internet-wide discovery
    dht_discovery: DhtDiscovery,
    
    /// QR code for manual connection
    qr_discovery: QrCodeDiscovery,
    
    /// Bluetooth for proximity
    bluetooth_discovery: Option<BluetoothDiscovery>,
}

/// Discovered device advertisement
#[derive(Debug, Clone)]
pub struct SpacedropAdvertisement {
    /// Device identification
    pub device_id: Uuid,
    pub device_name: String,
    pub device_type: DeviceType,
    pub network_fingerprint: NetworkFingerprint,
    
    /// Cryptographic identity
    pub public_key: PublicKey,
    pub signature: Signature,
    
    /// Discovery metadata
    pub discovered_via: DiscoveryMethod,
    pub proximity: NetworkProximity,
    pub capabilities: DeviceCapabilities,
    pub timestamp: DateTime<Utc>,
    
    /// Optional visual identification
    pub avatar_hash: Option<[u8; 32]>,
    pub device_icon: Option<DeviceIcon>,
}

#[derive(Debug, Clone)]
pub enum DiscoveryMethod {
    /// Local network multicast
    Mdns { 
        signal_strength: Option<i32>,
        network_interface: String,
    },
    
    /// DHT global discovery
    Dht { 
        hop_count: u32,
        discovery_latency: Duration,
    },
    
    /// QR code scanning
    QrCode { 
        scanned_at: DateTime<Utc>,
        verification_code: String,
    },
    
    /// Bluetooth proximity
    Bluetooth {
        rssi: i32,
        device_class: u32,
    },
}
```

### Smart Target Selection

```rust
/// Intelligent target selection based on context
pub struct TargetSelector {
    device_resolver: Arc<DeviceResolver>,
    user_preferences: UserPreferences,
    context_analyzer: ContextAnalyzer,
}

impl TargetSelector {
    /// Suggest best targets for sharing context
    pub async fn suggest_targets(
        &self,
        files: &[FileItem],
        context: SharingContext,
    ) -> Result<Vec<SuggestedTarget>> {
        let mut suggestions = Vec::new();
        
        // 1. Analyze file types and sharing patterns
        let file_analysis = self.analyze_files(files).await?;
        
        // 2. Get available devices
        let available_devices = self.get_available_devices().await?;
        
        // 3. Score devices based on context
        for device in available_devices {
            let score = self.score_device_for_context(
                &device,
                &file_analysis,
                &context,
            ).await?;
            
            if score.relevance > 0.3 {
                suggestions.push(SuggestedTarget {
                    device,
                    score,
                    reasoning: score.reasoning,
                });
            }
        }
        
        // 4. Sort by relevance and return top suggestions
        suggestions.sort_by(|a, b| b.score.relevance.partial_cmp(&a.score.relevance).unwrap());
        Ok(suggestions.into_iter().take(10).collect())
    }
}
```

## High-Level Integration

### Core API Integration

```rust
impl Core {
    /// Universal file sharing API
    pub async fn share_files(
        &self,
        files: Vec<PathBuf>,
        target: ShareTarget,
        options: ShareOptions,
    ) -> Result<FileShareHandle> {
        // Validate files exist and are accessible
        let file_items = self.validate_and_prepare_files(files).await?;
        
        // Create share request
        let request = FileShareRequest {
            files: file_items,
            sender_info: self.device_info().into(),
            target,
            options,
            message: None,
        };
        
        // Execute via file share manager
        self.file_share_manager.share_files(request).await
    }
    
    /// Discover available share targets
    pub async fn discover_share_targets(
        &self,
        filters: Vec<DeviceFilter>,
    ) -> Result<Vec<ShareTarget>> {
        let mut targets = Vec::new();
        
        // Add connected paired devices
        let connected_devices = self.get_connected_devices().await?;
        for device_id in connected_devices {
            if self.device_matches_filters(&device_id, &filters).await? {
                targets.push(ShareTarget::PairedDevice(device_id));
            }
        }
        
        // Add discovered Spacedrop devices
        let discovered = self.file_share_manager
            .discover_nearby_devices(filters.clone()).await?;
        for advertisement in discovered {
            targets.push(ShareTarget::DiscoveredDevice(advertisement));
        }
        
        Ok(targets)
    }
    
    /// Smart share that auto-selects best target
    pub async fn smart_share(
        &self,
        files: Vec<PathBuf>,
        context: SharingContext,
    ) -> Result<FileShareHandle> {
        // Get intelligent target suggestions
        let suggestions = self.file_share_manager
            .target_selector
            .suggest_targets(&files, context).await?;
        
        if let Some(best_target) = suggestions.first() {
            self.share_files(
                files,
                best_target.device.clone().into(),
                ShareOptions::for_context(&context),
            ).await
        } else {
            Err(FileShareError::NoSuitableTargets)
        }
    }
}
```

### User Interface Integration

```rust
/// UI-friendly file sharing interface
pub struct FileShareUI {
    core: Arc<Core>,
    event_bus: Arc<EventBus>,
}

impl FileShareUI {
    /// Show share dialog with available targets
    pub async fn show_share_dialog(
        &self,
        files: Vec<PathBuf>,
    ) -> Result<Option<FileShareHandle>> {
        // Discover available targets
        let targets = self.core.discover_share_targets(vec![]).await?;
        
        // Group targets by relationship type
        let grouped_targets = self.group_targets_by_relationship(targets).await?;
        
        // Show UI with grouped options
        let ui_result = self.present_share_dialog(ShareDialogData {
            files: files.clone(),
            paired_devices: grouped_targets.paired,
            nearby_devices: grouped_targets.nearby,
            suggestions: grouped_targets.suggestions,
        }).await?;
        
        match ui_result {
            ShareDialogResult::Share { target, options } => {
                let handle = self.core.share_files(files, target, options).await?;
                Ok(Some(handle))
            }
            ShareDialogResult::Cancel => Ok(None),
        }
    }
    
    /// Handle incoming share requests
    pub async fn handle_incoming_share(
        &self,
        request: IncomingShareRequest,
    ) -> Result<ShareResponse> {
        // Show acceptance dialog based on relationship
        let dialog_config = match request.relationship {
            DeviceRelationship::Paired { trust_level: TrustLevel::Trusted, .. } => {
                // Auto-accept or minimal dialog for trusted devices
                AcceptanceDialogConfig::minimal()
            }
            
            DeviceRelationship::Unknown { .. } => {
                // Full security dialog for unknown devices
                AcceptanceDialogConfig::full_security()
            }
            
            _ => AcceptanceDialogConfig::default(),
        };
        
        self.present_acceptance_dialog(request, dialog_config).await
    }
}
```

## Performance Considerations

### Transfer Optimization

```rust
/// Adaptive transfer configuration
#[derive(Debug, Clone)]
pub struct TransferOptimization {
    /// Chunk size adaptation
    pub adaptive_chunking: bool,
    
    /// Parallel transfer streams
    pub parallel_streams: u32,
    
    /// Compression for compatible files
    pub smart_compression: bool,
    
    /// Bandwidth management
    pub bandwidth_limit: Option<u64>,
    
    /// Resume capability
    pub resumable: bool,
}

impl TransferOptimization {
    /// Optimize for device relationship
    pub fn for_relationship(relationship: &DeviceRelationship) -> Self {
        match relationship {
            DeviceRelationship::Paired { trust_level: TrustLevel::Trusted, .. } => {
                Self {
                    adaptive_chunking: true,
                    parallel_streams: 4,
                    smart_compression: true,
                    bandwidth_limit: None,
                    resumable: true,
                }
            }
            
            DeviceRelationship::Unknown { .. } => {
                Self {
                    adaptive_chunking: false, // Consistent for security
                    parallel_streams: 1,
                    smart_compression: false, // Avoid metadata leakage
                    bandwidth_limit: Some(10 * 1024 * 1024), // 10MB/s limit
                    resumable: false, // Ephemeral transfers
                }
            }
            
            _ => Self::conservative_defaults(),
        }
    }
}
```

### Bandwidth Management

```rust
/// Global bandwidth coordination
pub struct BandwidthManager {
    /// Active transfer sessions
    active_transfers: Arc<RwLock<HashMap<Uuid, TransferBandwidth>>>,
    
    /// Global bandwidth limits
    global_limits: BandwidthLimits,
    
    /// QoS prioritization
    qos_manager: QoSManager,
}

#[derive(Debug, Clone)]
pub struct BandwidthLimits {
    /// Maximum total upload bandwidth
    pub max_upload: Option<u64>,
    
    /// Maximum total download bandwidth
    pub max_download: Option<u64>,
    
    /// Per-transfer limits
    pub per_transfer_limit: Option<u64>,
    
    /// Priority-based allocation
    pub priority_weights: HashMap<TransferPriority, f32>,
}
```

## Error Handling and Recovery

### Comprehensive Error Types

```rust
/// File sharing specific errors
#[derive(Debug, thiserror::Error)]
pub enum FileShareError {
    #[error("Device relationship could not be established: {0}")]
    RelationshipError(String),
    
    #[error("File access denied: {path}")]
    FileAccessDenied { path: PathBuf },
    
    #[error("Transfer was rejected by recipient: {reason}")]
    TransferRejected { reason: String },
    
    #[error("Network error during transfer: {0}")]
    NetworkError(#[from] NetworkError),
    
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    
    #[error("File integrity check failed")]
    IntegrityError,
    
    #[error("Transfer timeout")]
    TimeoutError,
    
    #[error("Insufficient storage space")]
    InsufficientStorage,
    
    #[error("Device is blocked: {reason}")]
    DeviceBlocked { reason: String },
    
    #[error("No suitable targets found")]
    NoSuitableTargets,
}
```

### Recovery Strategies

```rust
/// Transfer recovery and retry logic
impl FileShareSession {
    /// Attempt to recover from transfer failure
    pub async fn recover_from_failure(
        &mut self,
        error: FileShareError,
    ) -> Result<RecoveryAction> {
        match error {
            FileShareError::NetworkError(_) => {
                match self {
                    FileShareSession::Persistent { .. } => {
                        // Try to re-establish persistent connection
                        self.reconnect_persistent().await
                    }
                    
                    FileShareSession::Ephemeral { .. } => {
                        // Ephemeral transfers don't retry network errors
                        Ok(RecoveryAction::Fail)
                    }
                }
            }
            
            FileShareError::IntegrityError => {
                // Retry with smaller chunks
                self.retry_with_smaller_chunks().await
            }
            
            FileShareError::TimeoutError => {
                // Extend timeout and retry
                self.extend_timeout_and_retry().await
            }
            
            _ => Ok(RecoveryAction::Fail),
        }
    }
}
```

## Implementation Roadmap

### Phase 1: Foundation (Weeks 1-2)
- [ ] Implement `FileShareManager` core structure
- [ ] Create `DeviceResolver` for relationship determination
- [ ] Build unified `FileShareRequest` and `ShareTarget` types
- [ ] Implement basic session management

### Phase 2: Persistent Sharing (Weeks 3-4)
- [ ] Integrate with existing persistent networking
- [ ] Implement persistent file transfer using `DeviceMessage`
- [ ] Add trust-level based auto-acceptance
- [ ] Create resumable transfer capability

### Phase 3: Ephemeral Sharing (Weeks 5-6)
- [ ] Implement ephemeral Spacedrop service
- [ ] Add perfect forward secrecy key exchange
- [ ] Create discovery and advertisement system
- [ ] Implement explicit consent flow

### Phase 4: Discovery and Targeting (Week 7)
- [ ] Build multi-modal device discovery
- [ ] Implement smart target suggestion system
- [ ] Add QR code and proximity-based discovery
- [ ] Create device filtering and context analysis

### Phase 5: Core Integration (Week 8)
- [ ] Integrate with Core API
- [ ] Create high-level sharing methods
- [ ] Implement UI abstraction layer
- [ ] Add event system integration

### Phase 6: Optimization and Polish (Weeks 9-10)
- [ ] Implement bandwidth management
- [ ] Add adaptive transfer optimization
- [ ] Create comprehensive error recovery
- [ ] Performance testing and tuning

### Phase 7: Security Audit (Week 11)
- [ ] Security review of ephemeral key handling
- [ ] Audit trust level enforcement
- [ ] Validate encryption implementations
- [ ] Penetration testing

### Phase 8: User Experience (Week 12)
- [ ] Native UI integration points
- [ ] File preview and metadata handling
- [ ] Progress reporting and notifications
- [ ] Cross-platform compatibility testing

## Security Audit Checklist

### Cryptographic Security
- [ ] Ephemeral key generation uses secure randomness
- [ ] Perfect forward secrecy properly implemented
- [ ] Session key reuse follows security best practices
- [ ] Key deletion is cryptographically secure

### Protocol Security
- [ ] No downgrade attacks possible
- [ ] Replay attack prevention
- [ ] Message authentication and integrity
- [ ] Proper nonce and timestamp handling

### Privacy Protection
- [ ] Minimal metadata leakage in ephemeral mode
- [ ] Device fingerprinting resistance
- [ ] Transfer history handling
- [ ] Network traffic analysis resistance

### Access Control
- [ ] Trust level enforcement
- [ ] Device blocking mechanisms
- [ ] File access permission checking
- [ ] User consent validation

---

This file sharing design provides a unified, secure, and efficient system that adapts to device relationships while maintaining the separation of concerns between device pairing and file transfer protocols.