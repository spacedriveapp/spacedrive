# Objects

```rust

// Objects are primarily created by the identifier from Paths
// Some Objects are purely virtual, unless they have one or more associated Paths, which refer to a file found in a Location
// Objects are what can be added to Spaces
enum ObjectKind {
  // A file that can not be identified by the indexer
  Unknown,
  // A known filetype, but without specific support
  Document,
  // A virtual filesystem directory
  Folder
  // A file that contains human-readable text
  TextFile,
  // A virtual directory int
  Package,
  // An image file
  Image,
  // An audio file
  Audio,
  // A video file
  Video,
  // A compressed archive of data 
  Archive,
  // An executable, program or application
  Executable,
  // A link to another object
  Alias,
  // Raw bytes encrypted by Spacedrive with self contained metadata
  EncryptedBytes,
  // A link can open web pages, apps or Spaces
  Link,
  // A special filetype that represents a preserved webpage
  WebPageArchive,
  // A widget is a mini app that can be placed in a Space at various sizes, associated Widget struct required
  Widget,
  // Albums can only have one level of children, and are associated with the Album struct
  Album,
  // Its like a folder, but appears like a stack of files, designed for burst photos / associated groups of files
  Collection,
}

pub struct Object {
  kind: ObjectKind,
  cas_id: String?,
  integrity_checksum: String?,
  parent_id: String?,
  data: Binary?,
}


// raw file path on physical filesystem
pub struct Path {

}

```