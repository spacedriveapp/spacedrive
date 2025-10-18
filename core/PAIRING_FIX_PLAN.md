# Pairing Protocol - Comprehensive Fix Plan

## Overview

This plan addresses both **critical security vulnerabilities** and **protocol correctness issues** that cause test failures.

**Priority**: CRITICAL - Current system is broken and insecure

---

## Phase 1: Fix Critical Protocol Flow (Required for Tests to Pass)

### Issue #1: Bob Completes Pairing Too Early

**File**: `joiner.rs:17-211` (handle_pairing_challenge)

**Current (WRONG)**:

```rust
pub(crate) async fn handle_pairing_challenge(...) -> Result<Vec<u8>> {
    // Sign challenge
    let signature = self.identity.sign(&challenge)?;

    // Generate shared secret TOO EARLY
    let shared_secret = self.generate_shared_secret(session_id).await?;
    let session_keys = SessionKeys::from_shared_secret(shared_secret.clone());

    // Complete pairing BEFORE Alice confirms
    registry.complete_pairing(device_id, initiator_device_info.clone(), session_keys).await?;

    // Mark as connected BEFORE Alice confirms
    registry.mark_connected(device_id, simple_connection).await?;

    // Set state to Completed BEFORE receiving confirmation
    session.state = PairingState::Completed;

    // Send response
    let response = PairingMessage::Response { session_id, response: signature, device_info };
    Ok(serde_json::to_vec(&response)?)
}
```

**Fixed**:

```rust
pub(crate) async fn handle_pairing_challenge(...) -> Result<Vec<u8>> {
    // Sign challenge
    let signature = self.identity.sign(&challenge)?;

    // Store initiator info for later (when we receive Complete)
    {
        let mut sessions = self.active_sessions.write().await;
        if let Some(session) = sessions.get_mut(&session_id) {
            session.remote_device_info = Some(initiator_device_info.clone());
            session.state = PairingState::ResponseSent; // NOT Completed!
        }
    }

    // ONLY send response, don't complete anything yet
    let device_info = self.get_device_info().await?;
    let response = PairingMessage::Response {
        session_id,
        response: signature,
        device_info
    };

    Ok(serde_json::to_vec(&response)?)
}
```

**Changes**:

- Remove all lines 71-178 (shared secret, complete_pairing, mark_connected)
- Only store initiator_device_info for later use
- Transition to `ResponseSent` state (not `Completed`)
- Wait for `Complete` message before doing anything

### Issue #2: handle_completion is Redundant

**File**: `joiner.rs:214-411` (handle_completion)

**Current**: Does everything that handle_pairing_challenge already did

**Fixed**: This becomes the ONLY place Bob completes pairing

```rust
pub(crate) async fn handle_completion(...) -> Result<()> {
    if success {
        // NOW generate shared secret (not before!)
        let shared_secret = self.generate_shared_secret(session_id).await?;
        let session_keys = SessionKeys::from_shared_secret(shared_secret.clone());

        // Get initiator info we stored earlier
        let initiator_device_info = {
            let sessions = self.active_sessions.read().await;
            sessions.get(&session_id)
                .and_then(|s| s.remote_device_info.clone())
                .ok_or(NetworkingError::Protocol("No device info stored".to_string()))?
        };

        let device_id = initiator_device_info.device_id;

        // Register initiator in Pairing state
        registry.start_pairing(device_id, node_id, session_id, node_addr)?;

        // NOW complete pairing (Alice already confirmed!)
        registry.complete_pairing(device_id, initiator_device_info.clone(), session_keys).await?;

        // Mark as connected
        registry.mark_connected(device_id, simple_connection).await?;

        // Update session state
        session.state = PairingState::Completed;
        session.shared_secret = Some(shared_secret);
    }
    Ok(())
}
```

### Issue #3: Alice Must Guarantee Complete Send

**File**: `initiator.rs:120-295` (handle_pairing_response)

**Current**: Marks as completed, then sends Complete (might fail silently)

