---
index: 12
---

# Sync

Spacedrive synchronizes data using a combination of master-slave replication and last-write-wins CRDTs,
with the synchronization method encoded into the Prisma schema using [record type attributes](#record-types).

In the cases where LWW CRDTs are used,
conflicts are resolved using a [Hybrid Logical Clock](https://github.com/atolab/uhlc-rs)
to determine the ordering of events.

We would be remiss to not credit credit [Actual Budget](https://actualbudget.com/)
with many of the CRDT concepts used in Spacedrive's sync system.

## Record Types

All data in a library conforms to one of the following types.
Each type uses a different strategy for syncing.

### Local Records

Local records exist entirely outside of the sync system.
They don't have Sync IDs and never leave the node they were created on.

Used for Nodes, Statistics, and Sync Events.

`@local`

### Owned Records

Owned records are only ever modified by the node they are created by,
so they can be synced in a master-slave fashion.
The creator of an owned record dictates the state of the record to other nodes,
who will simply accept new changes without considering conflicts.

File paths are owned records since they only exist on one node,
and that node can inform all other nodes about the correct state of the paths.

Used for Locations, Paths, and Volumes.

`@owned(owner: String, id?: String)`

- `owner` - Field that identifies the owner of this model.
  If a scalar, will directly use that value in sync operations.
  If a relation, the Sync ID of the related model will be resolved for sync operations.
- `id` - Scalar field to override the default Sync ID.

### Shared Records

Shared records encompass most data synced in the CRDT fashion.
Updates are applied per-field using a last-write-wins strategy.

Used for Objects, Tags, Spaces, and Jobs.

`@shared(create: SharedCreateType, id?: String)`

- `id` - Scalar field to override the default Sync ID.
- `create` - How the model should be created.
  - `Unique` (default): Model can be created with many required arguemnts,
    but ID provided _must_ be unique across all nodes.
    Useful for Tags since their IDs are non-deterministic.
  - `Atomic`: Require the model to have no required arguments apart from ID and apply all create arguments as atomic updates.
    Necessary for models with the same ID that can be created on multiple nodes.
    Useful for Objects since their ID is dependent on their content,
    and could be the same across nodes.

### Relation Records

Similar to shared records, but represent a many-to-many relation between two records.
Sync ID is the combination of `item` and `group` Sync IDs.

Used for TagOnFile and FileInSpace.

`@relation(item: String, group: String)`

- `item` - Field that identifies the item that the relation is connecting.
  Similar to the `owner` argument of `@owned`.
- `group` - Field that identifies the group that the item should be connected to.
  Similar to the `owner` argument of `@owned`.

## Other Prisma Attributes

`@node`

Indicates that a relation field should be set to the current node.
This could be done manually,
but `@node` allows `node_id` fields to be resolved from the `node_id` field of a `CRDTOperation`,
saving on bandwidth
