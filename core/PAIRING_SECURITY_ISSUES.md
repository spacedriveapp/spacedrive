# Pairing Protocol Security Issues

## CRITICAL - Must Fix Immediately

### 1. Memory Exhaustion DoS Vulnerability
**Severity**: CRITICAL
**Location**: `mod.rs:704`, `mod.rs:647`
**Impact**: Attacker can crash the application by claiming 4GB message size

```rust
// VULNERABLE CODE
let msg_len = u32::from_be_bytes(len_buf) as usize;
let mut msg_buf = vec![0u8; msg_len];  // NO SIZE LIMIT!
```

**Fix**: Add maximum message size constant
```rust
const MAX_MESSAGE_SIZE: usize = 1024 * 1024; // 1MB
if msg_len > MAX_MESSAGE_SIZE {
    return Err(NetworkingError::Protocol("Message too large".to_string()));
}
```

---

### 2. Plaintext Storage of Cryptographic Secrets
**Severity**: CRITICAL
**Location**: `persistence.rs:118-161`
**Impact**: Filesystem access = complete security compromise

```rust
// VULNERABLE CODE
let json_data = serde_json::to_string_pretty(&persisted)  // Plaintext!
```

**Fix**: Encrypt sessions file or use platform keychain

---

### 3. Session State Split-Brain Condition
**Severity**: CRITICAL
**Location**: `joiner.rs:66-191`
**Impact**: Joiner completes pairing before confirming initiator received response

**Current Flow (BROKEN)**:
```
Joiner:
1. Generates shared secret (line 71)
2. Completes pairing in registry (line 126)
3. Marks session as Completed (line 175)
4. THEN sends response (line 989 in mod.rs)
   └─> If this fails, initiator never completes but joiner thinks it did
```

**Fix**: Only complete after receiving confirmation from initiator

---

### 4. Session Fixation via QR Codes
**Severity**: CRITICAL
**Location**: `types.rs:126-218`
**Impact**: Attacker can create QR code with controlled session_id

```rust
// Line 213: session_id from QR is trusted
session_id, // Use the session_id from the QR code
```

**Fix**: Always derive session_id from cryptographic secret

---

## HIGH - Important to Address

### 5. TOCTOU Race Condition in Session Creation
**Severity**: HIGH
**Location**: `mod.rs:254-279`
**Impact**: Concurrent session creation can corrupt state

```rust
// RACE CONDITION
let sessions = self.active_sessions.read().await;  // Check
if sessions.get(&session_id).is_some() { return Err(...); }
// Lock released - another thread could insert here!
let mut sessions = self.active_sessions.write().await; // Insert
sessions.insert(session_id, session);
```

**Fix**: Use single write lock with entry API

---

### 6. No Replay Attack Protection
**Severity**: HIGH
**Location**: `initiator.rs:34`
**Impact**: Captured challenge-response can be replayed

**Fix**: Add timestamp to challenges and track used challenges

---

### 7. No Key Derivation Function
**Severity**: HIGH
**Location**: `mod.rs:524-532`
**Impact**: Pairing code secret used directly as session key

```rust
// WEAK
Ok(pairing_code.secret().to_vec())  // Direct use!
```

**Fix**: Use HKDF with session-specific context

---

### 8. Mismatched Timeout Values
**Severity**: HIGH
**Location**: `mod.rs:368`, `types.rs:52`

```rust
// Session cleanup: 10 minutes
let timeout_duration = chrono::Duration::minutes(10);

// Code expiry: 5 minutes
expires_at: Utc::now() + chrono::Duration::minutes(5),
```

**Fix**: Align timeouts (use 5 minutes for both)

---

## MEDIUM

### 9. Unpredictable Pairing Code Selection
**Severity**: MEDIUM
**Location**: `mod.rs:233-237`

```rust
codes.values().last().cloned()  // HashMap order is random!
```

**Fix**: Use BTreeMap or track most recent explicitly

---

### 10. Overly Detailed Error Messages
**Severity**: MEDIUM
**Impact**: Reveals internal state to attackers

**Fix**: Generic errors for external-facing, detailed logs internally

---

### 11. Premature Connection Status
**Severity**: MEDIUM
**Location**: `initiator.rs:248-273`, `joiner.rs:140-163`

```rust
let simple_connection = ConnectionInfo {
    addresses: vec![],  // Empty! Not really connected
```

**Fix**: Only mark connected when real connection established

---

## LOW

### 12. Weak Cryptographic Validation
**Location**: `security.rs:50-55`
**Fix**: Check for more weak patterns beyond all-zeros

### 13. Verbose Production Logging
**Fix**: Reduce logging in production builds

### 14. Inconsistent Session ID Derivation
**Location**: `types.rs:59-92`
**Fix**: Clarify why session_id is re-derived in `from_session_id()`

---

## Fix Priority

1. **Phase 1 (Immediate)**: #1, #3, #5, #6
2. **Phase 2 (Important)**: #2, #4, #7, #8
3. **Phase 3 (Nice to have)**: #9, #10, #11, #12, #13, #14
