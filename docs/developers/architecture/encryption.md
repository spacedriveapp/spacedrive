---
index: 10
---

# Encryption

Here you will find all information relevant to encryption and decryption within Spacedrive. This also documents single-file headers, and the information/values which may be stored.

## Cryptography

We take a strong stance on cryptography within Spacedrive, and we aim to provide a user-friendly experience while remaining as secure as possible.

### In-depth Overview

Key encryption makes use of an LE31 STREAM construction. This means that the last 4 bytes of the nonce consist of a 31 bit little-endian counter, and a 1 bit "last block" flag, totalling 4 bytes. The nonce (n) is still the correct size for the AEAD, but we only have to generate (n - 4) due to the counter.

Each piece of data is read in `BLOCK_SIZE` (1MiB), as this offers the best performance/memory usage from our testing. If the read count is less than the block size, we encrypt the block as if it were the last, so the 1 bit "last block" flag gets enabled. This allows us to use the same stream reader everywhere, regardless of data size.

The file size gain with this `BLOCK_SIZE` is 16 bytes per 1MiB of data, which is extremely negligible. These additional 16 bytes are used for authenticating the ciphertext, to make sure it has not changed. This means the decrypted plaintext will **always** be identical to the original, provided decryption does not produce any errors. The algorithm used for this depends on the encryption algorithm selected, but it is either GHASH or Poly1305.

### RNGs

Throughout every cryptographic function within Spacedrive that requires cryptographically-secure random values, we use the system's entropy along with `ChaCha20Rng`. More information on this RNG can be found [here](https://rust-random.github.io/rand/rand_chacha/struct.ChaCha20Rng.html).

This is used for nonce, salt, master key and master passphrase generation to ensure none of these values are predictable to an attacker.

### Encryption and Hashing Algorithms

Cryptographic agility is never really recommended, so we don't provide too many options in this regard.

For encryption, we currently offer: `XChaCha20-Poly1305` (as the default), and `AES-256-GCM`. `XChaCha20-Poly1305` was chosen as the default due to better performance overall, and much better performance and security on devices where cryptographic hardware acceleration is unavailable. You can read about AES cache timing attacks [here](https://cr.yp.to/antiforgery/cachetiming-20050414.pdf). `XChaCha20-Poly1305` is also capable of encrypting more data than `AES-256-GCM`.

For hashing, we provide `Argon2id` at varying hashing levels (standard, hardened and paranoid). The internal `Argon2id` parameters are not set in stone yet, and may change. We chose `Argon2id` due to its memory-hardness, resistance to side channel attacks and resistance to TMTO attacks.

### Cryptographic Hygiene

In order to securely zero memory, we use a crate called `zeroize`. This is used throughout the entire `crypto` crate.

We use the `Protected` wrapper for aiding the handling of sensitive information. This wrapper does not allow `Copy`, and automatically redacts stored information from debugging logs. It also implements zeroize-on-drop, meaning once the value goes out of scope/is dropped, the memory is securely zeroed out.
### Libraries and Audits

We make use of [RustCrypto](https://github.com/RustCrypto)'s AEAD and hashing libraries, as they are created with security in mind.

NCC Group have audited the encryption libraries that we use, and those audits can be found [here](https://research.nccgroup.com/wp-content/uploads/2020/02/NCC_Group_MobileCoin_RustCrypto_AESGCM_ChaCha20Poly1305_Implementation_Review_2020-02-12_v1.0.pdf).

### AAD

We make use of AEADs for encryption, which means we are able to provide associated data while encrypting. This can be anything really, but the file will NOT decrypt without this information present.

The associated data is not encrypted, it is just authenticated during encryption and is required to be present during decryption. This allows us to implement specific checks, so we can ensure nothing has been tampered with.

We authenticate the associated data with *every* block of data, and this comes at no impactful performance cost.

AAD for encrypting within Spacedrive consists of the first 36 bytes of the header (including the padding). If this part of the header is tampered with, your file will not decrypt at all.

We only include the first 36 bytes of the header as the rest of the header is not static, and can change at any point. Keys may be changed, or preview media may be removed, so it's best to just use the critical parts that will not change.

## Headers

Headers store information that is critical to the decryption of the data. Most commonly, this consists of salts, nonces and version identifiers.

### Structure

The headers have support for lots of associated information. We can optionally store metadata and preview media within the header, and allow them to be accessible instantly.

Using multiple keyslots also allows us to support more than one key for decrypting a file, and you can change them too!

These structures will likely change multiple times before we officially set them in stone. Things like metadata/preview media length encryption are planned, as well as other header objects entirely.

The current header structure is as follows:

| Name               | Purpose                      | Size       |
|--------------------|------------------------------|------------|
| Magic Bytes        | To quickly identify the file | 7 bytes    |
| Header Version     |                              | 2 bytes    |
| Algorithm          | Encryption Algorithm         | 2 bytes    |
| Nonce              | Nonce used for the data      | 8/20 bytes |
| Padding            | To reach a total of 36 bytes | 5/17 bytes |
| Keyslot Area       | To store 2x keyslots         | 192 bytes  |
| Metadata Area      | To store metadata (optional) | Varies     |
| Preview Media Area | To store PVM (optional)      | Varies     |

The keyslot area:

| Name              | Purpose                      | Size       |
|-------------------|------------------------------|------------|
| Keyslot Version   |                              | 2 bytes    |
| Algorithm         | Encryption Algorithm         | 2 bytes    |
| Hashing Algorithm |                              | 2 bytes    |
| Salt              | Salt used for hashing        | 16 bytes   |
| Master Key        | (encrypted)                  | 48 bytes   |
| Nonce             | Nonce used for encrypting MK | 8/20 bytes |
| Padding           | To reach 96 total bytes      | 6/12 bytes |

The metadata area:

| Name             | Purpose                | Size       |
|------------------|------------------------|------------|
| Metadata Version |                        | 2 bytes    |
| Algorithm        | Encryption Algorithm   | 2 bytes    |
| Nonce            | Used for the data      | 8/20 bytes |
| Padding          | To reach 28 bytes      | 4/16 bytes |
| Length           | Length of the MD (u64) | 8 bytes    |
| Metadata         | (encrypted)            | Varies     |

The preview media area:

| Name        | Purpose                 | Size       |
|-------------|-------------------------|------------|
| PVM Version |                         | 2 bytes    |
| Algorithm   | Encryption Algorithm    | 2 bytes    |
| Nonce       | Used for the data       | 8/20 bytes |
| Padding     | To reach 28 bytes       | 4/16 bytes |
| Length      | Length of the PVM (u64) | 8 bytes    |
| Metadata    | (encrypted)             | Varies     |

### Additional Header Objects

Additional header objects, such as metadata and preview media, inherit the keyslot's master key. This is so that one hashed key is able to decrypt all data associated with the file, and provide immediate access to the user. This also means that changing a password will reflect upon all parts of the file, not just the data.

Metadata can be anything that implements `serde::Serialize`, and the size does not matter.

Preview media is intended to be a video thumbnail, or even a portion of a video. Raw bytes are the most suitable for this type, but storing something else here is not strictly forbidden (it is advised against, though).