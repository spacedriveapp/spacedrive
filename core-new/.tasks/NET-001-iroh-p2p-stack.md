---
id: NET-001
title: "Implement Unified P2P Stack with Iroh"
status: Done
assignee: james
parent: NET-000
priority: High
tags: [networking, iroh, p2p]
whitepaper: Section 4.5.2
---

## Description

The networking layer has been built using the Iroh library. A single, unified `NetworkingService` manages one Iroh endpoint that handles all P2P communication, including device discovery, pairing, and file transfers.

## Implementation Notes
-   The `NetworkingService` in `src/services/networking/core/mod.rs` encapsulates the Iroh `Endpoint`.
-   The `NetworkingEventLoop` processes all incoming connections and routes them to the appropriate protocol handler based on ALPNs.
-   Device identity is managed by `src/services/networking/utils/identity.rs`, which derives Iroh keys from the master device key.

## Acceptance Criteria
-   [x] The core has a single, unified `NetworkingService`.
-   [x] The service can bind to a port and establish an Iroh `Endpoint`.
-   [x] Device discovery on a local network is functional.
