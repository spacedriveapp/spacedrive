```rust 
// file properties add additional functionality to the file resource
pub enum NativeFileProperty {
    PreviewMedia { resolution: PreviewMediaResolution }
    Tag { id: String },
    ImageMetadata,
    GeoLocation,
    Package,
    SyntaxHighlight,
    GithubRepository
}

FileType::new(".png", vec![
    NativeFileProperty::PreviewMedia {
        resolution: PreviewMediaResolution::Medium
    },
    NativeFileProperty::ImageMetadata 
])

```