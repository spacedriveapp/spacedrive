# Distributed Data Sync

Synchronizing data between clients in a Spacedrive network is accomplished using various forms of [CRDTs](https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type) combined with a hybrid logical clock, ensuring eventual constancy.

Designed for synchronizing data in realtime between [SQLite](https://www.sqlite.org/) databases potentially in the gigabytes.

```rust
// we can now impl specfic CRDT traits to given resources
enum SyncResource {
  FilePath(dyn Replicate),
  File(dyn PropertyOperation),
  Tag(dyn PropertyOperation),
  TagOnFile(dyn LastWriteWin),
  Jobs(dyn Replicate + OperationalTransform)
}
```

## Data Types

Data is divided into several kinds, Shared and Owned.

- **Shared data** - Can be created and modified by any client. Has a `uuid`.

  _Sync Method:_ `Property operation*`

  > Shared resources could be,`files`, `tags`, `notes`, `albums` and `labels`. Since these can be created, updated or deleted by any client at any time.

- **Owned data** - Can only be modified by the client that created it. Has a `client_id` and `uuid`.

  _Sync Method:_ `Replicate`

  > Owned resources would be `file_paths`, `jobs`, `locations` and `media_data`, since a client is the single source of truth for this data. This means we can perform conflict free synchronization.

\*_Shared data doesn't always use this method, in some cases we can create shared resources in bulk, where conflicts are handled by simply merging. More on that in [Synchronization Strategy]()_.

## Node Pool

The node pool maintains record of all nodes in your network.

An exact replica of the client pool is synchronized on each client. When a given client has a state change, it will notify every other client in the pool via the `connection` struct.

The `ClientConnection` is maintained in memory and is established on startup.

```rust
struct NodePool {
  clients: Vec<Client>
}

struct Node {
  uuid: String,
  last_seen: DateTime<Utc>,
  last_synchronized: DateTime<Utc>,
  connection: Option<NodeConnection>
}
```

Nodes will ping-pong to ensure their connection stays alive, this logic is contained within the `NodeConnection` instance.

**Handling stale nodes**

If a node has not been seen in X amount of time, other nodes will not persist pending operations for them. Nodes take care of flushing the pending operation queue once all non-stale nodes have received the pending operations.

## Clock

With realtime synchronization it is important to maintain the true order of events, we can timestamp each operation, but have to account for time drift; there is no way to guarantee two machines have synchronized system clocks.

We can solve this with a Unique Hybrid Logical Clock ([UHLC]()): a globally-unique, monotonic timestamp.

```
2022-04-09T06:53:36.397295996Z/89F9DD8514914648989315B2D30D7BE5
```

Each client combines their hybrid time with a unique identifier. When receiving new [Sync Events](), a client will update its own clock with the incoming timestamp.

A client will reject operations with a timestamp drift greater than 100ms (can be adjusted).

This allows us to entirely avoid the need to synchronize time between clients, as each client controls its own order of operations, never producing a conflicting timestamp with another system in the network.

## Synchronization Strategy

Sync happens in the following order:

Owned data → Bulk shared data → Shared data

### Types of CRDT:

```rust
trait PropertyOperation;

trait Replicate;
```

- **PropertyOperation** - Update Shared resources at a property level. Operations stored in `pending_operations` table.
- **Replicate** - Used exclusively for Owned data, clients will replicate with no questions asked.

- ~~**Last Write Win** - The most recent event will always be applied, used for many-to-many datasets.~~

## Operations

Operations perform a Shared data change, they are cached in the database as `pending_operations`.

Operations are removed once all online clients have received the payload.

```rust
struct PropertyOperation<V> {
  method: OperationMethod,
  // the name of the database table
  resource_type: String,
  // the unique identifier of the resource (None for batched)
  resource_uuid: String,
  // the property on the resource whose value shall be affected
  resource_property: Option<String>
  // optional value for operation
  value: Option<Box<V>>,
}

enum OperationMethod {
  Create,
  Update
  Delete
}

```

## Pending operations

Here are some examples of how operations are stored to minimize disk usage and data duplication.

**Create operation for Shared data**

In the next case we're handling the creation of a Shared resource. The `method` is marked `Create` and the value is `NULL`. This is because we can also use the actual database record in the `tags` table as it was newly created.

| `client_uuid` | `uhlc_timestamp`       | `method` | `resource_key` | `resource_uuid` | `resource_property` | `value` |
| ------------- | ---------------------- | -------- | -------------- | --------------- | ------------------- | ------- |
| 2e8f85bf...   | 2022-04-09T06:53:36... | Create   | tags           | 2e8f85bf...     | NULL                | NULL    |

**Update operation for Shared data**

Shared data works at a property level

| `client_uuid` | `uhlc_timestamp`       | `method` | `resource_key` | `resource_uuid` | `resource_property` | `value` |
| ------------- | ---------------------- | -------- | -------------- | --------------- | ------------------- | ------- |
| 2e8f85bf...   | 2022-04-09T06:53:36... | Update   | albums         | 2e8f85bf...     | name                | "jeff"  |

## Owned Data Synchronization

Owned data does not use the Operation system, it is queried dynamically by the `updated_at` column on Owned datasets.

For the sake of compatibility with local relations, some resource properties can be ignored\*, such as `file_id` and `parent_id` on the `file_paths` resource, these are re-calculated on bulk ingest.

\*_This will require some form of definition when creating an owned data resource_.

## Bulk Shared Data Synchronization

In some cases we are able to create many shared data resources at once and resolve conflicts on the fly by merging where the oldest resource takes priority.

This is intended for the `files` resource. It requires Shared data behavior as most other shared resources are related at a database level and user defined metadata can be assigned, however it is initially derived from `file_paths` which is Owned data.

As `files` are created in abundance (hundreds of thousands at a time), it would be inefficient to record these changes in the `pending_operations` table. But we are also unable to sync in the same way as Owned data due to the possibility of conflicts.

We handle this by using `SyncMethod::Merge`, simply merging the data where the oldest resource properties are prioritized.

## Combining CRDTs

Combining CRDT types allow for some tailored functionality for particular resources.

Looking at the `jobs` resource let look how `OperationalTransform + Replicate` might work.

Jobs are unique in that they have frequent updates to some properties and

```rust
impl OperationalTransform for Job {
  pub fn create () {}
  pub fn update () {}
  pub fn delete () {}
}

impl Replicate for Job {

}
```

## Creating Sync Events

We have a simple Rust syntax for creating sync events in the core.

```rust
aysnc fn my_core_function(&ctx: CoreContext) -> Result<()> {
  let mut file = File::get_unique(1).await?;

  ctx.sync.operation(file.id,
    SyncResource::File(
      Operation::Update(
  			FileUpdate::HasThumbnail(true)
  		)
  	)
  );

  Ok(())
}
```

Then inside the `sync` function we send the event to the

```rust
  impl SyncEngine {
  	pub fn operation(&self, uuid: &str, sync_resource: SyncResource) {
      self.perform_operation(
        uuid.clone(),
        SyncTransport::Message(sync_resource)
      );
    }
	}
```

Files also implement `OperationalMerge` would use

# Resources

- https://archive.jlongster.com/using-crdts-in-the-wild
- https://cse.buffalo.edu/tech-reports/2014-04.pdf
- https://sergeiturukin.com/2017/06/26/hybrid-logical-clocks.html
- https://github.com/atolab/uhlc-rs
- https://github.com/alangibson/awesome-crdt
- https://blog.logrocket.com/libp2p-tutorial-build-a-peer-to-peer-app-in-rust/
