use crate::{
	error::{Error, FFmpegError},
	model::{FFmpegAudioProps, FFmpegCodec, FFmpegProps, FFmpegSubtitleProps, FFmpegVideoProps},
	utils::check_error,
};

use std::{
	ffi::{CStr, CString},
	ptr,
};

use ffmpeg_sys_next::{
	av_bprint_finalize, av_bprint_init, av_channel_layout_describe_bprint, av_chroma_location_name,
	av_color_primaries_name, av_color_range_name, av_color_space_name, av_color_transfer_name,
	av_fourcc_make_string, av_get_bits_per_sample, av_get_bytes_per_sample,
	av_get_media_type_string, av_get_pix_fmt_name, av_get_sample_fmt_name, av_pix_fmt_desc_get,
	av_reduce, avcodec_alloc_context3, avcodec_flush_buffers, avcodec_free_context,
	avcodec_get_name, avcodec_open2, avcodec_parameters_to_context, avcodec_profile_name,
	avcodec_receive_frame, avcodec_send_packet, AVBPrint, AVChromaLocation, AVCodec,
	AVCodecContext, AVCodecParameters, AVColorPrimaries, AVColorRange, AVColorSpace,
	AVColorTransferCharacteristic, AVFieldOrder, AVFrame, AVMediaType, AVPacket, AVPixelFormat,
	AVRational, AVSampleFormat, AVERROR, AVERROR_EOF, AV_FOURCC_MAX_STRING_SIZE,
	FF_CODEC_PROPERTY_CLOSED_CAPTIONS, FF_CODEC_PROPERTY_FILM_GRAIN, FF_CODEC_PROPERTY_LOSSLESS,
};
use libc::EAGAIN;

pub struct FFmpegCodecContext(*mut AVCodecContext);

impl FFmpegCodecContext {
	pub(crate) fn new() -> Result<Self, Error> {
		let ptr = unsafe { avcodec_alloc_context3(ptr::null_mut()) };
		if ptr.is_null() {
			Err(FFmpegError::VideoCodecAllocation)?;
		}

		Ok(Self(ptr))
	}

	pub(crate) fn as_ref(&self) -> &AVCodecContext {
		unsafe { self.0.as_ref() }.expect("initialized on struct creation")
	}

	pub(crate) fn as_mut(&mut self) -> &mut AVCodecContext {
		unsafe { self.0.as_mut() }.expect("initialized on struct creation")
	}

	pub(crate) fn parameters_to_context(
		&mut self,
		codec_params: &AVCodecParameters,
	) -> Result<&Self, Error> {
		check_error(
			unsafe { avcodec_parameters_to_context(self.as_mut(), codec_params) },
			"Fail to fill the codec context with codec parameters",
		)?;

		Ok(self)
	}

	pub(crate) fn open2(&mut self, video_codec: &AVCodec) -> Result<&Self, Error> {
		check_error(
			unsafe { avcodec_open2(self.as_mut(), video_codec, ptr::null_mut()) },
			"Failed to open video codec",
		)?;

		Ok(self)
	}

	pub(crate) fn flush(&mut self) {
		unsafe { avcodec_flush_buffers(self.as_mut()) };
	}

	pub(crate) fn send_packet(&mut self, packet: *mut AVPacket) -> Result<bool, FFmpegError> {
		match unsafe { avcodec_send_packet(self.as_mut(), packet) } {
			AVERROR_EOF => Ok(false),
			ret if ret == AVERROR(EAGAIN) => Err(FFmpegError::Again),
			ret if ret < 0 => Err(FFmpegError::from(ret)),
			_ => Ok(true),
		}
	}

	pub(crate) fn receive_frame(&mut self, frame: *mut AVFrame) -> Result<bool, FFmpegError> {
		match unsafe { avcodec_receive_frame(self.as_mut(), frame) } {
			AVERROR_EOF => Ok(false),
			ret if ret == AVERROR(EAGAIN) => Err(FFmpegError::Again),
			ret if ret < 0 => Err(FFmpegError::from(ret)),
			_ => Ok(true),
		}
	}

