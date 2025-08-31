---
id: SYNC-000
title: "Epic: VDFS-Powered Synchronization"
status: To Do
assignee: unassigned
priority: High
tags: [epic, core, sync, networking]
whitepaper: Section 4.5
---

## Description

This epic covers the implementation of a stateful, VDFS-powered synchronization engine. Unlike stateless tools, this system will leverage the central VDFS index to intelligently and efficiently keep locations in sync across devices. It will be built around a durable `SyncRelationship` entity that defines the behavior between a source and a target.
