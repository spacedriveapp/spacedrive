# CLI Pairing Implementation Plan

## Overview

This document outlines the implementation plan for integrating persistent pairing functionality into the Spacedrive CLI. The key difference from the demo (examples/network_pairing_demo.rs) is that CLI pairing must be **persistent** - devices remember each other across restarts and auto-reconnect.

## Current State Analysis

### ✅ What Works

- **LibP2PPairingProtocol**: Fully functional pairing protocol (proven by demo)
- **PersistentConnectionManager**: Complete long-term device connection management
- **NetworkingService**: Persistent networking infrastructure
- **CLI Interface**: Complete user interface for pairing commands
- **Daemon Integration**: Command routing and response handling

### ❌ What's Missing

- **Pairing-to-Persistence Bridge**: No integration between LibP2PPairingProtocol and PersistentConnectionManager
- **Core Implementation**: Core pairing methods contain TODO stubs
- **Session Management**: No tracking of active pairing sessions
- **Auto-Device Registration**: Successful pairings don't automatically register devices

## Architecture Overview

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   CLI Commands  │───▶│  Daemon/Core    │───▶│ NetworkingService│
│                 │    │                 │    │                 │
│ network pair    │    │ start_pairing_* │    │ NEW: Pairing    │
│ generate/join   │    │ join_pairing_*  │    │ Integration     │
└─────────────────┘    └─────────────────┘    └─────────────────┘
                                                        │
                                              ┌─────────▼─────────┐
                                              │ LibP2PPairingProto│
                                              │ (Ephemeral Pairing)│
                                              └─────────┬─────────┘
                                                        │
                                                        ▼
                                              ┌─────────────────┐
                                              │ PersistentConn  │
                                              │ Manager         │
                                              │ (Device Storage)│
                                              └─────────────────┘
```

## Implementation Plan

### Phase 1: NetworkingService Pairing Integration

**File**: `src/infrastructure/networking/persistent/service.rs`

Add pairing methods that bridge LibP2PPairingProtocol with persistent infrastructure:

```rust
impl NetworkingService {
    /// Start pairing as initiator with persistence integration
    pub async fn start_pairing_as_initiator(&self, auto_accept: bool) -> Result<PairingSession> {
        // 1. Create LibP2PPairingProtocol using existing libp2p swarm
        // 2. Start pairing process
        // 3. Return PairingSession for status tracking
        // 4. On completion → automatically call add_paired_device()
    }

    /// Join pairing session with persistence integration
    pub async fn join_pairing_session(&self, code: String) -> Result<()> {
        // 1. Use existing libp2p connection infrastructure
        // 2. Create LibP2PPairingProtocol as joiner
        // 3. On success → add_paired_device() automatically
        // 4. Trigger immediate persistent connection attempt
    }

    /// Get status of active pairing sessions
    pub fn get_pairing_status(&self) -> Vec<PairingSessionStatus> {
        // Return current pairing session states
    }

    /// Cancel active pairing session
    pub async fn cancel_pairing(&self, session_id: Uuid) -> Result<()> {
        // Clean up pairing session
    }
}

/// Tracks active pairing sessions
pub struct PairingSession {
    pub id: Uuid,
    pub code: String,
    pub expires_at: DateTime<Utc>,
    pub role: PairingRole, // Initiator or Joiner
    pub status: PairingStatus,
}

pub enum PairingRole {
    Initiator,
    Joiner,
}

pub enum PairingStatus {
    WaitingForConnection,
    Connected,
    Authenticating,
    Completed,
    Failed(String),
    Cancelled,
}
```

### Phase 2: Core Method Implementation

**File**: `src/lib.rs`

Remove TODO stubs and implement actual pairing logic:

```rust
impl Core {
    pub async fn start_pairing_as_initiator(&self, auto_accept: bool) -> Result<(String, u32), Box<dyn std::error::Error>> {
        // Remove TODO
        let networking = self.networking.as_ref()
            .ok_or("Networking not initialized")?;

        let session = networking.start_pairing_as_initiator(auto_accept).await?;
        Ok((session.code, session.expires_in_seconds()))
    }

    pub async fn start_pairing_as_joiner(&self, code: String) -> Result<(), Box<dyn std::error::Error>> {
        // Remove TODO
        let networking = self.networking.as_ref()
            .ok_or("Networking not initialized")?;

        networking.join_pairing_session(code).await?;
        Ok(())
    }

    pub async fn get_pairing_status(&self) -> Result<Vec<PairingSessionStatus>, Box<dyn std::error::Error>> {
        // Remove TODO
        let networking = self.networking.as_ref()
            .ok_or("Networking not initialized")?;

        Ok(networking.get_pairing_status())
    }

    // ... other pairing methods
}
```

### Phase 3: Pairing-to-Persistence Bridge

**File**: `src/infrastructure/networking/persistent/pairing_bridge.rs` (NEW)

Create the critical bridge between ephemeral pairing and persistent connections:

```rust
/// Handles the transition from successful pairing to persistent device management
pub struct PairingBridge {
    connection_manager: Arc<PersistentConnectionManager>,
    identity: Arc<PersistentNetworkIdentity>,
}

