---
id: LSYNC-000
title: "Epic: Library-based Synchronization"
status: To Do
assignee: unassigned
priority: High
tags: [epic, sync, networking, library-sync]
whitepaper: Section 4.5.1
---

## Description

This epic covers the implementation of the "Library Sync" model, a novel approach to synchronization that leverages the VDFS index to avoid the complexities of CRDTs. It treats metadata (index, audit log) and file operations as separate, coordinated streams, enabling efficient and robust syncing between peers.
