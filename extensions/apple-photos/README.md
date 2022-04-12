This extension must first register an indexer context to prevent the indexer from scanning the photo library

```rust
struct IndexerContext {
    key: String,
    is_dir: bool,
    extension: Option<String>,
    must_contain: Vec<String>,
    always_ignored: Option<String>
    scan: bool,
}
```

```rust
core.register_context(IndexerContext {
    key: "apple-photo-library",
    is_dir: false,
    extension: ".photoslibrary",
    must_contain: vec!["database", "originals"],
    always_ignored: None, 
    scan: false, // apple-photos extension takes care of scan
});

core.register_context(IndexerContext {
    key: "github-repository",
    is_dir: true,
    extension: None,
    must_contain: vec![".git"],
    always_ignored: Some("node_modules", "target")
    scan: true,
});
```

For Apple Photos we need:
- Hidden/Favorite items
- Live photo support
- Original creation date
- Edited photos
- Albums
