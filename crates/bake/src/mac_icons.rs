//! Native macOS file icons, rasterized into grid cells. Icons are keyed by
//! *type* (a folder, or a file extension), not by path, so each distinct
//! [`IconKey`] is painted exactly once and every file of that type shares the
//! tile.
//!
//! The pixels come from `NSWorkspace`'s type-icon lookup — the same Launch
//! Services binding Finder uses, so a registered app's document icon shows up
//! (a `.sketch` gets Sketch's icon). The `NSImage` is drawn into an offscreen
//! `NSBitmapImageRep` and composited over the cell background grey so
//! transparent areas read as background rather than black.
//!
//! AppKit's drawing machinery is safe from one thread at a time, not from
//! many at once; all painting in this module serializes behind one lock, so
//! producers can run from any worker thread.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use crate::{Decline, Producer, Tile, WorkItem};

/// What uniquely determines an icon tile. Files dedup by lowercased extension.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IconKey {
	Folder,
	Ext(String),
	/// No extension, or one the system doesn't recognize — the generic
	/// document icon.
	Generic,
}

impl IconKey {
	pub fn of(path: &Path, is_dir: bool) -> Self {
		if is_dir {
			return Self::Folder;
		}
		match path.extension().and_then(OsStr::to_str) {
			Some(ext) if !ext.is_empty() => Self::Ext(ext.to_ascii_lowercase()),
			_ => Self::Generic,
		}
	}
}

/// Serializes all AppKit access in this module (see the module docs).
static APPKIT: Mutex<()> = Mutex::new(());

/// The cell background grey transparent icons are composited over (matches the
/// atlas clear color so cells blend into the grid gaps). Stored B, G, R.
const BG: [u8; 3] = [0x35, 0x28, 0x27];
/// Resting name ink (the grey the label shows when unselected). Stored B, G, R.
const NAME_INK: [u8; 3] = [207, 214, 219];

/// Fraction of the cell kept clear on each side in icon-only mode, so the icon
/// floats with a margin rather than filling the cell edge-to-edge.
const ICON_PAD: f64 = 0.2;
const FILE_ICON_TOP: f64 = 49.0;
const FILE_ICON_SIZE: f64 = 62.0;
const TEXT_X: f64 = 8.0;
const NAME_Y: f64 = 27.0;
const DETAIL_Y: f64 = 12.0;
const TEXT_W: f64 = 112.0;
const NAME_H: f64 = 18.0;
const DETAIL_H: f64 = 14.0;

/// The baked name band as a cell-local (top-down) y range, derived from the
/// label layout. The grid passes this to the shader so the selection pill and
/// text tint land exactly on the baked name. (Top-down because the atlas tile
/// is stored top-row first, while the layout constants are authored y-up.)
pub const NAME_BAND_Y0: f32 = ((128.0 - (NAME_Y + NAME_H)) / 128.0) as f32;
pub const NAME_BAND_Y1: f32 = ((128.0 - NAME_Y) / 128.0) as f32;

/// The icon-tile stage: paints a type icon (and, when the item carries a
/// label, its name and detail text) into an opaque cell. Unlabeled tiles are
/// cached per `(key, size)`, so a grid of ten thousand PDFs paints one icon
/// and memcpys the rest.
pub struct IconProducer {
	cache: Mutex<HashMap<(IconKey, u32), Tile>>,
}

impl IconProducer {
	pub fn new() -> Self {
		Self {
			cache: Mutex::new(HashMap::new()),
		}
	}
}

impl Default for IconProducer {
	fn default() -> Self {
		Self::new()
	}
}

impl Producer for IconProducer {
	fn produce(&self, item: &WorkItem, tile_size: u32) -> Result<Tile, Decline> {
		let key = IconKey::of(&item.path, item.is_dir);

		if let Some(label) = &item.label {
			// Labeled cells embed per-item text, so there is nothing to cache.
			let bgra = render_file_cell(&key, &label.name, &label.detail, tile_size)
				.ok_or_else(|| paint_failed(&key))?;
			return Ok(Tile::new(tile_size, tile_size, bgra));
		}

		let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
		if let Some(tile) = cache.get(&(key.clone(), tile_size)) {
			return Ok(tile.clone());
		}
		let bgra = render_file_icon(&key, tile_size).ok_or_else(|| paint_failed(&key))?;
		let tile = Tile::new(tile_size, tile_size, bgra);
		cache.insert((key, tile_size), tile.clone());
		Ok(tile)
	}
}

fn paint_failed(key: &IconKey) -> Decline {
	Decline::Failed(format!("AppKit could not paint an icon tile for {key:?}"))
}

