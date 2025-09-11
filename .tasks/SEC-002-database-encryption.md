---
id: SEC-002
title: SQLCipher for At-Rest Library Encryption
status: To Do
assignee: unassigned
parent: SEC-000
priority: High
tags: [security, database, core, encryption]
whitepaper: Section 8.1
---

## Description

Implement transparent, at-rest encryption for all library databases (`.sdlibrary/database.db`) using SQLCipher. Keys should be derived from a user-provided password using PBKDF2 to protect against brute-force attacks.

## Implementation Steps

1.  Integrate a `SQLCipher` compatible Rust crate (e.g., `sqlx-sqlcipher`).
2.  Modify the `Database::open` and `Database::create` methods to accept an optional password.
3.  Implement key derivation logic using `PBKDF2` with a unique, stored salt for each library.
4.  Develop the CLI/UI flow for prompting for and managing library passwords.

## Acceptance Criteria

- [ ] A new library created with a password has its `database.db` file encrypted.
- [ ] The application can successfully connect to and query an encrypted database with the correct password.
- [ ] An attempt to open an encrypted database without a password fails with a clear error.
