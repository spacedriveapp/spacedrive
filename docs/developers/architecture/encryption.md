---
index: 10
---

# Encryption

Here you will find all information relevant to encryption and decryption within Spacedrive. This also documents single-file headers, and the information/values which may be stored.


## Headers

Headers store information that is critical to the decryption of the data. Most commonly, this consists of salts, nonces and version identifiers. Our header system allows us to optionally include other information, such as file metadata and preview media (video thumbnails and such).

### Structure

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