/// Paint the bare type icon for `key` into a `cell` × `cell` BGRA tile.
pub fn render_file_icon(key: &IconKey, cell: u32) -> Option<Vec<u8>> {
	let _appkit = APPKIT.lock().unwrap_or_else(PoisonError::into_inner);
	render_file_cell_inner(key, None, None, cell)
}

/// Paint the type icon plus the entry's name (centered, tintable by the grid
/// shader) and detail line into a `cell` × `cell` BGRA tile.
pub fn render_file_cell(key: &IconKey, name: &str, detail: &str, cell: u32) -> Option<Vec<u8>> {
	let _appkit = APPKIT.lock().unwrap_or_else(PoisonError::into_inner);
	render_file_cell_inner(key, Some(name), Some(detail), cell)
}

/// The rendered width of a cell's (ellipsized) name as a fraction of the cell,
/// for sizing the selection pill. Clamped to the label width.
pub fn name_width_frac(name: &str) -> f32 {
	let _appkit = APPKIT.lock().unwrap_or_else(PoisonError::into_inner);
	(measure_name_px(name) / 128.0) as f32
}

// `iconForFileType:` is the type-keyed icon lookup. Its UTType-based
// replacement (`iconForContentType:`) isn't exposed by this objc2-app-kit
// version, and the old call still resolves the same Launch Services binding.
#[allow(deprecated)]
fn render_file_cell_inner(
	key: &IconKey,
	name: Option<&str>,
	detail: Option<&str>,
	cell: u32,
) -> Option<Vec<u8>> {
	use objc2::ClassType;
	use objc2_app_kit::{
		NSBitmapImageRep, NSColor, NSCompositingOperation, NSDeviceRGBColorSpace, NSFont,
		NSGraphicsContext, NSImage, NSImageNameFolder, NSWorkspace,
	};
	use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

	objc2::rc::autoreleasepool(|_| unsafe {
		// The standard folder icon for directories; the type icon for files
		// (keyed by extension via Launch Services, so registered apps'
		// document icons show).
		let image: objc2::rc::Retained<NSImage> = match key {
			IconKey::Folder => NSImage::imageNamed(NSImageNameFolder)?,
			IconKey::Ext(ext) => {
				NSWorkspace::sharedWorkspace().iconForFileType(&NSString::from_str(ext))
			}
			IconKey::Generic => {
				NSWorkspace::sharedWorkspace().iconForFileType(&NSString::from_str(""))
			}
		};

		// Draw the icon into an offscreen RGBA bitmap at the cell resolution.
		let px = cell as isize;
		let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
			NSBitmapImageRep::alloc(),
			std::ptr::null_mut(),
			px,
			px,
			8,
			4,
			true,
			false,
			NSDeviceRGBColorSpace,
			px * 4,
			32,
		)?;

		// Start from a transparent buffer so the padding margin (and any
		// see-through areas of the icon) composite to the background grey
		// below.
		let n = (px * px * 4) as usize;
		if !rep.bitmapData().is_null() {
			std::ptr::write_bytes(rep.bitmapData(), 0, n);
		}

		let ctx = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
		let prev = NSGraphicsContext::currentContext();
		NSGraphicsContext::setCurrentContext(Some(&ctx));
		// The layout constants are authored in the 128pt design space; scale
		// them to the actual tile resolution so the cell looks identical at
		// any DPI.
		let s = cell as f64 / 128.0;
		// The box the icon occupies (128pt design space, y-up): labeled cells
		// get a centered box above the text; icon-only cells inset by
		// ICON_PAD. The icon is aspect-fit inside it.
		let (bx, by, bw, bh) = if name.is_some() {
			(
				(128.0 - FILE_ICON_SIZE) * 0.5,
				FILE_ICON_TOP,
				FILE_ICON_SIZE,
				FILE_ICON_SIZE,
			)
		} else {
			let pad = 128.0 * ICON_PAD;
			(pad, pad, 128.0 - 2.0 * pad, 128.0 - 2.0 * pad)
		};
		let dest = aspect_fit(image.size(), bx * s, by * s, bw * s, bh * s);
		let whole = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0));
		image.drawInRect_fromRect_operation_fraction(
			dest,
			whole,
			NSCompositingOperation::SourceOver,
			1.0,
		);

		if let Some(name) = name {
			// The name is baked white on transparent so the shader can tint
			// it (resting grey, white when selected) and draw the pill behind
			// it. Centered using its measured width, which also sizes the
			// pill.
			let name_attrs = text_attrs(
				NSFont::systemFontOfSize(11.0 * s),
				NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 1.0, 1.0, 1.0),
			);
			let w = measure_name_px(name) * s;
			let x = (cell as f64 - w) * 0.5;
			draw_text(name, x, NAME_Y * s, w + s, NAME_H * s, &name_attrs);
		}
		if let Some(detail) = detail {
			let detail_attrs = text_attrs(
				NSFont::systemFontOfSize(9.0 * s),
				NSColor::colorWithSRGBRed_green_blue_alpha(0.58, 0.55, 0.53, 1.0),
			);
			draw_text(
				detail,
				TEXT_X * s,
				DETAIL_Y * s,
				TEXT_W * s,
				DETAIL_H * s,
				&detail_attrs,
			);
		}
		NSGraphicsContext::setCurrentContext(prev.as_deref());

		let data = rep.bitmapData();
		if data.is_null() {
			return None;
		}
		let raw = std::slice::from_raw_parts(data, n);

		// Composite premultiplied RGBA over the background grey → opaque
		// BGRA. The whole tile stays opaque (no transparent rows to bleed a
		// seam); in the name band of a labeled cell the resting grey name is
		// composited in, and the glyph coverage is stashed in the alpha
		// channel so the shader can repaint it white over the selection pill.
		let pxw = cell as usize;
		let band0 = (NAME_BAND_Y0 as f64 * cell as f64) as usize;
		let band1 = (NAME_BAND_Y1 as f64 * cell as f64) as usize;
		let mut out = vec![0u8; n];
		for (i, (o, p)) in out.chunks_exact_mut(4).zip(raw.chunks_exact(4)).enumerate() {
			let cov = p[3] as u32;
			let inv = 255 - cov;
			let row = i / pxw;
			if name.is_some() && row >= band0 && row < band1 {
				// Resting grey name over the cell background; coverage kept
				// in alpha.
				o[0] = ((BG[0] as u32 * inv + NAME_INK[0] as u32 * cov) / 255) as u8;
				o[1] = ((BG[1] as u32 * inv + NAME_INK[1] as u32 * cov) / 255) as u8;
				o[2] = ((BG[2] as u32 * inv + NAME_INK[2] as u32 * cov) / 255) as u8;
				o[3] = cov as u8;
			} else {
				o[0] = (p[2] as u32 + BG[0] as u32 * inv / 255).min(255) as u8;
				o[1] = (p[1] as u32 + BG[1] as u32 * inv / 255).min(255) as u8;
				o[2] = (p[0] as u32 + BG[2] as u32 * inv / 255).min(255) as u8;
				o[3] = 0xff;
			}
		}
		Some(out)
	})
}

