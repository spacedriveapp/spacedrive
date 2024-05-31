# `sd-core-sync`

Spacedrive's sync system. Consumes types and helpers from `sd-sync`.

### Creating Records

Prepare a sync id by creating or obtaining its value,
and then wrapping it in the model's `SyncId` struct,
available at `prisma_sync::{model}::SyncId`.

Next, prepare the sync operations using some varaints of the `sync_entry` macros.
`sync_entry` and `option_sync_entry` take the value first, and then the path to the field's prisma module.
`sync_db_entry` and `option_sync_db_entry` take the same inputs, but additionally produce a prisma operation in a tuple with the sync operation, intended to be put into a `Vec` and unzipped.

Finally, use `sync.shared/relation_create` depending on if you're creating a standalone record or a relation between two records, and then write it to the database with `write_ops`.

```rs
let (sync_params, db_params): (Vec<_>, Vec<_>) = [
  sync_db_entry!(self.name, tag::name),
  sync_db_entry!(self.color, tag::color),
  sync_db_entry!(false, tag::is_hidden),
  sync_db_entry!(date_created, tag::date_created),
]
.into_iter()
.unzip();

sync.write_ops(
  db,
  (
    sync.shared_create(
      prisma_sync::tag::SyncId { pub_id },
      sync_params,
    ),
    db.tag().create(pub_id, db_params),
  ),
)
```

### Updating Records

This follows a similar process to creation, but with `sync.shared/relation_create`.

```rs
let (sync_params, db_params): (Vec<_>, Vec<_>) = [
  sync_db_entry!(name, tag::name),
  sync_db_entry!(color, tag::color),
]
.into_iter()
.unzip();

sync.write_ops(
  db,
  (
    sync.shared_update(prisma_sync::tag::SyncId { pub_id }, k, v),
    db.tag().update(tag::id::equals(id), db_params);
  )
)
```

### Deleting Records

This only requires a sync ID.

```rs
sync.write_op(
  db,
  sync.shared_delete(prisma_sync::tag::SyncId { pub_id }),
  db.tag().delete(tag::id::equals(id));
)
```

### Relation Records

Relations require sync IDs for both the item and the group being related together.
Apart from that they're basically the same as shared operations.

```rs
let (sync_params, db_params): (Vec<_>, Vec<_>) = [
  sync_db_entry!(date_created, tag_on_object::date_created)
]
.into_iter()
.unzip();

sync.write_ops(
  db,
  (
    sync.relation_create(
      prisma_sync::tag_on_object::SyncId {
        tag: prisma_sync::tag::SyncId { pub_id: tag_pub_id },
        object: prisma_sync::object::SyncId { pub_id: object_pub_id },
      },
      sync_params
    ),
    db.tag_on_object().create(
        object::id::equals(object_id),
        tag::id::equals(tag_id),
        db_params
    )
  )
)
```

### Setting Relation Fields

Setting relation fields requires providing the Sync ID of the relation.
Setting the relation field's scalar fields instead will not properly sync then relation,
usually because the scalar fields are local and disconnected from the Sync ID.

```rs
let (sync_params, db_params): (Vec<_>, Vec<_>) = [
	sync_db_entry!(
		prisma_sync::object::SyncId { pub_id: object_pub_id },
		file_path::object
	)
].into_iter().unzip();

sync.write_ops(
	db,
	(
		sync.shared_update(
			prisma_sync::file_path::SyncId {
				pub_id: file_path_pub_id
			},
			sync_params
		),
		db.file_path().update(
			file_path::id::equals(file_path_id),
			db_params
		)
	)
)
```