impl PairingBridge {
    /// Called when LibP2PPairingProtocol completes successfully
    pub async fn on_pairing_complete(
        &self,
        remote_device: RemoteDevice,
        session_keys: SessionKeys,
        peer_id: PeerId,
    ) -> Result<()> {
        // 1. Create PairedDeviceRecord
        let device_record = PairedDeviceRecord {
            device_id: remote_device.id,
            device_name: remote_device.name,
            trust_level: TrustLevel::Trusted,
            auto_connect: true,
            session_keys: EncryptedSessionKeys::encrypt(&session_keys, &self.identity.password_hash)?,
            paired_at: Utc::now(),
        };

        // 2. Store in persistent identity
        self.identity.add_paired_device(device_record).await?;

        // 3. Add to connection manager for immediate connection
        self.connection_manager.add_paired_device(
            remote_device.id,
            peer_id,
            session_keys,
            TrustLevel::Trusted,
        ).await?;

        // 4. Trigger immediate connection attempt
        self.connection_manager.connect_to_device(remote_device.id).await?;

        Ok(())
    }
}
```

### Phase 4: LibP2P Integration Strategy

**Integration Points**:

1. **Reuse Existing Swarm**: Don't create new libp2p instance, use NetworkingService's swarm
2. **Event Integration**: LibP2PPairingProtocol events flow through existing NetworkEvent system
3. **Protocol Registration**: Register pairing protocol with existing swarm behaviors

**Key Implementation Details**:

```rust
// In NetworkingService initialization
impl NetworkingService {
    pub async fn new(identity: PersistentNetworkIdentity, password: String) -> Result<Self> {
        // ... existing setup ...

        // Add pairing protocol to swarm
        let pairing_protocol = LibP2PPairingProtocol::new(
            &identity.network_identity,
            identity.local_device.clone(),
            identity.local_private_key.clone(),
            &password,
        ).await?;

        swarm.with_pairing(pairing_protocol);

        // ... rest of setup ...
    }
}
```

## Persistent Pairing Flow

### Successful Pairing Sequence

```
1. CLI: spacedrive --instance alice network pair generate --auto-accept
   ↓
2. Core: start_pairing_as_initiator(auto_accept=true)
   ↓
3. NetworkingService: start_pairing_as_initiator()
   ↓
4. LibP2PPairingProtocol: Generate code, start listening
   ↓
5. CLI: spacedrive --instance bob network pair join "code words"
   ↓
6. Core: start_pairing_as_joiner("code words")
   ↓
7. NetworkingService: join_pairing_session()
   ↓
8. LibP2PPairingProtocol: Connect, authenticate, exchange keys
   ↓
9. PairingBridge: on_pairing_complete()
   ├── Store device in PersistentNetworkIdentity
   ├── Add to PersistentConnectionManager
   └── Trigger immediate connection
   ↓
10. Both devices now persistently connected!
```

### Persistence Verification

```bash
# Test persistence (restart both instances)
spacedrive --instance alice stop
spacedrive --instance bob stop

# Restart - should auto-reconnect
spacedrive --instance alice start --enable-networking
spacedrive --instance bob start --enable-networking

# Verify persistent connection
spacedrive --instance alice network devices  # Shows bob (connected)
spacedrive --instance bob network devices    # Shows alice (connected)
```

## File Structure

```
src/infrastructure/networking/
├── persistent/
│   ├── service.rs              # Add pairing methods
│   ├── pairing_bridge.rs       # NEW: Pairing-to-persistence bridge
│   └── ...
├── pairing/
│   ├── protocol.rs             # Existing LibP2PPairingProtocol
│   └── ...
├── CLI_PAIRING_IMPLEMENTATION_PLAN.md  # This file
└── ...
```

## Testing Strategy

### Unit Tests

- PairingBridge device registration
- NetworkingService pairing method integration
- Core method completion

### Integration Tests

- Full pairing flow between two instances
- Persistence across daemon restarts
- Auto-reconnection verification
- Error handling (timeouts, invalid codes, network failures)

### Manual Testing Protocol

```bash
# 1. Start two instances
spacedrive --instance alice start --enable-networking --foreground &
spacedrive --instance bob start --enable-networking --foreground &

# 2. Initialize networking
spacedrive --instance alice network init --password "test123"
spacedrive --instance bob network init --password "test123"

# 3. Test pairing
spacedrive --instance alice network pair generate --auto-accept
spacedrive --instance bob network pair join "generated code"

# 4. Verify immediate connection
spacedrive --instance alice network devices
spacedrive --instance bob network devices

# 5. Test persistence (restart)
spacedrive --instance alice stop && spacedrive --instance bob stop
spacedrive --instance alice start --enable-networking
spacedrive --instance bob start --enable-networking

# 6. Verify auto-reconnection
spacedrive --instance alice network devices  # Should show bob
spacedrive --instance bob network devices    # Should show alice

# 7. Test protocol functionality
spacedrive --instance alice spacedrop send "test.txt" bob
```

## Success Criteria

1. ✅ **Pairing Works**: Two CLI instances can successfully pair
2. ✅ **Persistence**: Paired devices survive daemon restarts
3. ✅ **Auto-Reconnection**: Devices automatically reconnect when available
4. ✅ **Protocol Support**: Paired devices can use file transfer, Spacedrop, etc.
5. ✅ **Error Handling**: Proper error messages for common failure cases
6. ✅ **Multi-Instance**: Works with CLI's multi-instance architecture

## Implementation Order

1. **PairingBridge**: Create the persistence bridge module
2. **NetworkingService**: Add pairing methods with LibP2P integration
3. **Core**: Replace TODO stubs with NetworkingService calls
4. **Testing**: Verify full persistent pairing flow
5. **Error Handling**: Add comprehensive error handling and timeouts
6. **Documentation**: Update CLI documentation with persistence behavior

---

**Key Insight**: The demo proves LibP2PPairingProtocol works perfectly. The CLI implementation is about integrating it with the existing persistent networking infrastructure, not rebuilding pairing from scratch.
