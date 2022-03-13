use anyhow::Result;
use mime;
use std::fs::File;
use std::io::BufReader;
use std::io::Cursor;
use thumbnailer::{create_thumbnails, ThumbnailSize};

pub async fn create_thumb(path: &str) -> Result<()> {
	let file = File::open(path).unwrap();
	let reader = BufReader::new(file);

	let mut thumbnails = create_thumbnails(reader, mime::IMAGE_PNG, [ThumbnailSize::Small]).unwrap();

	let thumbnail = thumbnails.pop().unwrap();

	let mut buf = Cursor::new(Vec::new());

	thumbnail.write_png(&mut buf).unwrap();

	Ok(())
}
