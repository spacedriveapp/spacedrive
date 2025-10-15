---
id: PLUG-002
title: Define and Implement VDFS Plugin API Bridge
status: In Progress
assignee: james
parent: PLUG-000
priority: High
tags: [plugins, wasm, api, vdfs, wire]
whitepaper: Section 6.8
last_updated: 2025-10-14
---

## Description

Define and implement the VDFS Plugin API bridge. This will be a secure, capability-based API that exposes a subset of the VDFS functionality to the sandboxed WASM plugins.

The key architectural insight: expose ONE generic `spacedrive_call()` function that routes to the existing Wire operation registry, reusing all daemon RPC infrastructure.

## Implementation Steps

1.  Design the API, focusing on security and a minimal set of capabilities.
    - Generic host function design complete
    - Routes through RpcServer::execute_json_operation()
2.  Implement the host-side functions that will be exposed to the WASM guest modules.
    - Skeleton exists in core/src/infra/extension/host_functions.rs
    - Needs: WASM memory interaction, full Wire bridge, error handling
3.  Implement the guest-side bindings for the API.
    - spacedrive-sdk with #[extension], #[job] macros
    - Beautiful API in extensions/test-extension/
4.  Ensure that plugins can only access the data and functionality they have been granted permission for.
    - Permission system in core/src/infra/extension/permissions.rs
    - Rate limiting included

## Remaining Work

- [ ] Complete host_spacedrive_call() implementation
- [ ] Add WASM memory read/write helpers
- [ ] Connect to RpcServer::execute_json_operation()
- [ ] Add extension-specific operations (ai.ocr, credentials.store, vdfs.write_sidecar)
- [ ] End-to-end integration testing

## Acceptance Criteria

- [x] A clear API definition document is created.
- [ ] A plugin can call a host function to interact with the VDFS (e.g., read a file).
- [x] The API enforces the principle of least privilege.

## Implementation Files

- core/src/infra/extension/host_functions.rs - Host function skeleton
- core/src/infra/extension/permissions.rs - Capability-based security
- core/src/infra/extension/README.md - Architecture documentation
- extensions/spacedrive-sdk/ - Guest-side SDK (referenced)
