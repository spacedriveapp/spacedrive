//! Render a thumbnail for each file given on the command line and report what
//! the system codecs produced.
//!
//! ```sh
//! cargo run -p sd-imageio --example probe -- photo.heic scan.pdf clip.mp4
//! ```

#[cfg(target_os = "macos")]
fn main() {
	let paths: Vec<String> = std::env::args().skip(1).collect();
	if paths.is_empty() {
		eprintln!("usage: probe <file>...");
		std::process::exit(2);
	}

	let mut failed = 0;
	for path in &paths {
		let started = std::time::Instant::now();
		match sd_imageio::file_thumbnail_jpeg(std::path::Path::new(path), 256, 0.8) {
			Some(jpeg) => {
				// JPEG SOF0 carries the dimensions; enough to confirm the decode
				// produced a real image rather than an empty buffer.
				let dims = jpeg_dimensions(&jpeg)
					.map(|(w, h)| format!("{w}x{h}"))
					.unwrap_or_else(|| "unknown".into());
				println!(
					"ok    {path}  {} bytes  {dims}  {:?}",
					jpeg.len(),
					started.elapsed()
				);
			}
			None => {
				failed += 1;
				println!("none  {path}  no system preview");
			}
		}
	}

	if failed > 0 {
		std::process::exit(1);
	}
}

/// Read width and height from the first start-of-frame marker.
#[cfg(target_os = "macos")]
fn jpeg_dimensions(data: &[u8]) -> Option<(u16, u16)> {
	let mut i = 2;
	while i + 9 < data.len() {
		if data[i] != 0xFF {
			i += 1;
			continue;
		}
		let marker = data[i + 1];
		// SOF0 through SOF15, excluding the non-frame markers in that range.
		if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
			let height = u16::from_be_bytes([data[i + 5], data[i + 6]]);
			let width = u16::from_be_bytes([data[i + 7], data[i + 8]]);
			return Some((width, height));
		}
		let length = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
		i += 2 + length;
	}
	None
}

#[cfg(not(target_os = "macos"))]
fn main() {
	eprintln!("sd-imageio is macOS only");
	std::process::exit(2);
}
