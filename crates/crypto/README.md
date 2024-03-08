# Crypto

This crate contains Spacedrive's cryptographic modules.

This includes things such as:

- The key manager
- Encryption and decryption
- Encrypted file header formats (with extremely fast serialization and deserialization)
- Key hashing and derivation
- Keyring interfaces to access native OS keystores

It has support for the following cryptographic functions:

- `Argon2id`
- `Balloon` hashing
- `BLAKE3` key derivation
- `XChaCha20-Poly1305`
- `AES-256-GCM-SIV`

It aims to be (relatively) lightweight, easy to maintain and platform-agnostic where possible. It does contain some platform-specific code, although it's only built if the target matches.

## Features

A list of all features can be found below (NOTE: none of these features are enabled by default)

- `serde` - provides integration with `serde` and `serde_json`
<!-- - `uuid` - enables the `uuid` crate -->
- `tokio` - provides integration with the `tokio` crate
- `specta` - provides integration with the `specta` crate
- `bincode` - provides integration with the `bincode` crate (this will likely become part of the crate)
- `keyring` - provides a unified interface for interacting with OS-keyrings (currently only supports MacOS/iOS/Linux `keyutils`). `keyutils` is not persistent, so is best used in a headless server/docker environment, as keys are wiped on-reboot. The Secret Service API is not practically available in headless environments.
- `secret-service` - enables `keyring` but also enables the Secret Service API (a persistent keyring targeted at Gnome/KDE (via `gnome-keyring` and `kwallet` respectively)). Is a pretty heavy dependency.

## Security Notice

This crate has NOT received any security audit - however, a couple of our upstream libraries (provided by [RustCrypto](https://github.com/RustCrypto)) have.

You may find them below:

- AES-GCM and XChaCha20-Poly1305 audit by NCC group ([link](https://research.nccgroup.com/wp-content/uploads/2020/02/NCC_Group_MobileCoin_RustCrypto_AESGCM_ChaCha20Poly1305_Implementation_Review_2020-02-12_v1.0.pdf))

Breaking changes are very likely! Use at your own risk - no stability or security is guaranteed.
