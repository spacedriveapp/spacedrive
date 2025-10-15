---
id: CLOUD-002
title: Asynchronous Relay Server
status: To Do
assignee: james
parent: CLOUD-000
priority: High
tags: [cloud, networking, relay, sharing]
whitepaper: Section 5.3
---

## Description

Implement the relay server functionality that enables asynchronous communication between Spacedrive peers. This is critical for features like shareable links and asynchronous Spacedrop transfers, where peers may not be online at the same time.

## Implementation Steps

1.  Develop a standalone relay server application.
2.  The relay server should be able to store and forward messages for offline peers.
3.  Integrate the relay server with the core networking service.
4.  Implement the logic for clients to connect to and use the relay server when direct P2P connection is not possible.

## Acceptance Criteria

- [ ] The relay server can be deployed and run as a standalone service.
- [ ] Two peers can communicate asynchronously through the relay server.
- [ ] The system gracefully falls back to using the relay when direct connection fails.
