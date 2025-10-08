# Spacedrive Device System

**Status**: Production
**Version**: 2.0
**Last Updated**: 2025-10-08

## Overview

Devices are the fundamental building blocks of Spacedrive's multi-device architecture. A **Device** represents a single machine (laptop, phone, server) running Spacedrive. Devices can pair with each other, share libraries, and synchronize data.

This document covers the complete device lifecycle from initialization to pairing to sync participation.

## Architecture: Three Layers

Devices exist across three distinct layers:

```
┌──────────────────────────────────────────────────────────────────┐
│ 1. IDENTITY LAYER (device/manager.rs, device/config.rs)         │
│    • Device initialization and configuration                     │
│    • Persistent device ID and metadata                           │
│    • Master encryption key management                            │
│    • Platform-specific detection (OS, hardware)                  │
└──────────────────────────────────────────────────────────────────┘
                              ↓
┌──────────────────────────────────────────────────────────────────┐
│ 2. DOMAIN LAYER (domain/device.rs, infra/db/entities/device.rs) │
│    • Rich Device domain model                                    │
│    • Sync leadership per library                                 │
│    • Online/offline state                                        │
│    • Database persistence in each library                        │
└──────────────────────────────────────────────────────────────────┘
                              ↓
┌──────────────────────────────────────────────────────────────────┐
│ 3. NETWORK LAYER (service/network/device/)                      │
│    • P2P discovery and connections                               │
│    • Device pairing protocol                                     │
│    • Connection state management                                 │
│    • Session key management                                      │
└──────────────────────────────────────────────────────────────────┘
```

## Layer 1: Device Identity

### DeviceManager

The `DeviceManager` manages the **current device's** identity and configuration.

**Location**: `core/src/device/manager.rs`

```rust
pub struct DeviceManager {
    config: Arc<RwLock<DeviceConfig>>,
    device_key_manager: DeviceKeyManager,
    data_dir: Option<PathBuf>,
}

impl DeviceManager {
    /// Initialize device (creates new ID on first run)
    pub fn init() -> Result<Self, DeviceError>;

    /// Initialize with custom data directory (for iOS/Android)
    pub fn init_with_path_and_name(
        data_dir: &PathBuf,
        device_name: Option<String>,
    ) -> Result<Self, DeviceError>;

    /// Get the current device's UUID
    pub fn device_id(&self) -> Result<Uuid, DeviceError>;

    /// Get device as domain model
    pub fn to_device(&self) -> Result<Device, DeviceError>;

    /// Update device name
    pub fn set_name(&self, name: String) -> Result<(), DeviceError>;

    /// Get master encryption key
    pub fn master_key(&self) -> Result<[u8; 32], DeviceError>;
}
```

### DeviceConfig

Persistent configuration stored on disk.

**Location**: `core/src/device/config.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    /// Unique device identifier (generated once, never changes)
    pub id: Uuid,

    /// User-friendly device name (can be updated)
    pub name: String,

    /// When this device was first initialized
    pub created_at: DateTime<Utc>,

    /// Hardware model (e.g., "MacBook Pro 16-inch 2023")
    pub hardware_model: Option<String>,

    /// Operating system
    pub os: String,

    /// Spacedrive version that created this config
    pub version: String,
}
```

**Storage Location**:
- macOS: `~/Library/Application Support/com.spacedrive/device.json`
- Linux: `~/.config/spacedrive/device.json`
- Windows: `%APPDATA%/Spacedrive/device.json`
- iOS/Android: Custom data directory (passed via `init_with_path`)

### Global Device ID

For performance, the device ID is cached globally.

**Location**: `core/src/device/id.rs`

```rust
/// Global reference to current device ID
pub static CURRENT_DEVICE_ID: Lazy<RwLock<Uuid>> = Lazy::new(|| RwLock::new(Uuid::nil()));

/// Initialize the current device ID (called during Core init)
pub fn set_current_device_id(id: Uuid);

/// Get the current device ID (fast, no error handling)
pub fn get_current_device_id() -> Uuid;
```

