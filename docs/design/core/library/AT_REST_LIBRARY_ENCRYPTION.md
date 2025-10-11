<!--CREATED: 2025-07-29-->
Of course. Here is a complete implementation guide in Markdown format that incorporates the whitepaper's requirements, the new configuration setting, and a detailed plan for implementation.

---

# Implementation Guide: Data Protection at Rest

This document outlines the technical strategy for implementing the "Data Protection at Rest" model as described in the Spacedrive V2 Whitepaper. The goal is to align the Rust codebase with the whitepaper's security-first principles, ensuring user data is always protected on disk.

As the whitepaper states, a core tenet is providing robust privacy:

> [cite\_start]"...the robust, privacy-preserving principles of local-first architecture, when engineered for scalability, can bridge the gap between consumer-friendly design and enterprise-grade requirements." [cite: 38]

This guide provides the necessary steps to implement encryption for the library database, thumbnail cache, and network identity, directly addressing the threat model of a compromised device:

> [cite\_start]"**Scenario 2: Stolen Laptop with Sensitive Photo Library**...SQLCipher encryption on the library database prevents access without the user's password...attacker cannot: - View photo thumbnails (encrypted in cache)" [cite: 587]

---

## 1\. Library Configuration (`library.json`)

To give users control, we will add an `encryption_enabled` setting to the `LibrarySettings`. This setting will be **enabled by default** for all new libraries.

### Proposed Change

Modify the `LibrarySettings` struct in `src/library/config.rs`:

```rust
// [Source: 1056]
// src/library/config.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibrarySettings {
    // ... existing fields
    pub auto_track_external_volumes: bool,

    /// Whether the library is encrypted at rest
    pub encryption_enabled: bool,
}

// [Source: 1058]
impl Default for LibrarySettings {
    fn default() -> Self {
        Self {
            // ... existing defaults
            auto_track_system_volumes: true,
            auto_track_external_volumes: false,
            encryption_enabled: true, // Enabled by default
        }
    }
}
```

---

## 2\. Master Key Derivation (PBKDF2)

A strong cryptographic key must be derived from the user's password to encrypt the library. We will use PBKDF2 as specified.

> [cite\_start]"User passwords are strengthened using PBKDF2 with 100,000+ iterations and unique salts per library, providing strong protection against brute-force attacks." [cite: 572]

### Implementation

1.  **Dependencies**:

    ```toml
    [dependencies]
    pbkdf2 = "0.12"
    sha2 = "0.10"
    rand = "0.8"
    hex = "0.4"
    ```

2.  **Key Derivation Logic**:
    A utility function will handle key derivation. A unique, randomly generated salt must be created for each new encrypted library and stored in its `library.json` file.

    ```rust
    use pbkdf2::{
        password_hash::{PasswordHasher, SaltString},
        Pbkdf2
    };
    use rand::rngs::OsRng;

    /// Derives a 256-bit (32-byte) key from a password and salt.
    fn derive_library_key(password: &str, salt_str: &str) -> Result<[u8; 32], Box<dyn std::error::Error>> {
        let password_bytes = password.as_bytes();
        let mut key = [0u8; 32];

        Pbkdf2.hash_password_customized(
            password_bytes,
            None, // Algorithm identifier
            None, // Version
            pbkdf2::Params { rounds: 100_000, output_length: 32 },
            salt_str
        )?.hash_bytes_into(&mut key)?;

        Ok(key)
    }

    /// Generates a new salt for a library.
    fn generate_salt() -> String {
        let salt = SaltString::generate(&mut OsRng);
        salt.to_string()
    }
    ```

---

## 3\. Database Encryption (SQLCipher)

The core metadata database will be encrypted using SQLCipher.

> [cite\_start]"Library databases employ SQLCipher for transparent encryption at rest." [cite: 569]

### Implementation

1.  **Dependencies**: The `rusqlite` crate must be configured with the `sqlcipher` feature.

    ```toml
    [dependencies]
    rusqlite = { version = "0.31", features = ["sqlcipher"] }
    ```

