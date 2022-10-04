# FFMPEG Thumbnailer RS

Rust implementation of a thumbnail generation for video files using ffmpeg. 
Based on https://github.com/dirkvdb/ffmpegthumbnailer

For now only implements the minimum API for Spacedrive needs. PRs are welcome

## Usage

```rust

use ffmpegthumbnailer_rs::{to_thumbnail, ThumbnailerError};

#[tokio::main]
async fn main() -> Result<(), ThumbnailerError> {
    to_thumbnail("input.mp4", "output.webp", 256, 100.0).await
}

```

Or you can use a builder to change the default options

```rust

use ffmpegthumbnailer_rs::{ThumbnailerBuilder, ThumbnailerError};

#[tokio::main]
async fn main() -> Result<(), ThumbnailerError> {
    let thumbnailer = ThumbnailerBuilder::new()
        .width_and_height(420, 315)
        .seek_percentage(0.25)?
        .with_film_strip(false)
        .quality(80.0)?
        .build();
    
    thumbnailer.process("input.mp4", "output.webp").await
}

```