**Usage**:
```rust
// During Core initialization
let device_manager = DeviceManager::init()?;
set_current_device_id(device_manager.device_id()?);

// Anywhere in the codebase
let device_id = get_current_device_id();
```

**Rationale**:
- Device ID accessed frequently (audit logs, sync entries, actions)
- Immutable once set (no concurrency concerns)
- Performance: Avoids Arc<RwLock> overhead on every access
- Convenience: No need to pass CoreContext everywhere

## Layer 2: Domain & Database

### Device Domain Model

The rich domain model used in application logic and API responses.

**Location**: `core/src/domain/device.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    /// Unique identifier
    pub id: Uuid,

    /// Human-readable name
    pub name: String,

    /// Operating system
    pub os: OperatingSystem,

    /// Hardware model (e.g., "MacBook Pro", "iPhone 15")
    pub hardware_model: Option<String>,

    /// Network addresses for P2P connections
    pub network_addresses: Vec<String>,

    /// Whether this device is currently online
    pub is_online: bool,

    /// Sync leadership status per library
    pub sync_leadership: HashMap<Uuid, SyncRole>,

    /// Last time this device was seen
    pub last_seen_at: DateTime<Utc>,

    /// When this device was first added
    pub created_at: DateTime<Utc>,

    /// When this device info was last updated
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum SyncRole {
    /// This device maintains the sync log for the library
    Leader,

    /// This device syncs from the leader
    Follower,

    /// This device doesn't participate in sync for this library
    Inactive,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum OperatingSystem {
    MacOS,
    Windows,
    Linux,
    IOs,
    Android,
    Other,
}
```

### Key Methods

```rust
impl Device {
    /// Create the current device
    pub fn current() -> Self;

    /// Mark device as online/offline
    pub fn mark_online(&mut self);
    pub fn mark_offline(&mut self);

    /// Sync role management
    pub fn set_sync_role(&mut self, library_id: Uuid, role: SyncRole);
    pub fn sync_role(&self, library_id: &Uuid) -> SyncRole;
    pub fn is_sync_leader(&self, library_id: &Uuid) -> bool;
    pub fn leader_libraries(&self) -> Vec<Uuid>;
}
```

### Database Entity

Devices are stored **per library** (not globally).

**Location**: `core/src/infra/db/entities/device.rs`

```rust
#[derive(Clone, Debug, DeriveEntityModel)]
#[sea_orm(table_name = "devices")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,                    // Database primary key
    pub uuid: Uuid,                 // Global device identifier
    pub name: String,
    pub os: String,
    pub os_version: Option<String>,
    pub hardware_model: Option<String>,
    pub network_addresses: Json,    // Vec<String>
    pub is_online: bool,
    pub last_seen_at: DateTimeUtc,
    pub capabilities: Json,         // DeviceCapabilities
    pub sync_leadership: Json,      // HashMap<Uuid, SyncRole>
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}
```

**Why per-library?**
- Different devices may have access to different libraries
- Sync role is library-specific (leader in Library A, follower in Library B)
- Library-specific device metadata (last_seen per library)

## Layer 3: Network

### DeviceRegistry

Central state manager for all network-layer device interactions.

**Location**: `core/src/service/network/device/registry.rs`