**Fixed**: Send Complete synchronously, fail if it doesn't work

```rust
pub(crate) async fn handle_pairing_response(...) -> Result<Vec<u8>> {
    // ... signature verification ...

    if !signature_valid {
        // Mark as failed
        session.state = PairingState::Failed { reason: "Invalid signature".to_string() };

        // Send failure Complete message
        let response = PairingMessage::Complete {
            session_id,
            success: false,
            reason: Some("Invalid signature".to_string()),
        };
        return serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e));
    }

    // Signature valid - complete pairing
    let shared_secret = self.generate_shared_secret(session_id).await?;
    let session_keys = SessionKeys::from_shared_secret(shared_secret.clone());

    registry.start_pairing(device_id, node_id, session_id, node_addr)?;
    registry.complete_pairing(device_id, device_info.clone(), session_keys).await?;
    registry.mark_connected(device_id, simple_connection).await?;

    // Update session BEFORE sending Complete
    session.state = PairingState::Completed;
    session.shared_secret = Some(shared_secret);

    // Send success Complete message
    let response = PairingMessage::Complete {
        session_id,
        success: true,
        reason: None,
    };

    serde_json::to_vec(&response).map_err(|e| NetworkingError::Serialization(e))
}
```

**Key Change**: If Complete message fails to send, the error propagates and Bob never receives confirmation.

---

## Phase 2: Fix Critical Security Issues

### Issue #4: DoS via Unbounded Message Size

**File**: `mod.rs:704`, `mod.rs:647`

**Add constant**:

```rust
// At top of mod.rs
const MAX_MESSAGE_SIZE: usize = 1024 * 1024; // 1MB max
```

**Fix both locations**:

```rust
// Line 647 (send_pairing_message_to_node)
let mut len_buf = [0u8; 4];
match recv.read_exact(&mut len_buf).await {
    Ok(_) => {
        let resp_len = u32::from_be_bytes(len_buf) as usize;

        // Validate size
        if resp_len > MAX_MESSAGE_SIZE {
            return Err(NetworkingError::Protocol(
                format!("Message too large: {} bytes", resp_len)
            ));
        }

        let mut resp_buf = vec![0u8; resp_len];
        // ... rest of code ...
    }
}

// Line 704 (handle_stream)
let msg_len = u32::from_be_bytes(len_buf) as usize;

// Validate size
if msg_len > MAX_MESSAGE_SIZE {
    self.logger.error(&format!("Rejecting oversized message: {} bytes", msg_len)).await;
    break;
}

let mut msg_buf = vec![0u8; msg_len];
```

### Issue #5: Replay Attack Protection

**File**: `initiator.rs:34`, `mod.rs:516`

**Add challenge tracking**:

```rust
// In PairingProtocolHandler struct
used_challenges: Arc<RwLock<HashMap<Vec<u8>, chrono::DateTime<Utc>>>>,
```

**Generate challenge with timestamp**:

```rust
fn generate_challenge(&self) -> Result<Vec<u8>> {
    use rand::RngCore;
    let mut challenge = vec![0u8; 40]; // 32 random + 8 timestamp
    rand::thread_rng().fill_bytes(&mut challenge[0..32]);

    // Add timestamp
    let timestamp = chrono::Utc::now().timestamp();
    challenge[32..40].copy_from_slice(&timestamp.to_be_bytes());

    Ok(challenge)
}
```

**Verify challenge hasn't been used**:

