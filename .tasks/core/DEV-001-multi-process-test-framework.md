---
id: DEV-001
title: Develop Multi-Process Test Framework
status: Done
assignee: jamiepine
parent: DEV-000
priority: High
tags: [testing, dev-infra, networking]
whitepaper: Section 6.3.1
---

## Description

A custom testing framework will be built to validate complex, multi-device distributed scenarios directly within the Rust test suite. It orchestrates multiple `cargo test` subprocesses, each assuming a different device "role," to simulate real-world P2P interactions like pairing and file transfers.

## Implementation Notes
-   The core runner will be implemented in `src/test_framework/runner.rs`.
-   Tests like `device_pairing_test.rs` and `cross_device_copy_test.rs` use this framework by defining distinct, `#[ignore]`-ed test functions for each role (e.g., `alice_pairing_scenario`, `bob_pairing_scenario`).
-   The main test function acts as an orchestrator, spawning the subprocesses and coordinating their interaction using the filesystem for signaling.

## Acceptance Criteria
-   [x] The framework can spawn multiple, isolated `cargo test` subprocesses.
-   [x] Each subprocess can be assigned a unique role and data directory via environment variables.
-   [x] The framework can coordinate and validate the outcomes of distributed tests (e.g., successful pairing).
