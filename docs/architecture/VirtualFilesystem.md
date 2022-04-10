# Virtual Filesystem

Spacedrive maintains a virtual filesystem comprised of storage locations through various clients. It records important metadata about a given file as well as a unique checksum for content based addressing [CAS]().



### File

`Shared data`

Represents a unique file across the virtual filesystem, all Spacedrive metadata is tied to this resource through local data relations. 

```rust
struct File {
  id: i32,
  partial_checksum: String,
  checksum: Option<String>,
  
  kind: FileKind,

  has_thumbnail: bool,
  has_thumbstrip: bool,
  has_video_preview: bool,
  encryption: EncryptionAlgorithm,
  ipfs_id: Option<String>,
  
  tags: Vec<Tag>,
  labels: Vec<Label>,
  file_paths: Vec<FilePath>,
  comments: Vec<Comment>,
  albums: Vec<Album>,
  
  date_modified: DateTime<Utc>,
}
```

- `partial_checksum ` - A SHA256 checksum generated from 5 samples of 10,000 bytes throughout the file data, including the begining and end. This is used to identify a file as *likely* unique in under 100µs. 

  > It is impossible to have a unique constraint at a database level for the `partial_checksum` however we can asyncronously resolve conflicts by querying for duplicates and generating full checksums at a later date.
  >
  > For synchronization of this resource we can tolorate temporary duplicates, any client can calculate that two files resources are duplicate and merge them into a single resource. In turn, triggering a shared data merge operation, whereby the older record is prioritsed at a property level during the merge.

- `checksum` - A full SHA256 checksum of the file data used to verify uniqueness should a `partial_checksum` conflict occur.



### FilePath

`Owned data`

This represents a logical file path within a [Location](), used to derive `file` records.

```rust
struct FilePath {
  uuid: String,
  is_dir: bool,
  location_id: i32,
  materialized_path: String,
  name: String,
  extension: Option<String>,
  parent_id: Option<i32>,
  file_id: Option<i32>,
  size_in_bytes: String,
  date_created: DateTime<Utc>,
  date_modified: DateTime<Utc>,
  date_indexed: DateTime<Utc>,
}
```

