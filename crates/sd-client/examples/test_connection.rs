use sd_client::{SpacedriveClient, SdPath};
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Get socket path and HTTP URL from environment or use defaults
    let socket_path = env::var("SD_SOCKET_PATH")
        .unwrap_or_else(|_| {
            let home = env::var("HOME").expect("HOME not set");
            format!("{}/Library/Application Support/spacedrive/daemon/daemon.sock", home)
        });

    let http_url = env::var("SD_HTTP_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:54321".to_string());

    let library_id = env::var("SD_LIBRARY_ID")
        .expect("SD_LIBRARY_ID must be set");

    println!("Connecting to Spacedrive daemon...");
    println!("  Socket: {}", socket_path);
    println!("  HTTP: {}", http_url);
    println!("  Library: {}", library_id);

    let mut client = SpacedriveClient::new(socket_path.into(), http_url);
    client.set_library(library_id);

    println!("\nQuerying media files...");

    // Query for media files in root
    let files = client
        .media_listing(
            SdPath::Physical {
                device_slug: "james-s-macbook-pro".to_string(),
                path: "/Users/jamespine/Desktop".to_string(),
            },
            Some(100),
        )
        .await?;

    println!("\nFound {} files", files.len());

    // Display first 10 files with thumbnail URLs
    for (i, file) in files.iter().take(10).enumerate() {
        println!("\n[{}] {}", i + 1, file.name);
        println!("  ID: {}", file.id);
        println!("  Size: {} bytes", file.size);
        println!("  Kind: {}", file.content_kind);

        if let Some(content_id) = &file.content_identity {
            println!("  Content UUID: {}", content_id.uuid);

            // Find thumbnails
            let thumbnails: Vec<_> = file
                .sidecars
                .iter()
                .filter(|s| s.kind == "thumb")
                .collect();

            if !thumbnails.is_empty() {
                println!("  Thumbnails:");
                for thumb in thumbnails {
                    let url = client.thumbnail_url(
                        &content_id.uuid,
                        &thumb.variant,
                        &thumb.format,
                    );
                    println!("    - {} ({})", thumb.variant, thumb.status);
                    println!("      {}", url);
                }

                // Show best thumbnail for 200px size
                if let Some(best) = client.select_best_thumbnail(&file.sidecars, 200.0) {
                    println!("  Best thumbnail for 200px: {}", best.variant);
                }
            } else {
                println!("  No thumbnails available");
            }
        }
    }

    Ok(())
}
