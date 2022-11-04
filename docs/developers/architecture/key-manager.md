---
index: 10
---

# Key Manager

The key manager handles all keys used for encrypting and decrypting files/containers, and is essentially an entire password manager built into Spacedrive.

To function, it requires a master password (which is provided during library creation). Do **not** lose this key, as it is not recoverable.

More in-depth cryptographic information can be found on the [encryption page](./encryption).

## Master Password

During library creation, the user will be provided with a master password.

Each time Spacedrive is started, this master password is required. It is subsequently used to decrypt a randomly generated key, to ensure the password is correct. This verification process is not perfect.

Support for changing a master password will be added at a later date, but for now it is not a priority.

## Performance

Designing the key manager, and our key system as a whole, was a difficult task. It needs to be three things: secure, performant, and not annoying for the user. The main way to do this is to implement a hierarchical system, similar to what we have done.

The master password is the heart of the operation, which is why it's generated for you. In the event of a master password compromise, it's safe to assume that all underlying data is decryptable. Over time, we will refine the system and minimize this risk as much as possible (possibly by making the user store their master password's salt separately).

If a key from the keystore is compromised, the damage is limited to the data encrypted with that specific key. In this event, it's probably best to decrypt all data with that key, and re-encrypt it with a new one. We eventually plan to make this extremely simple within Spacedrive.

## Internal Architecture

The key manager is made up of a few parts: two `DashMap`'s, and two `Mutex<Option>`'s. Although this sounds very simple, it is a pretty complex system, and provides high levels of security.

Many functions within the key manager don't return values. This is by design, and prevents us from unnecessarily returning (potentially sensitive) information to a function that does not require it. The information may still be accessed, but with the use of a UUID and another function. This design also allows us to have tight control over accessing and logging, so we're able to provide information about when a key was last used and what for.

We chose `DashMap` for the key manager, as opposed to the standard `HashMap`. `DashMap` provides us with much better performance than alternatives, while also offering concurrency. It aims to be a direct replacement for `RwLock<HashMap<K, V, S>>`, and it fits our needs perfectly.

The key manager also stores the UUID for the default key, and the *hashed* master password. This allows us to near-instantly decrypt stored keys (although mounting still takes a while, depending on the parameters chosen).

## Terminology

The `keymount` and `keystore` refer to the area in memory where each key types are stored.

Keystore contains fully encrypted keys, and associated information.

The keymount contains encrypted keys, but also hashed keys (hashed with the content salt).

## Key Encryption

Each key has two completely randomly generated nonces, which are used to encrypt both the master key (different from the master password) and the key itself.

The hashed master password (the one provided by the user), is used to encrypt a master key (32-bytes generated with a CSPRNG). This master key is used to encrypt your plaintext key. We took this approach so keys themselves are encrypted with the highest possible entropy, and we can add support for changing the master password in the future.

## Content Salts

A content salt is specific to each key/parameter pair, and is stored within the library. When a key gets mounted, it is hashed with the content salt. We do this to prevent hashing the key each time it's going to be used.

All data encrypted with this key/parameter pair will be (indirectly) encrypted with this hashed key, which allows us to near-instantly access and show encrypted file metadata, preview media and even the data itself.

## Mounting

There are two types of key mounting: adding and mounting, and just mounting.

Adding and mounting first registers the key with the internal keystore, which securely generates things needed to store your key safely, before mounting the key.

Just mounting the key means exactly that - the key is mounted (decrypted in-memory, and then hashed with your content salt) before being stored within the keymount.

## Unmounting

Unmounting is relatively simple, and it just removes the key from the keymount. Unmounting also removes any in-memory data that was decrypted with the (now umounted) key.
