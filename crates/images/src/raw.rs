//! # Camera raw previews
//!
//! Camera raw files are drawn from the JPEG preview the camera embedded.
//! No raw decoder ships with Spacedrive, but nearly every raw format carries a
//! full or near-full size JPEG rendered by the camera. TIFF-based formats (CR2,
//! NEF, ARW, DNG, ORF, RW2, PEF and most others) point at it from an IFD, so
//! those are walked. Formats with their own container (CR3, RAF) are scanned for
//! JPEG start markers instead. The largest preview that decodes wins, since the
//! small ones are 160px EXIF thumbnails.
//!
//! ## Example
//! ```rust,no_run
//! // A raw extension routes `format_image` to the embedded preview.
//! let preview = sd_images::format_image("/photos/IMG_0001.CR2")?;
//! assert!(sd_images::is_raw(std::path::Path::new("/photos/IMG_0001.CR2")));
//! # Ok::<(), sd_images::Error>(())
//! ```

use std::{collections::HashSet, io::Cursor, path::Path};

use image::{DynamicImage, ImageFormat, ImageReader};

use crate::{
	error::{Error, Result},
	ImageHandler,
};

pub struct RawHandler {}

/// Whether `path` names a camera raw format this module draws from its
/// embedded preview.
#[must_use]
pub fn is_raw(path: &Path) -> bool {
	path.extension()
		.and_then(|ext| ext.to_str())
		.is_some_and(|ext| {
			crate::consts::RAW_EXTENSIONS
				.iter()
				.any(|raw| raw.eq_ignore_ascii_case(ext))
		})
}

impl ImageHandler for RawHandler {
	fn handle_image(&self, path: &Path) -> Result<DynamicImage> {
		let data = self.get_data(path)?;
		embedded_preview(&data).ok_or(Error::Unsupported)
	}
}

const COMPRESSION: u16 = 0x0103;
const STRIP_OFFSETS: u16 = 0x0111;
const STRIP_BYTE_COUNTS: u16 = 0x0117;
const SUB_IFDS: u16 = 0x014a;
const JPEG_OFFSET: u16 = 0x0201;
const JPEG_LENGTH: u16 = 0x0202;
const PANASONIC_JPEG: u16 = 0x002e;
const EXIF_IFD: u16 = 0x8769;

/// Scanning a non-TIFF container stops after this many start markers, so a
/// file with no preview does not try to decode every stray `FF D8` pair.
const SCAN_LIMIT: usize = 32;

/// The largest embedded JPEG in `data` that decodes, if any.
pub fn embedded_preview(data: &[u8]) -> Option<DynamicImage> {
	let candidates = tiff_previews(data).unwrap_or_else(|| scanned_previews(data));
	let mut sized: Vec<(u64, &[u8])> = candidates
		.into_iter()
		.filter_map(|jpeg| {
			let (width, height) = ImageReader::with_format(Cursor::new(jpeg), ImageFormat::Jpeg)
				.into_dimensions()
				.ok()?;
			Some((u64::from(width) * u64::from(height), jpeg))
		})
		.collect();
	sized.sort_by_key(|(pixels, _)| std::cmp::Reverse(*pixels));
	sized
		.into_iter()
		.find_map(|(_, jpeg)| image::load_from_memory_with_format(jpeg, ImageFormat::Jpeg).ok())
}

#[derive(Clone, Copy)]
enum Order {
	Little,
	Big,
}

impl Order {
	fn u16(self, data: &[u8], at: usize) -> Option<u16> {
		let bytes: [u8; 2] = data.get(at..at.checked_add(2)?)?.try_into().ok()?;
		Some(match self {
			Self::Little => u16::from_le_bytes(bytes),
			Self::Big => u16::from_be_bytes(bytes),
		})
	}

	fn u32(self, data: &[u8], at: usize) -> Option<u32> {
		let bytes: [u8; 4] = data.get(at..at.checked_add(4)?)?.try_into().ok()?;
		Some(match self {
			Self::Little => u32::from_le_bytes(bytes),
			Self::Big => u32::from_be_bytes(bytes),
		})
	}
}