```rust
pub struct DeviceRegistry {
    device_manager: Arc<DeviceManager>,
    devices: HashMap<Uuid, DeviceState>,
    node_to_device: HashMap<NodeId, Uuid>,
    session_to_device: HashMap<Uuid, Uuid>,
    persistence: DevicePersistence,
}

pub enum DeviceState {
    /// Discovered via Iroh (not yet paired)
    Discovered {
        node_id: NodeId,
        node_addr: NodeAddr,
        discovered_at: DateTime<Utc>,
    },

    /// Pairing in progress
    Pairing {
        node_id: NodeId,
        session_id: Uuid,
        started_at: DateTime<Utc>,
    },

    /// Successfully paired (persisted)
    Paired {
        info: DeviceInfo,
        session_keys: SessionKeys,
        paired_at: DateTime<Utc>,
    },

    /// Currently connected (active P2P connection)
    Connected {
        info: DeviceInfo,
        session_keys: SessionKeys,
        connection: ConnectionInfo,
        connected_at: DateTime<Utc>,
    },

    /// Disconnected (but still paired)
    Disconnected {
        info: DeviceInfo,
        session_keys: SessionKeys,
        last_seen: DateTime<Utc>,
        reason: DisconnectionReason,
    },
}
```

### Device Lifecycle

```
┌─────────────┐
│ Unknown     │
└──────┬──────┘
       │ Iroh discovery
       ↓
┌─────────────┐
│ Discovered  │ ← Device found on network
└──────┬──────┘
       │ User initiates pairing
       ↓
┌─────────────┐
│ Pairing     │ ← Cryptographic handshake
└──────┬──────┘
       │ Challenge/response succeeds
       ↓
┌─────────────┐
│ Paired      │ ← Persisted, can reconnect
└──────┬──────┘
       │ P2P connection established
       ↓
┌─────────────┐
│ Connected   │ ← Active, can send messages
└──────┬──────┘
       │ Connection lost
       ↓
┌─────────────┐
│ Disconnected│ ← Can reconnect
└─────────────┘
       │ Auto-reconnect
       └─→ Connected
```

### DeviceInfo

Metadata exchanged during pairing and stored in registry.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: Uuid,
    pub device_name: String,
    pub device_type: DeviceType,
    pub os_version: String,
    pub app_version: String,
    pub network_fingerprint: NetworkFingerprint,
    pub last_seen: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceType {
    Desktop,
    Laptop,
    Mobile,
    Server,
    Other(String),
}
```

## Device Pairing Protocol

Cryptographic authentication between two devices.

**Location**: `core/src/service/network/protocol/pairing/`

### Pairing Flow

```
Device A (Initiator)              Device B (Joiner)
─────────────────────            ──────────────────
1. Generate pairing code
   → "ABCD-1234-EFGH"

2. Display code to user
                                  3. User enters code

                                  4. PairingRequest →
                                     { device_info, public_key }

5. Generate challenge
   ← Challenge
     { challenge_bytes }

                                  6. Sign challenge with private key

                                  7. Response →
                                     { signature, device_info }

8. Verify signature

9. Derive shared secret

10. Complete →
    { success: true }
                                  11. Save as paired device

12. Save as paired device

13. Both devices now in "Paired" state
    • Can establish P2P connections
    • Can discover each other's libraries
    • Can set up library sync
```

### Pairing Actions

**Initiate pairing**:
```rust
client.action("network.pair.generate.v1", {}) → PairingCode
```

**Join pairing**:
```rust
client.action("network.pair.join.v1", { code: "ABCD-1234-EFGH" }) → Success
```

**Query pairing status**:
```rust
client.query("network.pair.status.v1", {}) → PairingStatus
```

## Device Discovery

Devices discover each other via **Iroh's mDNS** on local networks.

```rust
// Iroh automatically discovers nearby nodes
// NetworkingService listens for discovery events

// When node discovered:
device_registry.add_discovered_node(device_id, node_id, node_addr);
// State: Discovered