```rust
// In handle_pairing_response, BEFORE verifying signature
let challenge_timestamp = {
    if challenge.len() != 40 {
        return Err(NetworkingError::Protocol("Invalid challenge format".to_string()));
    }
    let ts_bytes: [u8; 8] = challenge[32..40].try_into().unwrap();
    let timestamp = i64::from_be_bytes(ts_bytes);
    chrono::DateTime::from_timestamp(timestamp, 0)
        .ok_or(NetworkingError::Protocol("Invalid challenge timestamp".to_string()))?
};

// Check if challenge is too old (> 5 minutes)
let now = chrono::Utc::now();
if now.signed_duration_since(challenge_timestamp) > chrono::Duration::minutes(5) {
    return Err(NetworkingError::Protocol("Challenge expired".to_string()));
}

// Check if challenge was already used
{
    let mut used = self.used_challenges.write().await;
    if used.contains_key(&challenge) {
        return Err(NetworkingError::Protocol("Challenge already used (replay attack?)".to_string()));
    }
    used.insert(challenge.clone(), now);

    // Cleanup old challenges (> 10 minutes)
    used.retain(|_, &mut used_at| {
        now.signed_duration_since(used_at) < chrono::Duration::minutes(10)
    });
}
```

### Issue #6: Use Proper KDF for Session Keys

**File**: `mod.rs:524-532`

**Current (WRONG)**:

```rust
async fn generate_shared_secret(&self, session_id: Uuid) -> Result<Vec<u8>> {
    let pairing_codes = self.pairing_codes.read().await;
    let pairing_code = pairing_codes.get(&session_id).ok_or_else(|| {
        NetworkingError::Protocol(format!("No pairing code found for session {}", session_id))
    })?;
    Ok(pairing_code.secret().to_vec()) // Direct use!
}
```

**Fixed (use HKDF)**:

```rust
async fn generate_shared_secret(&self, session_id: Uuid) -> Result<Vec<u8>> {
    let pairing_codes = self.pairing_codes.read().await;
    let pairing_code = pairing_codes.get(&session_id).ok_or_else(|| {
        NetworkingError::Protocol(format!("No pairing code found for session {}", session_id))
    })?;

    // Use HKDF with session-specific context
    use blake3::Hasher;
    let mut hasher = Hasher::new();
    hasher.update(b"spacedrive-pairing-session-key-v1");
    hasher.update(session_id.as_bytes());
    hasher.update(pairing_code.secret());

    Ok(hasher.finalize().as_bytes().to_vec())
}
```

### Issue #7: Fix TOCTOU Race in Session Creation

**File**: `mod.rs:254-279`

**Current (WRONG)**:

```rust
// Line 256: Check (read lock)
let sessions = self.active_sessions.read().await;
if let Some(existing_session) = sessions.get(&session_id) {
    return Err(...);
}
// Lock released - RACE CONDITION HERE!

// Line 277: Insert (write lock)
let mut sessions = self.active_sessions.write().await;
sessions.insert(session_id, session);
```

**Fixed**:

```rust
// Single write lock for atomic check-and-insert
let mut sessions = self.active_sessions.write().await;
if sessions.contains_key(&session_id) {
    return Err(NetworkingError::Protocol(format!(
        "Session {} already exists",
        session_id
    )));
}

// Create new session
let session = PairingSession {
    id: session_id,
    state: PairingState::Scanning,
    remote_device_id: None,
    remote_device_info: None,
    remote_public_key: None,
    shared_secret: None,
    created_at: chrono::Utc::now(),
};

sessions.insert(session_id, session);
```

### Issue #8: Align Timeout Values

**File**: `mod.rs:368`, `types.rs:52`

**Fix**:

```rust
// types.rs:52 - Keep 5 minutes
expires_at: Utc::now() + chrono::Duration::minutes(5),

// mod.rs:368 - Change from 10 to 5 minutes
let timeout_duration = chrono::Duration::minutes(5); // Match code expiry
```

---

## Phase 3: Fix Medium/Low Priority Issues

### Issue #9: Fix get_current_pairing_code

**File**: `mod.rs:233-237`

**Replace HashMap with BTreeMap** or track most recent explicitly:

```rust
// Option A: Use BTreeMap (ordered by insertion)
// In struct: pairing_codes: Arc<RwLock<BTreeMap<Uuid, PairingCode>>>,

// Option B: Track most recent explicitly
// In struct: most_recent_pairing_code: Arc<RwLock<Option<PairingCode>>>,

pub async fn get_current_pairing_code(&self) -> Option<PairingCode> {
    self.most_recent_pairing_code.read().await.clone()
}

// Update when creating pairing code:
*self.most_recent_pairing_code.write().await = Some(pairing_code.clone());
```

