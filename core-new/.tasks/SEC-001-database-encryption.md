---
id: SEC-001
title: Implement SQLCipher for At-Rest Library Encryption
status: To Do
assignee: james
parent: SEC-000
priority: High
tags: [security, database, core, encryption]
whitepaper: Section 8.1
---

## Description

This task involves integrating `SQLCipher` into the database layer to provide transparent, at-rest encryption for all library databases (`.sdlibrary/database.db`). This is a critical security feature outlined in the whitepaper.

## Implementation Steps

1.  **Integrate `sqlx-sqlcipher`:** Add the necessary crate and configure SeaORM to use a SQLCipher-compatible connection string.
2.  **Update Database Connection Logic:** Modify `Database::open` and `Database::create` to accept an optional password.
3.  **Key Derivation:** When a password is provided, derive the encryption key using `PBKDF2` with a unique salt per library, as specified in the whitepaper.
4.  **UI/CLI Hooks:** Add password prompts to the CLI commands for creating and opening libraries.

## Acceptance Criteria

-   A new library created with a password should have its `database.db` file be unreadable by standard SQLite tools.
-   The core application should be able to open and interact with the encrypted database when the correct password is provided.
-   Attempting to open an encrypted library without a password should result in a clear error.