/// Measure the name at the design font (128pt space), clamped to the label
/// width.
fn measure_name_px(name: &str) -> f64 {
	use objc2_app_kit::{NSColor, NSFont, NSStringDrawing};
	use objc2_foundation::NSString;
	objc2::rc::autoreleasepool(|_| unsafe {
		let attrs = text_attrs(
			NSFont::systemFontOfSize(11.0),
			NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 1.0, 1.0, 1.0),
		);
		let size = NSString::from_str(name).sizeWithAttributes(Some(&attrs));
		size.width.min(TEXT_W)
	})
}

/// The largest rect with `size`'s aspect ratio that fits inside the box,
/// centered. Never crops or stretches — letterboxes against the (matching)
/// cell background.
fn aspect_fit(
	size: objc2_foundation::NSSize,
	bx: f64,
	by: f64,
	bw: f64,
	bh: f64,
) -> objc2_foundation::NSRect {
	use objc2_foundation::{NSPoint, NSRect, NSSize};
	let (iw, ih) = (size.width, size.height);
	if iw <= 0.0 || ih <= 0.0 {
		return NSRect::new(NSPoint::new(bx, by), NSSize::new(bw, bh));
	}
	let scale = (bw / iw).min(bh / ih);
	let (w, h) = (iw * scale, ih * scale);
	NSRect::new(
		NSPoint::new(bx + (bw - w) * 0.5, by + (bh - h) * 0.5),
		NSSize::new(w, h),
	)
}

fn text_attrs(
	font: objc2::rc::Retained<objc2_app_kit::NSFont>,
	color: objc2::rc::Retained<objc2_app_kit::NSColor>,
) -> objc2::rc::Retained<
	objc2_foundation::NSDictionary<
		objc2_foundation::NSAttributedStringKey,
		objc2::runtime::AnyObject,
	>,
> {
	use objc2::rc::Retained;
	use objc2_app_kit::{NSFontAttributeName, NSForegroundColorAttributeName};
	use objc2_foundation::NSDictionary;

	unsafe {
		NSDictionary::from_vec(
			&[NSFontAttributeName, NSForegroundColorAttributeName],
			vec![Retained::cast(font), Retained::cast(color)],
		)
	}
}