### Issue #10: Sanitize Error Messages

**File**: Throughout

**Pattern**:

```rust
// Don't expose internal state
format!("Session {} already exists in state {:?}", session_id, existing_session.state)

// Generic error for external, detailed log for internal
self.log_error(&format!("Session {} already exists in state {:?}", session_id, existing_session.state)).await;
return Err(NetworkingError::Protocol("Session already exists".to_string()));
```

### Issue #11: Reduce Verbose Logging in Production

**File**: All files

Add log levels:

```rust
// Use debug! for verbose info
self.log_debug(&format!("Session state: {:?}", session.state)).await;

// Only info! for important events
self.log_info("Pairing completed successfully").await;
```

### Issue #12: Encrypt Persisted Sessions

**File**: `persistence.rs:118-161`

**Add encryption**:

```rust
// Use platform keychain to store encryption key
// Encrypt JSON before writing to disk
// Decrypt when reading

// Example using age or similar:
let encrypted_data = encrypt_with_platform_key(&json_data)?;
fs::write(&self.sessions_file, encrypted_data).await?;
```

---

## Implementation Order

### Step 1: Protocol Flow Fixes (REQUIRED FOR TESTS)

1. Fix `handle_pairing_challenge` to NOT complete pairing early
2. Fix `handle_completion` to be the ONLY place Bob completes
3. Fix `handle_pairing_response` to guarantee Complete send
4. Test: `cargo test device_pairing_test` should PASS

### Step 2: Critical Security (DO NOT SHIP WITHOUT)

5. Add message size limits (DoS fix)
6. Add replay attack protection
7. Fix TOCTOU race condition
8. Add proper KDF for session keys

### Step 3: Important Security

9. Encrypt persisted sessions
10. Align timeout values
11. Fix session_id derivation consistency (types.rs)

### Step 4: Polish

12. Fix get_current_pairing_code
13. Sanitize error messages
14. Reduce verbose logging
15. Improve cryptographic validation

---

## Testing Plan

### Unit Tests

```rust
#[tokio::test]
async fn test_challenge_replay_protection() {
    // Generate challenge
    // Use it once ✓
    // Try to reuse it ✗ should fail
}

#[tokio::test]
async fn test_message_size_limit() {
    // Try to send 5GB message
    // Should fail with "Message too large"
}

#[tokio::test]
async fn test_joiner_waits_for_complete() {
    // Bob sends Response
    // Bob session should be ResponseSent (NOT Completed)
    // Alice sends Complete
    // NOW Bob session should be Completed
}
```

### Integration Test

```bash
# Should pass after Phase 1
cargo test device_pairing_test
```

---

## Success Criteria

**Protocol Correctness**:

- Test `device_pairing_test` passes 100% of the time
- No split-brain states possible
- Both Alice and Bob atomically complete pairing

**Security**:

- No DoS via large messages
- No replay attacks possible
- Proper mutual authentication
- Session keys properly derived
- Secrets encrypted at rest

**Code Quality**:

- No redundant logic
- Clear state machine
- Proper error handling
- Reasonable logging

---

## Estimated Effort

- **Phase 1**: 4-6 hours (protocol flow fixes)
- **Phase 2**: 4-6 hours (critical security)
- **Phase 3**: 2-4 hours (important security)
- **Phase 4**: 2-3 hours (polish)
- **Total**: ~12-19 hours

---

## Risk Assessment

**High Risk if NOT fixed**:

- Split-brain pairing states
- DoS attacks crash daemon
- Replay attacks compromise security
- Plaintext secrets on disk

**Low Risk when fixed**:

- Well-tested protocol
- Defense in depth
- Cryptographic guarantees
- Production-ready security
