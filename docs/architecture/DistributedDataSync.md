Spacedrive Architecture

# Distributed Data Synchronization

Synchronizing data between clients in a Spacedrive network is unlike distributed systems such as crypto and blockchain. We do not need 100% accuracy and can resolve all possible conflicts and error cases with minimal dispute.

Utilizing various forms of [CRDTs](https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type) together with a hybrid logical clock; we ensure eventual constancy while optimizing performance on an [SQLite](https://www.sqlite.org/) database reaching into the gigabytes. 



## Data Types
Data is divided into two kinds, Shared and Owned.
- **Shared data** - can be created and modified by any client. 
- **Owned data** - can only be modified by the client that created it.

Shared resources could be, `tags`, `comments`, `albums`, `jobs`, `tags_on_files`. Since these can be created, updated or deleted by any client at any time.

Owned resources would be `file_paths` & `media_data`, since a client is the single source of truth for this data.

> Note: Not all data falls under these two categories, some might be derived from shared/owned data, and/or not synchronized at all.

The `files` dataset (unique records of files based on content adressable storage) is derived on each client from `file_paths`. This data is generated post-sync for the purposes of local relations for shared state to optimize data queries.




## Client Pool
The client pool maintains record of all clients in the network.

An exact replica of the Client Pool is synchronized on each client. When a given client has a state change, it will notify every other client in the pool via the `connection`  struct. 

The `ClientConnection` is maintained in memory and is established on startup.

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
Clients will ping-pong to ensure their connection stays alive via TCP, however this logic is contained within the `ClientConnection` instance.

**Handling stale clients**

If a client has not been seen in X amount of time, other clients will not persist pending operations for them. Clients take care of flushing the pending operation queue one all non-stale clients have received the pending operations.



## Operations
Operations wrap a given data change, they are stored in the database as `pending_operations`. 

Operations are removed once all online clients have received the payload.

```rust
struct Operation<V> {
  // unique identifier for this client
	client_uuid: String,
  // a unique hybrid logical clock timestamp
	uhlc_timestamp: uhlc::Timestamp,
  // the kind of operation to perform
	method: OperationMethod,
  // the name of the database table
	resource_type: String,
  // the unique identifer of the resource (None for batched)
  resource_uuid: Option<String>,
  // the property on the resource whoes value shall be affected
  resource_property: Option<String>
  // optional value for operation
	value: Option<Box<V>>,
}

enum OperationMethod {
  OwnedBatchCreate,
  OwnedDelete,
  SharedCreate,
  SharedUpdate
  SharedDelete
}

```



## Clock
For realtime synchronization it is important to maintain the true order of events, we can timestamp each operation, but have to account for time drift across the distributed system. There is no way to guarantee two machines have synchronized system clocks.

We can solve this with a Unique Hybrid Logical Clock ([UHLC]()): a globally-unique, monotonic timestamp. Each client combines their hybrid time with a unique identifier. When receiving new operations, a client will update its own clock with the incoming timestamp.
```
2022-04-09T06:53:36.397295996Z/89F9DD8514914648989315B2D30D7BE5
```
Each client maintains an instance of a UHLC clock and will reject operations with a timestamp drift greater than 100ms (can be adjusted).

This allows us to entirely avoid needing to synchronize time between clients, as each client controls its own order of operations, never producing a conflicting timestamp with another system.



## Pending operations

Here are some examples of how operations are stored to minimize disk usage and data duplication.

**Batch create operation for Owned data**

In the below case, this data tells the sync engine to query the database for all resources created or updated before sending the sync event. This is typically used for Owned data. It will be conflict free but the ordering is still important, potentially hundreds of thousands of `file_path` resources might be represented by a single entry here.

| `client_uuid` |      `uhlc_timestamp`      |      `method`      | `resource_key` | `resource_uuid` | `resource_property` | `value` |
|----------|-------------|------|----------|----------|----------|----------|
| 2e8f85bf... | 2022-04-09T06:53:36... | OwnedBatchCreate | file_paths | NULL | NULL | NULL |

**Create operation for Shared data**

In the next case we're handling the creation of a Shared resource. The `method` is marked `Create` and the value is `NULL`. This is because we can also use the actual database record in the `tags` table as it was newly created.

| `client_uuid` |      `uhlc_timestamp`      |      `method`      | `resource_key` | `resource_uuid` | `resource_property` | `value` |
|----------|-------------|------|----------|----------|----------|----------|
| 2e8f85bf... | 2022-04-09T06:53:36... | SharedCreate | tags | 2e8f85bf...     | NULL | NULL |

**Update operation for Shared data**

Shared data works at a property level

| `client_uuid` |      `uhlc_timestamp`      |      `method`      | `resource_key` | `resource_uuid` | `resource_property` | `value` |
|----------|-------------|------|----------|----------|----------|----------|
| 2e8f85bf... | 2022-04-09T06:53:36... | SharedUpdate | albums | 2e8f85bf...     | name | "jeff" |



# Resources

- https://archive.jlongster.com/using-crdts-in-the-wild
- https://cse.buffalo.edu/tech-reports/2014-04.pdf 
- https://sergeiturukin.com/2017/06/26/hybrid-logical-clocks.html
- https://github.com/atolab/uhlc-rs
- https://github.com/alangibson/awesome-crdt
- https://blog.logrocket.com/libp2p-tutorial-build-a-peer-to-peer-app-in-rust/





### <!--OperationLevel-->

<!--We can define an `OperationLevel` to treat the incoming data as an entire resource, or an update to a given property of a resource. The `ClientPool` itself uses `OperationLevel::Resource` and `OnConflict::Overwrite` by default to synchronize clients.-->

<!--Operations that are for `OperationLevel::Resource` need not store a value in the operation queue, we can simply store a single entry with a timestamp that instructs the engine to query for updated resourced via their `updated_at` column in the database.-->