	fn kind(&self) -> (Option<String>, Option<String>) {
		let kind = unsafe { av_get_media_type_string(self.as_ref().codec_type).as_ref() }
			.map(|media_type| unsafe { CStr::from_ptr(media_type) });

		let sub_kind = unsafe { self.as_ref().codec.as_ref() }
			.and_then(|codec| unsafe { codec.name.as_ref() })
			.map(|name| unsafe { CStr::from_ptr(name) })
			.and_then(|sub_kind| {
				if let Some(kind) = kind {
					if kind == sub_kind {
						return None;
					}
				}

				Some(String::from_utf8_lossy(sub_kind.to_bytes()).to_string())
			});

		(
			kind.map(|cstr| String::from_utf8_lossy(cstr.to_bytes()).to_string()),
			sub_kind,
		)
	}

	fn name(&self) -> Option<String> {
		unsafe { avcodec_get_name(self.as_ref().codec_id).as_ref() }.map(|codec_name| {
			let cstr = unsafe { CStr::from_ptr(codec_name) };
			String::from_utf8_lossy(cstr.to_bytes()).to_string()
		})
	}

	fn profile(&self) -> Option<String> {
		if self.as_ref().profile == 0 {
			None
		} else {
			unsafe { avcodec_profile_name(self.as_ref().codec_id, self.as_ref().profile).as_ref() }
				.map(|profile| {
					let cstr = unsafe { CStr::from_ptr(profile) };
					String::from_utf8_lossy(cstr.to_bytes()).to_string()
				})
		}
	}

	fn tag(&self) -> Option<String> {
		if self.as_ref().codec_tag != 0 {
			CString::new(vec![
				0;
				usize::try_from(AV_FOURCC_MAX_STRING_SIZE).expect(
					"AV_FOURCC_MAX_STRING_SIZE is 32, must fit in an usize"
				)
			])
			.ok()
			.map(|buffer| {
				let tag = unsafe {
					CString::from_raw(av_fourcc_make_string(
						buffer.into_raw(),
						self.as_ref().codec_tag,
					))
				};
				String::from_utf8_lossy(tag.as_bytes()).to_string()
			})
		} else {
			None
		}
	}

	fn bit_rate(&self) -> i32 {
		// TODO: use i64 instead of i32 when rspc supports it
		let ctx = self.as_ref();
		match self.as_ref().codec_type {
			AVMediaType::AVMEDIA_TYPE_VIDEO
			| AVMediaType::AVMEDIA_TYPE_DATA
			| AVMediaType::AVMEDIA_TYPE_SUBTITLE
			| AVMediaType::AVMEDIA_TYPE_ATTACHMENT => ctx.bit_rate.try_into().unwrap_or_default(),
			AVMediaType::AVMEDIA_TYPE_AUDIO => {
				let bits_per_sample = unsafe { av_get_bits_per_sample(ctx.codec_id) };
				if bits_per_sample != 0 {
					let bit_rate = ctx.sample_rate * ctx.ch_layout.nb_channels;
					if bit_rate <= i32::MAX / bits_per_sample {
						return bit_rate * (bits_per_sample);
					}
				}
				ctx.bit_rate.try_into().unwrap_or_default()
			}
			_ => 0,
		}
	}

	fn video_props(&self) -> Option<FFmpegVideoProps> {
		let ctx = self.as_ref();
		if ctx.codec_type != AVMediaType::AVMEDIA_TYPE_VIDEO {
			return None;
		}

		let pixel_format = extract_pixel_format(ctx);

		let bits_per_channel = extract_bits_per_channel(ctx);

		let color_range = extract_color_range(ctx);

		let (color_space, color_primaries, color_transfer) = extract_colors(ctx);

		// Field Order
		let field_order = extract_field_order(ctx);

		// Chroma Sample Location
		let chroma_location = extract_chroma_location(ctx);

		let width = ctx.width;
		let height = ctx.height;

		let (aspect_ratio_num, aspect_ratio_den) = extract_aspect_ratio(ctx, width, height);

		let mut properties = vec![];
		if ctx.properties & (FF_CODEC_PROPERTY_LOSSLESS.unsigned_abs()) != 0 {
			properties.push("Closed Captions".to_string());
		}
		if ctx.properties & (FF_CODEC_PROPERTY_CLOSED_CAPTIONS.unsigned_abs()) != 0 {
			properties.push("Film Grain".to_string());
		}
		if ctx.properties & (FF_CODEC_PROPERTY_FILM_GRAIN.unsigned_abs()) != 0 {
			properties.push("lossless".to_string());
		}

		Some(FFmpegVideoProps {
			pixel_format,
			color_range,
			bits_per_channel,
			color_space,
			color_primaries,
			color_transfer,
			field_order,
			chroma_location,
			width,
			height,
			aspect_ratio_num,
			aspect_ratio_den,
			properties,
		})
	}

