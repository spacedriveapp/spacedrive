use crate::{error::Error, model::MediaCodec, utils::check_error};

use std::{
	ffi::{CStr, CString},
	io::Error as IOError,
	ptr,
};

use ffmpeg_sys_next::{
	av_fourcc_make_string, av_get_bits_per_sample, av_get_media_type_string,
	avcodec_alloc_context3, avcodec_free_context, avcodec_get_name, avcodec_parameters_to_context,
	avcodec_profile_name, AVCodecContext, AVCodecParameters, AVMediaType,
	AV_FOURCC_MAX_STRING_SIZE,
};
use libc::ENOMEM;

#[derive(Debug)]
pub(crate) struct FFmpegCodecContext(*mut AVCodecContext);

impl FFmpegCodecContext {
	pub(crate) fn parameters_to_context(av_dict: *mut AVCodecParameters) -> Result<Self, Error> {
		let ctx = unsafe { avcodec_alloc_context3(ptr::null_mut()) };
		if ctx.is_null() {
			Err(IOError::from_raw_os_error(ENOMEM))?;
		}

		check_error(
			unsafe { avcodec_parameters_to_context(ctx, av_dict) },
			"Fail to fill the codec context with codec parameters",
		)?;

		Ok(Self(ctx))
	}

	unsafe fn as_ref<'a>(&self) -> Option<&'a AVCodecContext> {
		self.0.as_ref()
	}

	fn kind(&self) -> Option<(String, Option<String>)> {
		unsafe { self.as_ref() }.map(|ctx| {
			let media_type = unsafe { av_get_media_type_string(ctx.codec_type) };
			let codec_kind = unsafe { CStr::from_ptr(media_type) };

			(
				codec_kind.to_string_lossy().into_owned(),
				unsafe { ctx.codec.as_ref() }.and_then(|codec| {
					let subkind = unsafe { CStr::from_ptr(codec.name) };
					if codec_kind == subkind {
						None
					} else {
						Some(subkind.to_string_lossy().into_owned())
					}
				}),
			)
		})
	}

	fn name(&self) -> Option<String> {
		unsafe { self.as_ref() }.map(|ctx| {
			let codec_name = unsafe { avcodec_get_name(ctx.codec_id) };
			let cstr = unsafe { CStr::from_ptr(codec_name) };
			String::from_utf8_lossy(cstr.to_bytes()).to_string()
		})
	}

	fn profile(&self) -> Option<String> {
		unsafe { self.as_ref() }.and_then(|ctx| {
			if ctx.profile != 0 {
				let profile = unsafe { avcodec_profile_name(ctx.codec_id, ctx.profile) };
				let cstr = unsafe { CStr::from_ptr(profile) };
				Some(String::from_utf8_lossy(cstr.to_bytes()).to_string())
			} else {
				None
			}
		})
	}

	fn tag(&self) -> Option<String> {
		unsafe { self.as_ref() }.and_then(|ctx| {
			if ctx.codec_tag != 0 {
				CString::new(vec![0; AV_FOURCC_MAX_STRING_SIZE as usize])
					.ok()
					.map(|buffer| {
						let tag = unsafe {
							CString::from_raw(av_fourcc_make_string(
								buffer.into_raw(),
								ctx.codec_tag,
							))
						};
						String::from_utf8_lossy(tag.as_bytes()).to_string()
					})
			} else {
				None
			}
		})
	}

	fn bit_rate(&self) -> Option<i64> {
		unsafe { self.as_ref() }.map(|ctx| match ctx.codec_type {
			AVMediaType::AVMEDIA_TYPE_VIDEO
			| AVMediaType::AVMEDIA_TYPE_DATA
			| AVMediaType::AVMEDIA_TYPE_SUBTITLE
			| AVMediaType::AVMEDIA_TYPE_ATTACHMENT => ctx.bit_rate,
			AVMediaType::AVMEDIA_TYPE_AUDIO => {
				let bits_per_sample = unsafe { av_get_bits_per_sample(ctx.codec_id) };
				if bits_per_sample != 0 {
					let bit_rate = ctx.sample_rate as i64 * ctx.ch_layout.nb_channels as i64;
					if bit_rate <= std::i64::MAX / bits_per_sample as i64 {
						return bit_rate * (bits_per_sample as i64);
					}
				}
				ctx.bit_rate
			}
			_ => 0,
		})
	}
}

impl Drop for FFmpegCodecContext {
	fn drop(&mut self) {
		if !self.0.is_null() {
			unsafe { avcodec_free_context(&mut self.0) };
			self.0 = std::ptr::null_mut();
		}
	}
}

impl From<FFmpegCodecContext> for MediaCodec {
	fn from(val: FFmpegCodecContext) -> Self {
		let (kind, subkind) = match val.kind() {
			Some((kind, subkind)) => (Some(kind), subkind),
			None => (None, None), // Handle the case when self.kind() returns None
		};

		MediaCodec {
			kind,
			subkind,
			name: val.name(),
			profile: val.profile(),
			tag: val.tag(),
			bit_rate: val.bit_rate(),
			props: None,
		}
	}
}
