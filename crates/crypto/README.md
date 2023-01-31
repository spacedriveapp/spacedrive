# Crypto

This crate contains Spacedrive's cryptographic modules.

This includes things such as:

* The key manager
* Encryption and decryption
* Encrypted file header formats (with extremely fast serialization and deserialization)
* Key hashing and derivation
* Keyring interfaces to access native OS keystores

It aims to be (relatively) lightweight, easy to maintain and platform-agnostic where possible. It does contain some platform-specific code, although it's only built if the target matches.