/// JPEG previews an IFD points at, or `None` when `data` is not TIFF-based.
///
/// The magic is loose on purpose: Olympus writes `IIRO` and Panasonic
/// `IIU\0` where TIFF has `II*\0`.
fn tiff_previews(data: &[u8]) -> Option<Vec<&[u8]>> {
	let order = match data.get(..2)? {
		b"II" => Order::Little,
		b"MM" => Order::Big,
		_ => return None,
	};
	let magic = order.u16(data, 2)?;
	if !matches!(magic, 0x002a | 0x4f52 | 0x5352 | 0x0055) {
		return None;
	}

	let mut found = Vec::new();
	let mut queue = vec![usize::try_from(order.u32(data, 4)?).ok()?];
	let mut seen = HashSet::new();
	while let Some(ifd) = queue.pop() {
		if ifd == 0 || !seen.insert(ifd) || seen.len() > 64 {
			continue;
		}
		let Some(count) = order.u16(data, ifd) else {
			continue;
		};
		let mut jpeg = (None, None);
		let mut strip = (None, None, None);
		for index in 0..usize::from(count) {
			let entry = ifd + 2 + index * 12;
			let (Some(tag), Some(kind), Some(n)) = (
				order.u16(data, entry),
				order.u16(data, entry + 2),
				order.u32(data, entry + 4),
			) else {
				break;
			};
			let value = || -> Option<u32> {
				match kind {
					3 => order.u16(data, entry + 8).map(u32::from),
					_ => order.u32(data, entry + 8),
				}
			};
			match tag {
				JPEG_OFFSET => jpeg.0 = value(),
				JPEG_LENGTH => jpeg.1 = value(),
				COMPRESSION => strip.0 = value(),
				STRIP_OFFSETS if n == 1 => strip.1 = value(),
				STRIP_BYTE_COUNTS if n == 1 => strip.2 = value(),
				PANASONIC_JPEG => {
					if let Some(bytes) = slice(data, order.u32(data, entry + 8), Some(n)) {
						found.push(bytes);
					}
				}
				SUB_IFDS | EXIF_IFD => {
					let offset = if n == 1 {
						value()
					} else {
						order.u32(data, entry + 8)
					};
					let Some(offset) = offset.and_then(|o| usize::try_from(o).ok()) else {
						continue;
					};
					if n == 1 {
						queue.push(offset);
					} else {
						for sub in 0..n.min(16) {
							if let Some(child) = order.u32(data, offset + sub as usize * 4) {
								queue.extend(usize::try_from(child).ok());
							}
						}
					}
				}
				_ => {}
			}
		}
		found.extend(slice(data, jpeg.0, jpeg.1));
		// Compression 6 is old-style JPEG and 7 is JPEG, which NEF and DNG use
		// for full size previews stored as a single strip.
		if matches!(strip.0, Some(6 | 7)) {
			found.extend(slice(data, strip.1, strip.2));
		}
		let next = ifd + 2 + usize::from(count) * 12;
		if let Some(next) = order.u32(data, next).and_then(|o| usize::try_from(o).ok()) {
			queue.push(next);
		}
	}
	Some(found)
}

/// A JPEG stream at `offset` running `length` bytes, when it is one.
fn slice(data: &[u8], offset: Option<u32>, length: Option<u32>) -> Option<&[u8]> {
	let start = usize::try_from(offset?).ok()?;
	let end = start.checked_add(usize::try_from(length?).ok()?)?;
	let bytes = data.get(start..end)?;
	bytes.starts_with(&[0xff, 0xd8, 0xff]).then_some(bytes)
}

/// Every JPEG start marker in a container this module cannot walk, each
/// running to the end of the file. The decoder stops at its own end marker.
fn scanned_previews(data: &[u8]) -> Vec<&[u8]> {
	data.windows(4)
		.enumerate()
		.filter(|(_, window)| {
			window[..3] == [0xff, 0xd8, 0xff]
				&& matches!(window[3], 0xdb | 0xe0 | 0xe1 | 0xc0 | 0xc4)
		})
		.take(SCAN_LIMIT)
		.map(|(start, _)| &data[start..])
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	fn jpeg(width: u32, height: u32) -> Vec<u8> {
		let mut out = Cursor::new(Vec::new());
		DynamicImage::new_rgb8(width, height)
			.write_to(&mut out, ImageFormat::Jpeg)
			.expect("encode");
		out.into_inner()
	}

	/// A little-endian TIFF whose IFD0 points at a small JPEG and whose
	/// SubIFD holds a larger one as a single JPEG strip, the NEF layout.
	fn tiff(small: &[u8], large: &[u8]) -> Vec<u8> {
		let ifd0 = 8usize;
		let ifd0_len = 2 + 3 * 12 + 4;
		let sub = ifd0 + ifd0_len;
		let sub_len = 2 + 3 * 12 + 4;
		let small_at = sub + sub_len;
		let large_at = small_at + small.len();
		let mut out = b"II\x2a\x00".to_vec();
		out.extend(u32::try_from(ifd0).unwrap_or_default().to_le_bytes());
		let entry = |out: &mut Vec<u8>, tag: u16, kind: u16, value: usize| {
			out.extend(tag.to_le_bytes());
			out.extend(kind.to_le_bytes());
			out.extend(1u32.to_le_bytes());
			out.extend(u32::try_from(value).unwrap_or_default().to_le_bytes());
		};
		out.extend(3u16.to_le_bytes());
		entry(&mut out, SUB_IFDS, 4, sub);
		entry(&mut out, JPEG_OFFSET, 4, small_at);
		entry(&mut out, JPEG_LENGTH, 4, small.len());
		out.extend(0u32.to_le_bytes());
		out.extend(3u16.to_le_bytes());
		entry(&mut out, COMPRESSION, 3, 7);
		entry(&mut out, STRIP_OFFSETS, 4, large_at);
		entry(&mut out, STRIP_BYTE_COUNTS, 4, large.len());
		out.extend(0u32.to_le_bytes());
		out.extend(small);
		out.extend(large);
		out
	}

	#[test]
	fn the_largest_tiff_preview_wins() {
		let file = tiff(&jpeg(16, 12), &jpeg(64, 48));
		let preview = embedded_preview(&file).expect("a preview");
		assert_eq!((preview.width(), preview.height()), (64, 48));
	}

	#[test]
	fn a_container_is_scanned_for_jpeg_markers() {
		let mut file = b"ftypcrx \x00\x00\x00\x00 header".to_vec();
		file.extend(jpeg(32, 24));
		file.extend(b"raw sensor data");
		let preview = embedded_preview(&file).expect("a preview");
		assert_eq!((preview.width(), preview.height()), (32, 24));
	}

	#[test]
	fn a_file_with_no_preview_has_none() {
		assert!(embedded_preview(b"II\x2a\x00\x08\x00\x00\x00\x00\x00").is_none());
		assert!(embedded_preview(b"not an image at all").is_none());
	}
}
