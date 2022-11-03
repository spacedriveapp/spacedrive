---
index: 10
---

# Key Manager

The key manager handles all keys used for encrypting and decrypting files/containers, and is essentially an entire password manager built into Spacedrive.

To function, it requires a master password (which is provided during library creation). Do **not** lose this key, as it is not recoverable.

The `keymount` and `keystore` refer to the area in memory where each key types are stored.

## Audits

NCC Group have audited the encryption libraries that we use, and those audits can be found [here](https://research.nccgroup.com/wp-content/uploads/2020/02/NCC_Group_MobileCoin_RustCrypto_AESGCM_ChaCha20Poly1305_Implementation_Review_2020-02-12_v1.0.pdf).

## Master Password

During library creation, the user will be provided with a master password.

Each time Spacedrive is started, this master password is required. It is subsequently used to decrypt a randomly generated key, to ensure the password is correct. This verification process is not perfect.

Support for changing a master password will be added at a later date, but for now it is not a priority.

## Performance

Designing the key manager, and our key system as a whole, was a difficult task. It needs to be three things: secure, performant, and not annoying for the user. The main way to do this is to implement a hierarchical system, similar to what we have done.

The master password is the heart of the operation, which is why it's generated for you. In the event of a master password compromise, it's safe to assume that all underlying data is decryptable. Over time, we will refine the system and minimize this risk as much as possible (possibly by making the user store their master password's salt separately).

If a key from the keystore is compromised, the damage is limited to the data encrypted with that specific key. In this event, it's probably best to decrypt all data with that key, and re-encrypt it with a new one. We eventually plan to make this extremely simple within Spacedrive.

## Cryptographic Hygiene

In order to securely zero memory, we use a crate called `zeroize`. This is used throughout the entire `crypto` crate.

We use the `Protected` wrapper for aiding the handling of sensitive information. This wrapper does not allow `Copy`, and automatically redacts stored information from debugging logs. It also implements zeroize-on-drop, meaning once the value goes out of scope/is dropped, the memory is securely zeroed out.

## Internal Architecture

The key manager is made up of a few parts: two `DashMap`'s, and two `Mutex<Option>`'s. Although this sounds very simple, it is a pretty complex system, and provides high levels of security.

Many functions within the key manager don't return values. This is by design, and prevents us from unnecessarily returning (potentially sensitive) information to a function that does not require it. The information may still be accessed, but with the use of a UUID and another function. This design also allows us to have tight control over accessing and logging, so we're able to provide information about when a key was last used and what for.

We chose `DashMap` for the key manager, as opposed to the standard `HashMap`. `DashMap` provides us with much better performance than alternatives, while also offering concurrency. It aims to be a direct replacement for `RwLock<HashMap<K, V, S>>`, and it fits our needs perfectly.

The key manager also stores the UUID for the default key, and the *hashed* master password. This allows us to near-instantly decrypt stored keys (although mounting still takes a while, depending on the parameters chosen).

## RNGs

Throughout every cryptographic function within Spacedrive that requires cryptographically-secure random values, we use the system's entropy along with `ChaCha20Rng`. More information on this RNG can be found [here](https://rust-random.github.io/rand/rand_chacha/struct.ChaCha20Rng.html).

This is used for nonce, salt and master key generation to ensure none of these values are predictable to an attacker.

## Key Encryption

Key encryption makes use of an LE31 STREAM construction. This means that the last 4 bytes of the nonce consist of a 31 bit little-endian counter, and a 1 bit "last block" flag, totalling 4 bytes. The nonce (n) is still the correct size for the AEAD, but we only have to generate (n - 4) due to the counter.

Each key has two completely randomly generated nonces, which are used to encrypt both the master key (different from the master password) and the key itself.

The hashed master password (the one provided by the user), is used to encrypt a master key (32-bytes generated with a CSPRNG). This master key is used to encrypt your plaintext key. We took this approach so keys themselves are encrypted with the highest possible entropy, and we can add support for changing the master password in the future.

## Available Configuration

Cryptographic agility is never really recommended, so we don't provide too many options in this regard.

For encryption, we currently offer: `XChaCha20-Poly1305` (as the default), and `AES-256-GCM`. `XChaCha20-Poly1305` was chosen as the default due to better performance overall, and much better performance and security on devices where cryptographic hardware acceleration is unavailable. You can read about AES cache timing attacks [here](https://cr.yp.to/antiforgery/cachetiming-20050414.pdf).

For hashing, we provide `Argon2id` at varying hashing levels (standard, hardened and paranoid). The internal `Argon2id` parameters are not set in stone yet, and may change.

## Content Salts

A content salt is specific to each key/parameter pair, and is stored within the library. When a key gets mounted, it is hashed with the content salt. We do this to prevent hashing the key each time it's going to be used.

All data encrypted with this key/parameter pair will be (indirectly) encrypted with this hashed key, which allows us to near-instantly access and show encrypted file metadata, preview media and even the data itself.

## Mounting

There are two types of key mounting: adding and mounting, and just mounting.

Adding and mounting first registers the key with the internal keystore, which securely generates things needed to store your key safely, before mounting the key.

Just mounting the key means exactly that - the key is mounted (decrypted in-memory, and then hashed with your content salt) before being stored within the keymount.

## Unmounting

Unmounting is relatively simple, and it just removes the key from the keymount. Unmounting also removes any in-memory data that was decrypted with the (now umounted) key.