// User can now initiate pairing with this device
```

## Device Registration in Libraries

After pairing, devices must be **registered in each other's libraries** to enable sync.

**Process** (see `sync-setup.md`):
1. Pair devices (network layer)
2. Discover remote libraries
3. Register Device B in Library A's database
4. Register Device A in Library B's database
5. Elect sync leader
6. Start sync service

**Database Entry**:
```sql
-- In Library A's database
INSERT INTO devices (uuid, name, os, sync_leadership, ...)
VALUES ('device-b-uuid', 'Bob's MacBook', 'macOS', '{"lib-a-uuid": "Follower"}', ...);

-- In Library B's database
INSERT INTO devices (uuid, name, os, sync_leadership, ...)
VALUES ('device-a-uuid', 'Alice's iPhone', 'iOS', '{"lib-b-uuid": "Leader"}', ...);
```

## Sync Leadership

Each library has **one leader device** that assigns sync log sequence numbers.

### Leadership Model

```rust
// Device domain model tracks leadership per library
pub struct Device {
    pub sync_leadership: HashMap<Uuid, SyncRole>, // library_id → role
}

impl Device {
    pub fn set_sync_role(&mut self, library_id: Uuid, role: SyncRole);
    pub fn is_sync_leader(&self, library_id: &Uuid) -> bool;
    pub fn leader_libraries(&self) -> Vec<Uuid>;
}
```

### Election Strategy

1. **Initial leader**: Device that creates the library
2. **Explicit assignment**: During library sync setup
3. **Failover** (future): Heartbeat-based re-election if leader goes offline

### Usage in TransactionManager

```rust
impl TransactionManager {
    async fn next_sequence(&self, library_id: Uuid) -> Result<u64, TxError> {
        // Check if current device is leader
        if !self.is_leader(library_id).await {
            return Err(TxError::NotLeader);
        }

        // Assign next sequence number
        let mut sequences = self.sync_sequence.lock().unwrap();
        let seq = sequences.entry(library_id).or_insert(0);
        *seq += 1;
        Ok(*seq)
    }

    async fn is_leader(&self, library_id: Uuid) -> bool {
        // Query device table in library database
        let device = self.get_current_device(library_id).await?;
        device.is_sync_leader(&library_id)
    }
}
```

## Device Relationships

Devices have relationships with other core entities:

### Devices ↔ Libraries

**Relationship**: Many-to-Many
- One device can access multiple libraries
- One library can be accessed by multiple devices
- Each device has a role (Leader/Follower/Inactive) per library

**Implementation**:
- Devices stored in each library's database
- Global device registry managed by NetworkingService
- Library sync setup creates bidirectional registration

### Devices ↔ Locations

**Relationship**: One-to-Many
- Each location belongs to one device
- One device can have multiple locations

**Schema**:
```sql
CREATE TABLE locations (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL,
    device_id INTEGER NOT NULL,  -- Foreign key to devices.id
    entry_id INTEGER NOT NULL,
    -- ...
    FOREIGN KEY (device_id) REFERENCES devices(id)
);
```

**Semantics**:
- `/Users/alice/Photos` on Device A is a different location from `/storage/DCIM` on Device B
- Each device indexes its own filesystem
- Location ownership never changes (location is tied to device)

### Devices ↔ Volumes

**Relationship**: One-to-Many
- Each volume belongs to one device
- One device can have multiple volumes (drives)

**Schema**:
```sql
CREATE TABLE volumes (
    id INTEGER PRIMARY KEY,
    uuid TEXT NOT NULL,
    device_id TEXT NOT NULL,  -- Foreign key to devices.uuid
    fingerprint TEXT NOT NULL,
    -- ...
);
```

**Semantics**:
- Volumes are device-specific (external SSD on Device A)
- Volume fingerprints enable cross-device recognition (same SSD connected to Device B)
- Volume metadata syncs (name, capacity) but content does not (unless user configures sync conduit)

## Queries and Actions

### Query: List Paired Devices

**Endpoint**: `query:network.devices.list.v1`

**Location**: `core/src/ops/network/devices/query.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListPairedDevicesInput {
    /// Whether to include only connected devices
    #[serde(default)]
    pub connected_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ListPairedDevicesOutput {
    pub devices: Vec<PairedDeviceInfo>,
    pub total: usize,
    pub connected: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PairedDeviceInfo {
    pub id: Uuid,
    pub name: String,
    pub device_type: String,
    pub os_version: String,
    pub app_version: String,
    pub is_connected: bool,
    pub last_seen: DateTime<Utc>,
}
```

**Usage**:
```rust
// Get all paired devices
let output = client.query("network.devices.list.v1", {
    connected_only: false
}).await?;

println!("Paired devices: {}", output.total);
println!("Connected: {}", output.connected);
```

### Action: Generate Pairing Code

**Endpoint**: `action:network.pair.generate.v1`

**Location**: `core/src/ops/network/pair/generate/action.rs`

```rust
pub struct GeneratePairingCodeAction;

impl CoreAction for GeneratePairingCodeAction {
    type Output = PairingCodeOutput;

    async fn execute(self, context: Arc<CoreContext>) -> ActionResult<Self::Output> {
        let networking = context.get_networking().await?;
        let code = networking.start_pairing().await?;

        Ok(PairingCodeOutput {
            code: code.to_string(),
            expires_at: Utc::now() + Duration::seconds(300), // 5 minutes
        })
    }
}
```

### Action: Join Pairing

**Endpoint**: `action:network.pair.join.v1`

```rust
pub struct JoinPairingAction {
    pub code: String,
}

impl CoreAction for JoinPairingAction {
    type Output = JoinPairingOutput;

    async fn execute(self, context: Arc<CoreContext>) -> ActionResult<Self::Output> {
        let networking = context.get_networking().await?;
        let pairing_code = PairingCode::from_string(&self.code)?;

        networking.join_pairing(pairing_code).await?;

        Ok(JoinPairingOutput { success: true })
    }
}
```

## Device as an Identifiable Resource

Devices should be **cacheable** on the client.

### Implementation

```rust
impl Identifiable for Device {
    type Id = Uuid;

    fn resource_id(&self) -> Self::Id {
        self.id
    }

    fn resource_type() -> &'static str {
        "device"
    }
}
```

### Syncable Implementation

Devices sync across libraries when registered.

```rust
impl Syncable for entities::device::Model {
    const SYNC_MODEL: &'static str = "device";

    fn sync_id(&self) -> Uuid {
        self.uuid
    }

    fn version(&self) -> i64 {
        // Devices use timestamp as version
        self.updated_at.timestamp()
    }

    fn exclude_fields() -> Option<&'static [&'static str]> {
        Some(&[
            "id",              // Database primary key
            "is_online",       // Ephemeral state
            "network_addresses", // Network-specific
        ])
    }
}
```

**What syncs**:
- ✅ Device name changes
- ✅ Hardware model updates
- ✅ Sync role assignments
- ❌ Online/offline status (ephemeral)
- ❌ Network addresses (connection-specific)

## Device Events

Using the unified event system:

```rust
// When device connects
Event {
    envelope: { id, timestamp, library_id: None },
    kind: ResourceChanged {
        resource_type: "device",
        resource: Device { id, name, is_online: true, ... }
    }
}

// When device disconnects
Event {
    envelope: { id, timestamp, library_id: None },
    kind: ResourceChanged {
        resource_type: "device",
        resource: Device { id, name, is_online: false, ... }
    }
}

// When sync role changes
Event {
    envelope: { id, timestamp, library_id: Some(lib_uuid) },
    kind: ResourceChanged {
        resource_type: "device",
        resource: Device { id, name, sync_leadership: { lib_uuid: Leader }, ... }
    }
}
```

**Client handling** (automatic via type registry):
```swift
// NO device-specific code needed!
// Generic handler works automatically:
case .ResourceChanged("device", let json):
    let device = try ResourceTypeRegistry.decode("device", from: json)
    cache.updateEntity(device)
    // UI showing device list updates instantly!
```

## Security

### Cryptographic Identity

Each device has a unique cryptographic identity managed by Iroh:
- **NodeId**: Derived from Ed25519 public key
- **Key pair**: Generated and stored securely by Iroh
- **NetworkFingerprint**: Combines NodeId + device UUID

### Session Keys

After pairing, devices derive session keys for encrypted communication:

```rust
pub struct SessionKeys {
    pub encrypt_key: [u8; 32],
    pub decrypt_key: [u8; 32],
    pub mac_key: [u8; 32],
}

impl SessionKeys {
    /// Derive from shared secret (via ECDH)
    pub fn from_shared_secret(secret: Vec<u8>) -> Self;
}
```

### Trust Levels

```rust
pub enum TrustLevel {
    /// Cryptographically verified via pairing
    Verified,

    /// User manually approved
    Trusted,

    /// Pending verification
    Pending,

    /// Explicitly untrusted
    Blocked,
}
```

## Persistence

### Network Layer Persistence

Paired devices persisted to survive app restarts.

**Location**: `~/.spacedrive/paired_devices.json`

```rust
pub struct PersistedPairedDevice {
    pub device_id: Uuid,
    pub device_info: DeviceInfo,
    pub session_keys: SessionKeys,
    pub trust_level: TrustLevel,
    pub paired_at: DateTime<Utc>,
    pub auto_reconnect: bool,
}
```

### Library Database Persistence

Devices registered in each library's database.

**Table**: `devices` (per library)

```sql
CREATE TABLE devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    os TEXT NOT NULL,
    os_version TEXT,
    hardware_model TEXT,
    network_addresses TEXT NOT NULL, -- JSON array
    is_online BOOLEAN NOT NULL DEFAULT 0,
    last_seen_at TEXT NOT NULL,
    capabilities TEXT NOT NULL,      -- JSON object
    sync_leadership TEXT NOT NULL,   -- JSON: { "lib-uuid": "Leader" }
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_devices_uuid ON devices(uuid);
```

## API Examples

### Swift Client

```swift
// List paired devices
let devices = try await client.query(
    "network.devices.list.v1",
    input: ListPairedDevicesInput(connectedOnly: false)
)

print("Paired devices: \(devices.total)")
for device in devices.devices {
    print("\(device.name) - \(device.isConnected ? "Connected" : "Offline")")
}

// Start pairing
let pairingCode = try await client.action(
    "network.pair.generate.v1",
    input: EmptyInput()
)

print("Pairing code: \(pairingCode.code)")
print("Show this code to the other device")

// Join pairing
let result = try await client.action(
    "network.pair.join.v1",
    input: JoinPairingInput(code: "ABCD-1234-EFGH")
)

if result.success {
    print("Successfully paired!")
}
```

### TypeScript Client

```typescript
// List paired devices
const devices = await client.query('network.devices.list.v1', {
  connectedOnly: false
});

console.log(`Paired devices: ${devices.total}`);
devices.devices.forEach(device => {
  console.log(`${device.name} - ${device.isConnected ? 'Connected' : 'Offline'}`);
});

// Generate pairing code
const pairing = await client.action('network.pair.generate.v1', {});
console.log(`Pairing code: ${pairing.code}`);

// Join pairing
const result = await client.action('network.pair.join.v1', {
  code: 'ABCD-1234-EFGH'
});
```

## Device State Queries

### Get Current Device

```rust
// Get the device running this code
let device_manager = context.device_manager();
let device = device_manager.to_device()?;

println!("Device ID: {}", device.id);
println!("Device name: {}", device.name);
println!("OS: {}", device.os);
```

### Get Device by ID

```rust
// Query device from library database
let device = entities::device::Entity::find()
    .filter(entities::device::Column::Uuid.eq(device_id))
    .one(library.db().conn())
    .await?
    .ok_or(QueryError::DeviceNotFound(device_id))?;

let domain_device = Device::try_from(device)?;
```

### Get Network State

```rust
// Get device network state (from registry)
let networking = context.get_networking().await?;
let registry = networking.device_registry();
let registry_lock = registry.read().await;

if let Some(state) = registry_lock.get_device_state(device_id) {
    match state {
        DeviceState::Connected { connection, .. } => {
            println!("Device connected with {} addresses", connection.addresses.len());
        }
        DeviceState::Paired { .. } => {
            println!("Device paired but not connected");
        }
        _ => {}
    }
}
```

## Platform-Specific Considerations

### iOS/Android

Mobile platforms require special handling:

```rust
// iOS: UIDevice.name from Swift passed to Rust
let device_manager = DeviceManager::init_with_path_and_name(
    &app_data_dir,
    Some(ui_device_name), // From UIDevice.current.name
)?;

// Device name updates when user changes it in Settings
// (On next app launch, name is updated in config)
```

### Desktop vs Mobile

```rust
fn detect_device_type() -> DeviceType {
    if cfg!(target_os = "ios") || cfg!(target_os = "android") {
        DeviceType::Mobile
    } else if cfg!(target_os = "macos") {
        // Could detect MacBook vs iMac vs Mac Pro
        DeviceType::Laptop
    } else {
        DeviceType::Desktop
    }
}
```

## Integration with Core Systems

### With Libraries

```rust
// Get all devices in a library
let devices = entities::device::Entity::find()
    .all(library.db().conn())
    .await?;

// Check if device has access to library
let has_access = devices.iter().any(|d| d.uuid == device_id);

// Get sync leader for library
let leader = devices.iter()
    .find(|d| {
        let sync_leadership: HashMap<Uuid, SyncRole> =
            serde_json::from_value(d.sync_leadership.clone()).unwrap();
        matches!(sync_leadership.get(&library_id), Some(SyncRole::Leader))
    });
```

### With Locations

```rust
// Get all locations on a device
let locations = entities::location::Entity::find()
    .filter(entities::location::Column::DeviceId.eq(device_db_id))
    .all(library.db().conn())
    .await?;

// Location indexing is device-local
// Each device indexes its own filesystem independently
```

### With Volumes

```rust
// Get all volumes on a device
let volumes = entities::volume::Entity::find()
    .filter(entities::volume::Column::DeviceId.eq(device_uuid))
    .all(library.db().conn())
    .await?;

// Volumes follow devices (USB drive connected to Device A)
// Volume fingerprints enable cross-device recognition
```

### With Sync System

```rust
// Check if this device should sync
if device.is_sync_leader(&library_id) {
    // This device creates sync logs
    tm.commit(library, model).await?;
} else {
    // This device is a follower - apply sync entries
    sync_follower.sync_iteration().await?;
}
```

## Testing

### Unit Tests

```rust
#[test]
fn test_device_creation() {
    let device = Device::current();
    assert!(!device.id.is_nil());
    assert!(!device.name.is_empty());
}

#[test]
fn test_sync_role_management() {
    let mut device = Device::new("Test Device".into());
    let library_id = Uuid::new_v4();

    // Initially inactive
    assert_eq!(device.sync_role(&library_id), SyncRole::Inactive);

    // Set as leader
    device.set_sync_role(library_id, SyncRole::Leader);
    assert!(device.is_sync_leader(&library_id));

    // Get leader libraries
    let leaders = device.leader_libraries();
    assert_eq!(leaders.len(), 1);
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_device_pairing_flow() {
    // Device A generates code
    let code = device_a.generate_pairing_code().await?;

    // Device B joins
    device_b.join_pairing(code).await?;

    // Verify both in Paired state
    let a_state = device_a.get_device_state(device_b.id());
    assert!(matches!(a_state, DeviceState::Paired { .. }));

    let b_state = device_b.get_device_state(device_a.id());
    assert!(matches!(b_state, DeviceState::Paired { .. }));
}
```

## Performance

### Device Lookups

- **By UUID**: Indexed, O(1) lookup
- **By NodeId**: HashMap in DeviceRegistry, O(1)
- **By session**: HashMap in DeviceRegistry, O(1)
- **All devices**: O(n) scan, but typically <10 devices per library

### Network State

- DeviceRegistry holds in-memory state (fast)
- Persistence updates are async (no blocking)
- Auto-reconnect on startup (loads paired devices from disk)

## Monitoring

### Device Status

```rust
// Query core status includes device info
let status = client.query("core.status.v1", {}).await?;
println!("Current device: {}", status.device_name);
println!("Device ID: {}", status.device_id);

// List paired devices with connection status
let devices = client.query("network.devices.list.v1", {}).await?;
println!("{} of {} devices connected", devices.connected, devices.total);
```

### Events

```rust
// Device connected event
Event {
    kind: ResourceChanged {
        resource_type: "device",
        resource: Device { is_online: true, ... }
    }
}

// Device disconnected event
Event {
    kind: ResourceChanged {
        resource_type: "device",
        resource: Device { is_online: false, ... }
    }
}
```

## Troubleshooting

### Device Not Pairing

**Symptom**: Pairing fails or times out

**Checks**:
1. Both devices on same network?
2. mDNS discovery working? (check Iroh logs)
3. Firewall blocking connections?
4. Pairing code entered correctly?
5. Pairing code expired? (5 minute TTL)

**Debug**:
```bash
# Check device discovery
RUST_LOG=iroh=debug,sd_core::service::network=debug cargo run

# Look for:
# - "Node discovered via mDNS"
# - "Pairing request received"
# - "Challenge/response exchange"
```

### Device Showing as Offline

**Symptom**: Paired device shows offline but is actually running

**Checks**:
1. Connection lost? (network change, sleep)
2. Auto-reconnect disabled?
3. Device behind NAT/firewall?

**Resolution**:
- Devices auto-reconnect every 30 seconds
- Manual reconnect: Close and reopen app
- Check relay connection if direct P2P fails

### Sync Not Working

**Symptom**: Changes not syncing between devices

**Checks**:
1. Devices registered in each library?
2. Sync leader elected?
3. Follower sync service running?
4. Check sync log sequence numbers

**Debug**:
```rust
// Check if device is registered in library
let device = entities::device::Entity::find()
    .filter(entities::device::Column::Uuid.eq(device_id))
    .one(library.db().conn())
    .await?;

if device.is_none() {
    println!("Device not registered in library!");
    // Run library sync setup
}

// Check sync role
let device = device.unwrap();
let sync_leadership: HashMap<Uuid, SyncRole> =
    serde_json::from_value(device.sync_leadership)?;

match sync_leadership.get(&library_id) {
    Some(SyncRole::Leader) => println!("This is the leader"),
    Some(SyncRole::Follower) => println!("This is a follower"),
    _ => println!("Not participating in sync for this library!"),
}
```

## Future Enhancements

### Multi-Leader Support

Current design: Single leader per library
Future: Multiple leaders with conflict-free sequence assignment

### Device Capabilities

```rust
pub struct DeviceCapabilities {
    pub can_index: bool,           // Has filesystem access
    pub can_generate_thumbnails: bool,
    pub can_transcode_video: bool,
    pub has_gpu: bool,
    pub storage_capacity: Option<u64>,
}
```

Usage: Job dispatch optimization (assign thumbnail generation to device with GPU)

### Device Groups

```rust
pub struct DeviceGroup {
    pub id: Uuid,
    pub name: String,
    pub device_ids: Vec<Uuid>,
    pub sync_policy: SyncPolicy,
}
```

Usage: "Sync all my personal devices" vs "Work devices only"

## References

- **Sync System**: `docs/core/sync.md` (sync leadership and follower service)
- **Sync Setup**: `docs/core/sync-setup.md` (library registration flow)
- **Pairing Protocol**: `docs/core/design/DEVICE_PAIRING_PROTOCOL.md` (crypto details)
- **Networking**: `docs/core/design/NETWORKING_SYSTEM_DESIGN.md` (P2P architecture)
- **Implementation**:
  - `core/src/device/` (identity layer)
  - `core/src/domain/device.rs` (domain model)
  - `core/src/service/network/device/` (network layer)
