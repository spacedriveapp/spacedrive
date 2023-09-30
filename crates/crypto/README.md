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
- `AES-256-GCM`

It aims to be (relatively) lightweight, easy to maintain and platform-agnostic where possible. It does contain some platform-specific code, although it's only built if the target matches.

## Features

A list of all features can be found below (NOTE: none of these features are enabled by default)

- `serde` - provides integration with the `serde` and `serde_json` crates. this also enables header metadata
- `rspc` - provides integration with the `rspc` crate
- `keymanager` - provides an interface for handling the encryption, decryption, storage and derivation of passwords/keys. this enables the `os-keyrings` feature
- `os-keyrings` - provides a unified interface for interacting with OS-keyrings (currently only supports MacOS/iOS and Gnome/KDE (via `gnome-keyring` and `kwallet` respectively))

## Security Notice

This crate has NOT received any security audit - however, a couple of our upstream libraries (provided by [RustCrypto](https://github.com/RustCrypto)) have.

You may find them below:

- AES-GCM and XChaCha20-Poly1305 audit by NCC group ([link](https://research.nccgroup.com/wp-content/uploads/2020/02/NCC_Group_MobileCoin_RustCrypto_AESGCM_ChaCha20Poly1305_Implementation_Review_2020-02-12_v1.0.pdf))

Breaking changes are very likely! Use at your own risk - no stability or security is guaranteed.

## Security Policy

Please refer to the [security policy](../../SECURITY.md) for details and information on how to responsibly report a security vulnerability or issue.