	fn audio_props(&self) -> Option<FFmpegAudioProps> {
		let ctx = self.as_ref();
		if ctx.codec_type != AVMediaType::AVMEDIA_TYPE_AUDIO {
			return None;
		}

		let sample_rate = if ctx.sample_rate > 0 {
			Some(ctx.sample_rate)
		} else {
			None
		};

		let mut bprint = AVBPrint {
			str_: ptr::null_mut(),
			len: 0,
			size: 0,
			size_max: 0,
			reserved_internal_buffer: [0; 1],
			reserved_padding: [0; 1000],
		};
		unsafe {
			av_bprint_init(&mut bprint, 0, u32::MAX /* AV_BPRINT_SIZE_UNLIMITED */);
		};
		let mut channel_layout = ptr::null_mut();
		let channel_layout =
			if unsafe { av_channel_layout_describe_bprint(&ctx.ch_layout, &mut bprint) } < 0
				|| unsafe { av_bprint_finalize(&mut bprint, &mut channel_layout) } < 0
				|| channel_layout.is_null()
			{
				None
			} else {
				let cstr = unsafe { CStr::from_ptr(channel_layout) };
				Some(String::from_utf8_lossy(cstr.to_bytes()).to_string())
			};

		let sample_format = if ctx.sample_fmt == AVSampleFormat::AV_SAMPLE_FMT_NONE {
			None
		} else {
			unsafe { av_get_sample_fmt_name(ctx.sample_fmt).as_ref() }.map(|sample_fmt| {
				let cstr = unsafe { CStr::from_ptr(sample_fmt) };
				String::from_utf8_lossy(cstr.to_bytes()).to_string()
			})
		};

		let bit_per_sample = if ctx.bits_per_raw_sample > 0
			&& ctx.bits_per_raw_sample != unsafe { av_get_bytes_per_sample(ctx.sample_fmt) } * 8
		{
			Some(ctx.bits_per_raw_sample)
		} else {
			None
		};

		Some(FFmpegAudioProps {
			delay: ctx.initial_padding,
			padding: ctx.trailing_padding,
			sample_rate,
			sample_format,
			bit_per_sample,
			channel_layout,
		})
	}

	fn subtitle_props(&self) -> Option<FFmpegSubtitleProps> {
		if self.as_ref().codec_type != AVMediaType::AVMEDIA_TYPE_SUBTITLE {
			return None;
		}

		Some(FFmpegSubtitleProps {
			width: self.as_ref().width,
			height: self.as_ref().height,
		})
	}

	fn props(&self) -> Option<FFmpegProps> {
		match self.as_ref().codec_type {
			AVMediaType::AVMEDIA_TYPE_VIDEO => self.video_props().map(FFmpegProps::Video),
			AVMediaType::AVMEDIA_TYPE_AUDIO => self.audio_props().map(FFmpegProps::Audio),
			AVMediaType::AVMEDIA_TYPE_SUBTITLE => self.subtitle_props().map(FFmpegProps::Subtitle),
			_ => None,
		}
	}
}

fn extract_aspect_ratio(
	ctx: &AVCodecContext,
	width: i32,
	height: i32,
) -> (Option<i32>, Option<i32>) {
	if ctx.sample_aspect_ratio.num == 0 {
		(None, None)
	} else {
		let mut display_aspect_ratio = AVRational { num: 0, den: 0 };
		let num = i64::from(width * ctx.sample_aspect_ratio.num);
		let den = i64::from(height * ctx.sample_aspect_ratio.den);
		let max = 1024 * 1024;
		unsafe {
			av_reduce(
				&mut display_aspect_ratio.num,
				&mut display_aspect_ratio.den,
				num,
				den,
				max,
			);
		}

		(
			Some(display_aspect_ratio.num),
			Some(display_aspect_ratio.den),
		)
	}
}

fn extract_chroma_location(ctx: &AVCodecContext) -> Option<String> {
	if ctx.chroma_sample_location == AVChromaLocation::AVCHROMA_LOC_UNSPECIFIED {
		None
	} else {
		unsafe { av_chroma_location_name(ctx.chroma_sample_location).as_ref() }.map(
			|chroma_location| {
				let cstr = unsafe { CStr::from_ptr(chroma_location) };
				String::from_utf8_lossy(cstr.to_bytes()).to_string()
			},
		)
	}
}

