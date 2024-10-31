pub use crate::error::{Error, Result};
use crate::ImageHandler;
use image::DynamicImage;
use libheif_rs::{ColorSpace, HeifContext, LibHeif, RgbChroma};
use std::io::{Cursor, SeekFrom};
use std::io::{Read, Seek};
use std::path::Path;
use std::sync::LazyLock;

static HEIF: LazyLock<LibHeif> = LazyLock::new(LibHeif::new);

pub struct HeifHandler {}

impl ImageHandler for HeifHandler {
	// fn validate_image(&self, bits_per_pixel: u8, length: usize) -> Result<()> {
	// 	if bits_per_pixel != 8 {
	// 		return Err(Error::InvalidBitDepth);
	// 	} else if length % 3 != 0 || length % 4 != 0 {
	// 		return Err(Error::InvalidLength);
	// 	}

	// 	Ok(())
	// }

	fn handle_image(&self, path: &Path) -> Result<DynamicImage> {
		let img = {
			let data = self.get_data(path)?;
			let handle = HeifContext::read_from_bytes(&data)?.primary_image_handle()?;
			HEIF.decode(&handle, ColorSpace::Rgb(RgbChroma::Rgb), None)
		}?;

		let planes = img.planes();

		if let Some(i) = planes.interleaved {
			// self.validate_image(i.bits_per_pixel, i.data.len())?;

			let mut reader = Cursor::new(i.data);
			let mut sequence = vec![];
			let mut buffer = [0u8; 3]; // [r, g, b]

			// this is the interpolation stuff, it essentially just makes the image correct
			// in regards to stretching/resolution, etc
			(0..img.height()).try_for_each(|x| {
				let x: usize = x.try_into()?;
				let start: u64 = (i.stride * x).try_into()?;
				reader
					.seek(SeekFrom::Start(start))
					.map_err(|e| Error::Io(e, path.to_path_buf().into_boxed_path()))?;

				(0..img.width()).try_for_each(|_| {
					reader
						.read_exact(&mut buffer)
						.map_err(|e| Error::Io(e, path.to_path_buf().into_boxed_path()))?;

					sequence.extend_from_slice(&buffer);
					Ok::<(), Error>(())
				})?;
				Ok::<(), Error>(())
			})?;

			image::RgbImage::from_raw(img.width(), img.height(), sequence).map_or_else(
				|| Err(Error::RgbImageConversion),
				|x| Ok(DynamicImage::ImageRgb8(x)),
			)
		} else if let (Some(r), Some(g), Some(b)) = (planes.r, planes.g, planes.b) {
			// This implementation is **ENTIRELY** untested, as I'm unable to source
			// a HEIF image that has separate r/g/b channels, let alone r/g/b/a.
			// This was hand-crafted using my best judgement, and I think it should work.
			// I'm sure we'll get a GH issue opened regarding it if not - brxken128

			// self.validate_image(r.bits_per_pixel, r.data.len())?;
			// self.validate_image(g.bits_per_pixel, g.data.len())?;
			// self.validate_image(b.bits_per_pixel, b.data.len())?;

			let mut red = Cursor::new(r.data);
			let mut green = Cursor::new(g.data);
			let mut blue = Cursor::new(b.data);

			let (mut alpha, has_alpha) = planes.a.map_or_else(
				|| (Cursor::new([].as_ref()), false),
				|a| (Cursor::new(a.data), true),
			);

			let mut sequence = vec![];
			let mut buffer: [u8; 4] = [0u8; 4];

			// this is the interpolation stuff, it essentially just makes the image correct
			// in regards to stretching/resolution, etc
			(0..img.height()).try_for_each(|x| {
				let x: usize = x.try_into()?;
				let start: u64 = (r.stride * x).try_into()?;

				red.seek(SeekFrom::Start(start))
					.map_err(|e| Error::Io(e, path.to_path_buf().into_boxed_path()))?;

				(0..img.width()).try_for_each(|_| {
					red.read_exact(&mut buffer[0..1])
						.map_err(|e| Error::Io(e, path.to_path_buf().into_boxed_path()))?;

					green
						.read_exact(&mut buffer[1..2])
						.map_err(|e| Error::Io(e, path.to_path_buf().into_boxed_path()))?;

					blue.read_exact(&mut buffer[2..3])
						.map_err(|e| Error::Io(e, path.to_path_buf().into_boxed_path()))?;

					sequence.extend_from_slice(&buffer[..3]);

					if has_alpha {
						alpha
							.read_exact(&mut buffer[3..4])
							.map_err(|e| Error::Io(e, path.to_path_buf().into_boxed_path()))?;

						sequence.extend_from_slice(&buffer[3..4]);
					}
					Ok::<(), Error>(())
				})?;
				Ok::<(), Error>(())
			})?;

			if has_alpha {
				image::RgbaImage::from_raw(img.width(), img.height(), sequence).map_or_else(
					|| Err(Error::RgbImageConversion),
					|x| Ok(DynamicImage::ImageRgba8(x)),
				)
			} else {
				image::RgbImage::from_raw(img.width(), img.height(), sequence).map_or_else(
					|| Err(Error::RgbImageConversion),
					|x| Ok(DynamicImage::ImageRgb8(x)),
				)
			}
		} else {
			Err(Error::Unsupported)
		}
	}
}