2.  **Connection Logic**: The `Database::open` and `Database::create` functions in `src/infrastructure/database/mod.rs` must be modified to handle a password. The derived key is passed to SQLCipher via a `PRAGMA` command.

    ```rust
    use rusqlite::{Connection, OpenFlags};
    use std::path::Path;

    /// Opens or creates an encrypted database connection.
    fn open_encrypted_db(path: &Path, key: &[u8; 32]) -> Result<Connection, Box<dyn std::error::Error>> {
        // 1. Format the key for the SQLCipher PRAGMA command.
        let key_hex = hex::encode(key);
        let pragma_key = format!("PRAGMA key = 'x''{}''", key_hex);

        // 2. Open the database connection.
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE)?;

        // 3. Set the key. This must be the first command executed.
        conn.execute_batch(&pragma_key)?;

        // 4. Verify the key. A test query will fail if the key is incorrect.
        conn.query_row("SELECT count(*) FROM sqlite_master;", [], |_| Ok(()))?;

        Ok(conn)
    }
    ```

---

## 4\. Thumbnail Cache Encryption

Because the thumbnail cache resides inside the library directory but outside the database file, each thumbnail must be individually encrypted.

> [cite\_start]An attacker with a stolen laptop "cannot: - View photo thumbnails (encrypted in cache)" [cite: 587]

### Implementation

1.  **Strategy**: Use the same derived library key to encrypt each thumbnail file using an AEAD cipher like ChaCha20-Poly1305. Store a unique nonce with each file.

2.  **Dependencies**:

    ```toml
    [dependencies]
    chacha20poly1305 = "0.10"
    ```

3.  **Modify `Library::save_thumbnail`**: Encrypt thumbnail data before writing to disk.

    ```rust
    // In src/library/mod.rs
    use chacha20poly1305::{aead::{Aead, KeyInit}, ChaCha20Poly1305, Nonce};

    // Assume `key` is the 32-byte library key held in the Library struct.
    pub async fn save_thumbnail(&self, cas_id: &str, size: u32, data: &[u8], key: &[u8; 32]) -> Result<()> {
        let path = self.thumbnail_path(cas_id, size);

        let cipher = ChaCha20Poly1305::new(key.into());
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng); // Generate a unique nonce

        let ciphertext = cipher.encrypt(&nonce, data)
            .map_err(|e| LibraryError::Other(format!("Encryption failed: {}", e)))?;

        // Prepend the nonce to the ciphertext for storage
        let mut file_content = nonce.to_vec();
        file_content.extend_from_slice(&ciphertext);

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(path, &file_content).await?;

        Ok(())
    }
    ```

4.  **Modify `Library::get_thumbnail`**: Decrypt thumbnail data after reading from disk.

    ```rust
    // In src/library/mod.rs

    // Assume `key` is the 32-byte library key held in the Library struct.
    pub async fn get_thumbnail(&self, cas_id: &str, size: u32, key: &[u8; 32]) -> Result<Vec<u8>> {
        let path = self.thumbnail_path(cas_id, size);
        let encrypted_content = tokio::fs::read(path).await?;

        if encrypted_content.len() < 12 {
            return Err(LibraryError::Other("Invalid encrypted thumbnail file".to_string()));
        }

        // Split the nonce (first 12 bytes) from the ciphertext
        let (nonce_bytes, ciphertext) = encrypted_content.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt
        let cipher = ChaCha20Poly1305::new(key.into());
        let decrypted_data = cipher.decrypt(nonce, ciphertext)
            .map_err(|e| LibraryError::Other(format!("Decryption failed: {}", e)))?;

        Ok(decrypted_data)
    }
    ```

---

## 5\. Device Identity Encryption

To protect against network impersonation, the device's private network key must be encrypted at rest, unlocked by a master user password.

> [cite\_start]"Network identity protection employs a layered approach: Ed25519 private keys are encrypted using ChaCha20-Poly1305 with keys derived through Argon2id from user passwords." [cite: 573]

This process is similar to library encryption but uses **Argon2id** for key derivation (stronger against GPU cracking) and applies to a global `device.json` configuration file, not a per-library config.

---

## 6\. Performance Considerations

Implementing at-rest encryption introduces a deliberate performance trade-off for enhanced security.

- **One-Time Costs**: The expensive key derivation functions (**PBKDF2** for libraries, **Argon2id** for the device identity) are executed only once upon unlock or application startup. This adds a slight, intentional delay to these initial operations.
- **Continuous Costs**:
  - **Database**: Every database read/write incurs the overhead of AES encryption/decryption by **SQLCipher**. This will primarily affect I/O-heavy operations like mass indexing and complex searches.
  - **Thumbnails**: Every thumbnail access will incur the overhead of **ChaCha20-Poly1305** decryption. This may add minor latency to UI interactions that load many images at once.

This performance impact is a fundamental aspect of the security model and is necessary to fulfill the privacy-preserving promises of the whitepaper.
