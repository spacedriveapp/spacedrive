---
index: 10
---

# Sync

Spacedrive synchronizes library data in realtime across the distributed network of Nodes.

Using a Unique Hybrid Logicial Clock for distributed time synchronization.

A combination of several property level CRDT types:

- **Local data** - migrations, statistics, sync events
- **Owned data** - locations, paths, volumes
- **Shared data** - objects, tags, spaces, jobs
- **Relationship data** - many to many tables

Built in Rust on top of Prisma, it uses the schema file to determine these sync rules.