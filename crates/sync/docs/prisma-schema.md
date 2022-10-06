# Prisma Schema

`prisma-crdt` introduces new attributes that must be applied to fields and models via triple slash documentation comments.

_Sync ID_: As well as having a primary key - denoted in Prisma with the `@id` field attribute - `prisma-crdt` introduces another ID - the _Sync ID_.
A model's Sync ID defaults to its regular ID, and is what identifies a model inside a sync operation.
Regular IDs are often not suitable for use inside a sync operation, as they may not be unique when sent to other nodes - eg. autoincrementing IDs - so something more unique can used, like a UUID.

## Model Attributes

#### `@local`

Model that is entirely disconnected from sync.

_Arguments_

- `id` (optional): Scalar field to override the default Sync ID.

#### `@owned`

Model that is synced via replicating from its owner to all other nodes, with the other nodes treating the model's owner as its single source of truth.

_Arguments_

- `owner`: Field that identifies the owner of this model. If a scalar, will directly use that value in sync operations. If a relation, the Sync ID of the related model will be resolved for sync operations.
- `id` (optional): Scalar field to override the default Sync ID.

#### `@shared`

Model that is synced via updates on a per-field basis.

_Arguments_

- `id` (optional): Scalar field to override the default Sync ID.
- `create` (optional): How the model should be created.
  - `Unique` (default): Model can be created with many required arguemnts, but ID provided _must_ be unique across all nodes. Useful for Tags since their IDs are non-deterministic.
  - `Atomic`: Require the model to have no required arguments apart from ID and apply all create arguments as atomic updates. Necessary for models with the same ID that can be created on multiple nodes. Useful for Files since their ID is dependent on their content, and could be the same across nodes.

#### `@relation`

Similar to `@shared`, but identified by the two records that it relates. Sync ID is always the combination of `item` and `group`.

_Arguments_

- `item`: Field that identifies the item that the relation links to. Operates like the `owner` argument of `@owned`.
- `group`: Field that identifies the group that the item should be related to. Operates like the `owner` argument of `@owned`.

## Field Attributes

#### `@node`

A relation whose value is automatically set to the current node. This could be done manually, but `@node` allows `node_id` fields to not be stored in `OwnedOperationData`, but rather be resolved from the `node_id` field of a `CRDTOperation`, saving on bandwidth.
