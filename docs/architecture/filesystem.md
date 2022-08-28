# Virtual Filesystem

Spacedrive maintains a virtual filesystem comprised of storage locations through various clients. It records important metadata about a given file as well as a unique checksum for content based addressing [CAS]().

### File — `Shared data`

Represents a unique file across the virtual filesystem, all Spacedrive metadata is tied to this resource through local data relations.

```rust
struct File {
  id: i32,
  cas_id: str,
  integrity_checksum: Option<str>,
  kind: FileKind,
  hidden: bool,
  favorite: bool,
  has_thumbnail: bool,
  has_thumbstrip: bool,
  has_video_preview: bool,
  key: Key,
  ipfs_id: Option<str>,
  paths: Vec<FilePath>,
  tags: Vec<Tag>,
  labels: Vec<Label>,
  notes: Vec<Note>,
  albums: Vec<Album>,
  media_data: Option<MediaData>,
  date_created: DateTime<Utc>,
  date_modified: DateTime<Utc>,
}
```

- `cas_id ` - A SHA256 checksum generated from 5 samples of 10,000 bytes throughout the file data, including the beginning and end + total byte count. This is used to identify a file as _likely_ unique in under 100µs.

> ~~It is impossible to have a unique constraint at a database level for the `partial_checksum` however we can asynchronously resolve conflicts by querying for duplicates and generating full checksums at a later date.~~
>
> For synchronization of this resource we can tolerate temporary duplicates, any client can calculate that two files resources are duplicate and merge them into a single resource. In turn, triggering a shared data merge operation, whereby the older record is prioritized at a property level during the merge.

- `integrity_checksum` - A full SHA256 checksum of the file data used to verify uniqueness should a `cas_id` conflict occur.

### FilePath — `Owned data`

This represents a logical file path within a [Location](), used to derive `file` records.

```rust
struct FilePath {
  uuid: String,
  is_dir: bool,
  location_id: i32,
  path: String,
  name: String,
  extension: Option<String>,
  size_in_bytes: String,
  permissions: Option<String>,

  parent_id: Option<i32>,
  file_id: Option<i32>,

  date_created: DateTime<Utc>,
  date_modified: DateTime<Utc>,
  date_indexed: DateTime<Utc>,
}
```

```typescript
export function useBridgeCommand<
	K extends CommandKeyType,
	CC extends CCType<K>,
	CR extends CRType<K>
>(key: K, options: UseMutationOptions<ExtractData<CC>> = {}) {
	return useMutation<ExtractData<CR>, unknown, any>(
		[key],
		async (vars: ExtractParams<CC>) => await commandBridge(key, vars),
		options
	);
}
```
