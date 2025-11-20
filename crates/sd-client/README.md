# sd-client

Rust client library for connecting to the Spacedrive daemon.

## Features

- Unix socket communication with Spacedrive core
- Type-safe query and action execution
- Media file listing queries
- Thumbnail URL construction
- Smart thumbnail variant selection

## Usage

```rust
use sd_client::{SpacedriveClient, SdPath};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create client
    let mut client = SpacedriveClient::new(
        "/path/to/daemon.sock".into(),
        "http://localhost:54321".into(),
    );

    // Set library context
    client.set_library("library-uuid".to_string());

    // Query media files
    let files = client.media_listing(
        SdPath::Physical {
            device_id: "local".to_string(),
            path: "/Users/you/Photos".to_string(),
        },
        Some(1000),
    ).await?;

    // Get thumbnail URLs
    for file in files {
        if let Some(content_id) = file.content_identity {
            if let Some(thumb) = client.select_best_thumbnail(&file.sidecars, 256.0) {
                let url = client.thumbnail_url(
                    &content_id.uuid,
                    &thumb.variant,
                    &thumb.format,
                );
                println!("{}: {}", file.name, url);
            }
        }
    }

    Ok(())
}
```

## Example

Run the test connection example:

```bash
export SD_LIBRARY_ID="your-library-uuid"
export SD_SOCKET_PATH="$HOME/.spacedrive/daemon.sock"  # optional
export SD_HTTP_URL="http://127.0.0.1:54321"            # optional

cargo run --example test_connection
```

## API

### SpacedriveClient

- `new(socket_path, http_base_url)` - Create a new client
- `set_library(library_id)` - Set the current library context
- `execute(wire_method, input)` - Execute a query or action
- `media_listing(path, limit)` - Query media files
- `thumbnail_url(content_uuid, variant, format)` - Construct thumbnail URL
- `select_best_thumbnail(sidecars, target_size)` - Pick optimal thumbnail variant

### Types

- `File` - File metadata with content identity and sidecars
- `ContentIdentity` - Content hash and UUID for deduplication
- `Sidecar` - Generated derivatives (thumbnails, proxies, etc.)
- `SdPath` - Location-independent file reference
- `ImageMediaData` - Image-specific metadata (dimensions, date taken)
- `VideoMediaData` - Video-specific metadata (dimensions, duration)
