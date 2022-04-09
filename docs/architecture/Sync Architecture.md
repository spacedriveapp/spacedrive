# Distributed Data Sync Architecture

## Data Types
We have two forms of data, shared & owned.
- **Shared data** - can be created and modified by any client. 
- **Owned data** - can only be modified by the client that created it.

> Note: Not all data falls under these two categories, some might be derived from shared/owned data, or simply not synchronized at all.

Shared resources could be, `tags`, `comments`, `albums`, `jobs`, `tags_on_files`. Since these can be created, updated or deleted by any client at any time.

Owned resources would be `file_paths`, since a client is the single source of truth for this data.

The `files` dataset is derived on each client from already synced `file_paths`, and is created automatically after ingesting new `file_paths`.

## Client Pool
Clients are synchronized in a shared pool, without possibility of conflicts. 
```rust
struct ClientPool {
	clients: Vec<Client>
}

struct Client {
	uuid: String,
	last_seen: DateTime<Utc>,
	last_synchronized: DateTime<Utc>,
	connection: Option<ClientConnection>
}
```
Knowledge of all clients are stored in the database, per client. When a given client's state changes it will notify every other client in the pool via the open `connection`. The `ClientConnection` is maintained in memory and is established on startup.

Clients will ping-pong to ensure their connection stays alive via TCP, however this logic happens within the `ClientConnection`.

<!-- The `last_synchronized` timestamp determines -->

**Handling stale clients**

If a client has not been seen in X amount of time, other clients will not keep pending operations for them. Clients take care of flushing the pending operation queue one all non-stale clients have received the pending operations.

## Operations
Operations wrap a given change, they are stored in the database as `pending_operations`. Operations are removed once all online clients have received the payload.
```rust
struct Operation<V> {
	client_uuid: String,
	uhlc_timestamp: uhlc::Timestamp,
	resource_key: String,
	resource_uuid: String,
	action: Action,
	value: Option<Box<V>>,
	on_conflict: OnConflict // maybe
}

enum Action {
	Create,
	Update,
	Delete
}

enum OnConflict {
	Ignore,
	Merge,
	Overwrite
}
```
 Operations are timestamped with a UHLC (Unique Hybrid Logical Clock)
```
2022-04-09T06:53:36.397295996Z/89F9DD8514914648989315B2D30D7BE5
```
Each client maintains an instance of a UHLC clock and will reject operations with a timestamp drift greater than 100ms (can be adjusted).

<!-- ## Conflict Resolution
Certain resources will have different methods of conflict resolution.
For example, the `file` resource will always be `OnConflict::Merge` since it -->

## Sync Methods


## Thoughts
- Not all updates are stored as pending operations, some can be calculated at sync time from the recipient node's `last_synchronized` value and queried by `updated_at`. This is a conflict free synchronization and the resources with the newer date value will take priority.
- `CrdtType` could determine the conflict resolution outcome per resource.
```rust
enum CrdtType {
	GSet,
	LwwSet,
}
```


<!-- ## Shared Resources
- Shared resources have a globally unique identifier.


## 
- Files and directories partitioned by client that owns them
- Global state modified using Operational Transforms-like system
	- Transforms defined at the property level
	- Transforms can have different priority
	- eg. Tags that can be created and modified, but deleting takes precedence
- All clients store a list of other clients and a list of pending changes to be synced to each client
- New clients need to retrieve entire database from any client
	- Includes any pending changes from said database
	- Thus will be in the same state as the database it synced from
	- Needs to ask other databases for changes and identify itself


# Resources
https://archive.jlongster.com/using-crdts-in-the-wild
https://cse.buffalo.edu/tech-reports/2014-04.pdf -->