fn draw_text(
	text: &str,
	x: f64,
	y: f64,
	w: f64,
	h: f64,
	attrs: &objc2_foundation::NSDictionary<
		objc2_foundation::NSAttributedStringKey,
		objc2::runtime::AnyObject,
	>,
) {
	use objc2_app_kit::NSStringDrawing;
	use objc2_foundation::{NSPoint, NSString};

	unsafe {
		let text = fit_text(text, w, attrs);
		let ns = NSString::from_str(&text);
		let size = ns.sizeWithAttributes(Some(attrs));
		let dx = ((w - size.width).max(0.0)) * 0.5;
		let dy = ((h - size.height).max(0.0)) * 0.5;
		ns.drawAtPoint_withAttributes(NSPoint::new(x + dx, y + dy), Some(attrs));
	}
}

/// Ellipsize `s` so it renders within `max_width`.
fn fit_text(
	s: &str,
	max_width: f64,
	attrs: &objc2_foundation::NSDictionary<
		objc2_foundation::NSAttributedStringKey,
		objc2::runtime::AnyObject,
	>,
) -> String {
	use objc2_app_kit::NSStringDrawing;
	use objc2_foundation::NSString;

	unsafe {
		if NSString::from_str(s).sizeWithAttributes(Some(attrs)).width <= max_width {
			return s.to_string();
		}
		let chars: Vec<char> = s.chars().collect();
		for len in (1..chars.len()).rev() {
			let candidate = chars[..len].iter().collect::<String>() + "...";
			if NSString::from_str(&candidate)
				.sizeWithAttributes(Some(attrs))
				.width <= max_width
			{
				return candidate;
			}
		}
	}
	"...".to_string()
}

#[cfg(test)]
mod tests {
	use super::*;

	const CELL: u32 = 64;

	fn is_uniform(tile: &Tile) -> bool {
		let first = &tile.bgra()[..4];
		tile.bgra().chunks_exact(4).all(|px| px == first)
	}

	#[test]
	fn paints_type_and_folder_icon_tiles() {
		let producer = IconProducer::new();

		let pdf = producer
			.produce(&WorkItem::file("/nonexistent/report.pdf"), CELL)
			.expect("type icons resolve without the file existing");
		assert_eq!((pdf.width(), pdf.height()), (CELL, CELL));
		assert_eq!(pdf.bgra().len(), (CELL * CELL * 4) as usize);
		assert!(
			!is_uniform(&pdf),
			"an icon should paint over the background"
		);

		let folder = producer
			.produce(&WorkItem::dir("/nonexistent"), CELL)
			.expect("folder icon paints");
		assert!(!is_uniform(&folder));

		// The unlabeled tile is cached; a second produce returns the same
		// pixels.
		let again = producer
			.produce(&WorkItem::file("/elsewhere/other.pdf"), CELL)
			.expect("cached icon");
		assert_eq!(again.bgra(), pdf.bgra());
	}

	#[test]
	fn paints_labeled_cells_with_a_name_band() {
		let producer = IconProducer::new();
		let tile = producer
			.produce(
				&WorkItem::file("/nonexistent/notes.txt").with_label("notes.txt", "12 KB"),
				CELL,
			)
			.expect("labeled cell paints");
		assert_eq!((tile.width(), tile.height()), (CELL, CELL));

		// Glyph coverage is stashed in the alpha channel inside the name
		// band, so some pixel there has non-zero alpha while the rest of the
		// tile is opaque.
		let band0 = (NAME_BAND_Y0 as f64 * CELL as f64) as u32;
		let band1 = (NAME_BAND_Y1 as f64 * CELL as f64) as u32;
		let mut band_has_glyphs = false;
		for y in band0..band1 {
			for x in 0..CELL {
				let i = ((y * CELL + x) * 4) as usize;
				if tile.bgra()[i + 3] > 0 {
					band_has_glyphs = true;
				}
			}
		}
		assert!(
			band_has_glyphs,
			"the baked name should leave glyph coverage in alpha"
		);
	}

	#[test]
	fn icon_key_derivation() {
		assert_eq!(IconKey::of(Path::new("/x"), true), IconKey::Folder);
		assert_eq!(
			IconKey::of(Path::new("/x/Photo.JPG"), false),
			IconKey::Ext("jpg".into())
		);
		assert_eq!(
			IconKey::of(Path::new("/x/Makefile"), false),
			IconKey::Generic
		);
	}
}