fn extract_field_order(ctx: &AVCodecContext) -> Option<String> {
	if ctx.field_order == AVFieldOrder::AV_FIELD_UNKNOWN {
		None
	} else {
		Some(
			(match ctx.field_order {
				AVFieldOrder::AV_FIELD_TT => "top first",
				AVFieldOrder::AV_FIELD_BB => "bottom first",
				AVFieldOrder::AV_FIELD_TB => "top coded first (swapped)",
				AVFieldOrder::AV_FIELD_BT => "bottom coded first (swapped)",
				_ => "progressive",
			})
			.to_string(),
		)
	}
}

fn extract_colors(ctx: &AVCodecContext) -> (Option<String>, Option<String>, Option<String>) {
	if ctx.colorspace == AVColorSpace::AVCOL_SPC_UNSPECIFIED
		&& ctx.color_primaries == AVColorPrimaries::AVCOL_PRI_UNSPECIFIED
		&& ctx.color_trc == AVColorTransferCharacteristic::AVCOL_TRC_UNSPECIFIED
	{
		(None, None, None)
	} else {
		let color_space =
			unsafe { av_color_space_name(ctx.colorspace).as_ref() }.map(|color_space| {
				let cstr = unsafe { CStr::from_ptr(color_space) };
				String::from_utf8_lossy(cstr.to_bytes()).to_string()
			});
		let color_primaries = unsafe { av_color_primaries_name(ctx.color_primaries).as_ref() }.map(
			|color_primaries| {
				let cstr = unsafe { CStr::from_ptr(color_primaries) };
				String::from_utf8_lossy(cstr.to_bytes()).to_string()
			},
		);
		let color_transfer =
			unsafe { av_color_transfer_name(ctx.color_trc).as_ref() }.map(|color_transfer| {
				let cstr = unsafe { CStr::from_ptr(color_transfer) };
				String::from_utf8_lossy(cstr.to_bytes()).to_string()
			});

		(color_space, color_primaries, color_transfer)
	}
}

fn extract_color_range(ctx: &AVCodecContext) -> Option<String> {
	if ctx.color_range == AVColorRange::AVCOL_RANGE_UNSPECIFIED {
		None
	} else {
		unsafe { av_color_range_name(ctx.color_range).as_ref() }.map(|color_range| {
			let cstr = unsafe { CStr::from_ptr(color_range) };
			String::from_utf8_lossy(cstr.to_bytes()).to_string()
		})
	}
}

fn extract_bits_per_channel(ctx: &AVCodecContext) -> Option<i32> {
	if ctx.bits_per_raw_sample == 0 || ctx.pix_fmt == AVPixelFormat::AV_PIX_FMT_NONE {
		None
	} else {
		unsafe { av_pix_fmt_desc_get(ctx.pix_fmt).as_ref() }.and_then(|pix_fmt_desc| {
			let comp = pix_fmt_desc.comp[0];
			if ctx.bits_per_raw_sample < comp.depth {
				Some(ctx.bits_per_raw_sample)
			} else {
				None
			}
		})
	}
}

fn extract_pixel_format(ctx: &AVCodecContext) -> Option<String> {
	if ctx.pix_fmt == AVPixelFormat::AV_PIX_FMT_NONE {
		None
	} else {
		unsafe { av_get_pix_fmt_name(ctx.pix_fmt).as_ref() }.map(|pixel_format| {
			let cstr = unsafe { CStr::from_ptr(pixel_format) };
			String::from_utf8_lossy(cstr.to_bytes()).to_string()
		})
	}
}

impl Drop for FFmpegCodecContext {
	fn drop(&mut self) {
		if !self.0.is_null() {
			unsafe { avcodec_free_context(&mut self.0) };
			self.0 = ptr::null_mut();
		}
	}
}

impl From<&FFmpegCodecContext> for FFmpegCodec {
	fn from(ctx: &FFmpegCodecContext) -> Self {
		let (kind, sub_kind) = ctx.kind();

		Self {
			kind,
			sub_kind,
			name: ctx.name(),
			profile: ctx.profile(),
			tag: ctx.tag(),
			bit_rate: ctx.bit_rate(),
			props: ctx.props(),
		}
	}
}
