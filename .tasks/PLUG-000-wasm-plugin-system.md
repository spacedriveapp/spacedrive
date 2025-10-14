---
id: PLUG-000
title: "Epic: WASM Extension System"
status: In Progress
assignee: unassigned
priority: High
tags: [epic, plugins, wasm, extensibility, extensions]
whitepaper: Section 6.7
last_updated: 2025-10-14
---

## Description

This epic covers the implementation of the WebAssembly (WASM) based extension system. This allows third-party developers to extend Spacedrive's functionality in a secure and sandboxed environment, turning it into a true platform.

## Current Status

**Infrastructure:** Core WASM runtime is integrated and compiling. Extension SDK with beautiful `#[extension]` and `#[job]` macros is functional. Test extensions exist and compile to WASM.

**In Progress:** WASM memory interaction helpers, complete host function bridge, and production extensions (Photos, Finance, Email).

**Reference:** See `core/src/infra/extension/README.md` and `extensions/README.md` for implementation details.