//! Thin wrapper over the macOS system codecs, the same ones Finder and Photos
//! use. ImageIO decodes any format it supports (HEIC, RAW/ARW/DNG, JPEG, PNG,
//! WebP, AVIF, GIF, PSD) on the hardware path and applies EXIF orientation;
//! QuickLook renders a content preview for anything else Finder can preview
//! (PDF pages, video posters, documents). No bundled libraries, all in-process.
//!
//! The crate compiles to nothing on other platforms, where `sd-images` and
//! `sd-ffmpeg` keep their existing roles behind the `heif` and `ffmpeg`
//! features.

#![cfg(target_os = "macos")]

use core_foundation::base::{CFRelease, CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::number::CFNumber;
use core_foundation::string::{CFString, CFStringRef};
use core_foundation::url::CFURL;
use std::os::raw::c_void;
use std::path::Path;

type CGImageSourceRef = *const c_void;
type CGImageDestinationRef = *const c_void;
type CGImageRef = *const c_void;

#[link(name = "ImageIO", kind = "framework")]
extern "C" {
	fn CGImageSourceCreateWithURL(url: CFTypeRef, options: CFDictionaryRef) -> CGImageSourceRef;
	fn CGImageSourceCreateThumbnailAtIndex(
		isrc: CGImageSourceRef,
		index: usize,
		options: CFDictionaryRef,
	) -> CGImageRef;
	fn CGImageDestinationCreateWithData(
		data: CFTypeRef,
		ty: CFStringRef,
		count: usize,
		options: CFDictionaryRef,
	) -> CGImageDestinationRef;
	fn CGImageDestinationAddImage(
		dest: CGImageDestinationRef,
		image: CGImageRef,
		props: CFDictionaryRef,
	);
	fn CGImageDestinationFinalize(dest: CGImageDestinationRef) -> bool;

	static kCGImageSourceCreateThumbnailFromImageAlways: CFStringRef;
	static kCGImageSourceCreateThumbnailFromImageIfAbsent: CFStringRef;
	static kCGImageSourceThumbnailMaxPixelSize: CFStringRef;
	static kCGImageSourceCreateThumbnailWithTransform: CFStringRef;
	static kCGImageDestinationLossyCompressionQuality: CFStringRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
	fn CFDataCreateMutable(allocator: CFTypeRef, capacity: isize) -> CFTypeRef;
	fn CFDataGetLength(data: CFTypeRef) -> isize;
	fn CFDataGetBytePtr(data: CFTypeRef) -> *const u8;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
	fn CGImageGetWidth(image: CGImageRef) -> usize;
	fn CGImageGetHeight(image: CGImageRef) -> usize;
}

// ImageIO autoreleases internal temporaries. Worker threads under `spawn_blocking`
// have no pool of their own, so one is drained per asset to keep memory flat over
// a long indexing run.
#[link(name = "objc", kind = "dylib")]
extern "C" {
	fn objc_autoreleasePoolPush() -> *mut c_void;
	fn objc_autoreleasePoolPop(ctx: *mut c_void);
}

#[repr(C)]
struct CGSize {
	width: f64,
	height: f64,
}

// QuickLook's legacy synchronous thumbnailer produces a content preview for
// anything Finder can preview, as a CGImage. Deprecated but still functional, and
// synchronous, which suits the blocking generator model. `QLThumbnailGenerator` in
// `QuickLookThumbnailing` is the modern async replacement if this is withdrawn.
#[link(name = "QuickLook", kind = "framework")]
extern "C" {
	fn QLThumbnailImageCreate(
		allocator: CFTypeRef,
		url: CFTypeRef,
		max_size: CGSize,
		options: CFDictionaryRef,
	) -> CGImageRef;
}

/// Run `f` inside a fresh Objective-C autorelease pool.
pub fn autoreleased<R>(f: impl FnOnce() -> R) -> R {
	unsafe {
		let pool = objc_autoreleasePoolPush();
		let r = f();
		objc_autoreleasePoolPop(pool);
		r
	}
}

/// An opened image. Decoding cost is paid lazily per thumbnail, so one source can
/// emit several sizes cheaply.
pub struct Source {
	src: CGImageSourceRef,
}

impl Source {
	pub fn open(path: &Path) -> Option<Source> {
		unsafe {
			let url = CFURL::from_path(path, false)?;
			let src = CGImageSourceCreateWithURL(url.as_CFTypeRef(), std::ptr::null());
			if src.is_null() {
				None
			} else {
				Some(Source { src })
			}
		}
	}

	/// Create a thumbnail CGImage whose longest side is `max_px`, EXIF-oriented.
	/// `from_image` forces a full-resolution decode for best quality; when false,
	/// ImageIO reuses an embedded thumbnail if the file has one, which is faster.
	/// The caller owns the returned ref and must release it.
	unsafe fn make_thumbnail(&self, max_px: i64, from_image: bool) -> CGImageRef {
		let key_kind = if from_image {
			kCGImageSourceCreateThumbnailFromImageAlways
		} else {
			kCGImageSourceCreateThumbnailFromImageIfAbsent
		};
		let key_kind = CFString::wrap_under_get_rule(key_kind);
		let key_xform = CFString::wrap_under_get_rule(kCGImageSourceCreateThumbnailWithTransform);
		let key_max = CFString::wrap_under_get_rule(kCGImageSourceThumbnailMaxPixelSize);
		let opts: CFDictionary<CFString, CFType> = CFDictionary::from_CFType_pairs(&[
			(key_kind, CFBoolean::true_value().as_CFType()),
			(key_xform, CFBoolean::true_value().as_CFType()),
			(key_max, CFNumber::from(max_px).as_CFType()),
		]);
		CGImageSourceCreateThumbnailAtIndex(self.src, 0, opts.as_concrete_TypeRef())
	}

	unsafe fn jpeg_props(quality: f64) -> CFDictionary<CFString, CFType> {
		let key_q = CFString::wrap_under_get_rule(kCGImageDestinationLossyCompressionQuality);
		CFDictionary::from_CFType_pairs(&[(key_q, CFNumber::from(quality).as_CFType())])
	}

	/// Render a thumbnail whose longest side is `max_px` and return it as JPEG
	/// bytes in memory. An embedded thumbnail is reused when the file carries one,
	/// which skips a full decode, but those are frequently smaller than requested,
	/// so an undersized result is re-rendered from the full image.
	pub fn thumbnail_jpeg(&self, max_px: i64, quality: f64) -> Option<Vec<u8>> {
		unsafe {
			let mut img = self.make_thumbnail(max_px, false);
			if !img.is_null() && longest_side(img) < max_px {
				let full = self.make_thumbnail(max_px, true);
				if !full.is_null() {
					CFRelease(img as CFTypeRef);
					img = full;
				}
			}
			if img.is_null() {
				return None;
			}
			let jpeg = cgimage_to_jpeg(img, quality);
			CFRelease(img as CFTypeRef);
			jpeg
		}
	}
}

impl Drop for Source {
	fn drop(&mut self) {
		unsafe { CFRelease(self.src as CFTypeRef) }
	}
}

unsafe fn longest_side(img: CGImageRef) -> i64 {
	CGImageGetWidth(img).max(CGImageGetHeight(img)) as i64
}

/// Encode a CGImage to JPEG bytes in memory. The caller still owns `img`.
unsafe fn cgimage_to_jpeg(img: CGImageRef, quality: f64) -> Option<Vec<u8>> {
	let data = CFDataCreateMutable(std::ptr::null(), 0);
	if data.is_null() {
		return None;
	}
	let jpeg_ty = CFString::new("public.jpeg");
	let dest =
		CGImageDestinationCreateWithData(data, jpeg_ty.as_concrete_TypeRef(), 1, std::ptr::null());
	if dest.is_null() {
		CFRelease(data);
		return None;
	}
	let props = Source::jpeg_props(quality);
	CGImageDestinationAddImage(dest, img, props.as_concrete_TypeRef());
	let ok = CGImageDestinationFinalize(dest);

	let result = if ok {
		let len = CFDataGetLength(data) as usize;
		let ptr = CFDataGetBytePtr(data);
		if ptr.is_null() || len == 0 {
			None
		} else {
			Some(std::slice::from_raw_parts(ptr, len).to_vec())
		}
	} else {
		None
	};
	CFRelease(dest as CFTypeRef);
	CFRelease(data);
	result
}

/// A content thumbnail for any file, as JPEG bytes with its longest side around
/// `max_px`. Images including RAW and HEIC go through ImageIO; everything else
/// Finder can preview (PDF, video, documents) falls back to QuickLook. `None`
/// means neither produced a preview, and the caller shows the file's type icon.
///
/// `quality` is ImageIO's lossy compression factor, from 0.0 to 1.0.
pub fn file_thumbnail_jpeg(path: &Path, max_px: i64, quality: f64) -> Option<Vec<u8>> {
	autoreleased(|| unsafe {
		if let Some(src) = Source::open(path) {
			if let Some(jpeg) = src.thumbnail_jpeg(max_px, quality) {
				return Some(jpeg);
			}
		}
		let url = CFURL::from_path(path, false)?;
		let size = CGSize {
			width: max_px as f64,
			height: max_px as f64,
		};
		let img =
			QLThumbnailImageCreate(std::ptr::null(), url.as_CFTypeRef(), size, std::ptr::null());
		if img.is_null() {
			return None;
		}
		let jpeg = cgimage_to_jpeg(img, quality);
		CFRelease(img as CFTypeRef);
		jpeg
	})
}
