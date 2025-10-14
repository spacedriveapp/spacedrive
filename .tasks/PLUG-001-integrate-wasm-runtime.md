---
id: PLUG-001
title: Integrate WASM Runtime
status: In Progress
assignee: unassigned
parent: PLUG-000
priority: High
tags: [plugins, wasm, runtime, wasmer]
whitepaper: Section 6.7
last_updated: 2025-10-14
---

## Description

Integrate a WebAssembly runtime (e.g., Wasmer or Wasmtime) into the Spacedrive core. This will be the foundation for running sandboxed plugin code.

## Implementation Steps

1.  Research and select the most suitable WASM runtime for the project's needs (performance, security, ease of use).
    - Selected: Wasmer 4.2
2.  Add the selected runtime as a dependency to the project.
    - Added to core/Cargo.toml with wasmer-middlewares
3.  Create a `PluginManager` service that can load and execute a simple WASM module.
    - Implemented in core/src/infra/extension/manager.rs
4.  Develop a basic "hello world" plugin to test the integration.
    - Test extension exists at extensions/test-extension/
    - Compiles to test_extension.wasm (254KB)

## Remaining Work

- [ ] Complete WASM memory interaction helpers (read_string_from_wasm, write_json_to_wasm)
- [ ] Integrate with guest allocator for memory management
- [ ] Add hot-reload support for development

## Acceptance Criteria
-   [x] A WASM runtime is successfully integrated into the Spacedrive core.
-   [x] The `PluginManager` can load and run a WASM module from a file.
-   [x] The "hello world" plugin executes successfully and returns the expected output.

## Implementation Files

- core/src/infra/extension/manager.rs - PluginManager
- core/src/infra/extension/README.md - Architecture and status
- extensions/test-extension/ - Working test extension