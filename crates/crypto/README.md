# Crypto

This crate contains Spacedrive's cryptographic modules.

This includes things such as:

- Encryption and decryption

It has support for the following cryptographic functions:

- `XChaCha20-Poly1305`

It aims to be (relatively) lightweight, easy to maintain and platform-agnostic where possible. It does contain some platform-specific code, although it's only built if the target matches.

## Security Notice

This crate has NOT received any security audit - however, a couple of our upstream libraries (provided by [RustCrypto](https://github.com/RustCrypto)) have.

You may find them below:

- XChaCha20-Poly1305 audit by NCC group ([link](https://research.nccgroup.com/wp-content/uploads/2020/02/NCC_Group_MobileCoin_RustCrypto_AESGCM_ChaCha20Poly1305_Implementation_Review_2020-02-12_v1.0.pdf))

Breaking changes are very likely! Use at your own risk - no stability or security is guaranteed.
