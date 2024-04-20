use crate::{
	error::Error,
	model::{MediaAudioProps, MediaCodec, MediaSubtitleProps, MediaVideoProps, Props},
	utils::check_error,
};

use std::{
	ffi::{CStr, CString},
	io::Error as IOError,
	ptr,
};

use ffmpeg_sys_next::{
	av_bprint_finalize, av_bprint_init, av_channel_layout_describe_bprint, av_chroma_location_name,
	av_color_primaries_name, av_color_range_name, av_color_space_name, av_color_transfer_name,
	av_fourcc_make_string, av_get_bits_per_sample, av_get_bytes_per_sample,
	av_get_media_type_string, av_get_pix_fmt_name, av_get_sample_fmt_name, av_pix_fmt_desc_get,
	av_reduce, avcodec_alloc_context3, avcodec_free_context, avcodec_get_name,
	avcodec_parameters_to_context, avcodec_profile_name, AVBPrint, AVChromaLocation,
	AVCodecContext, AVCodecParameters, AVColorPrimaries, AVColorRange, AVColorSpace,
	AVColorTransferCharacteristic, AVFieldOrder, AVMediaType, AVPixelFormat, AVRational,
	AVSampleFormat, AV_FOURCC_MAX_STRING_SIZE, FF_CODEC_PROPERTY_CLOSED_CAPTIONS,
	FF_CODEC_PROPERTY_FILM_GRAIN, FF_CODEC_PROPERTY_LOSSLESS,
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

	fn kind(&self) -> (Option<String>, Option<String>) {
		unsafe { self.as_ref() }
			.map(|ctx| {
				let kind = unsafe { av_get_media_type_string(ctx.codec_type).as_ref() }
					.map(|media_type| unsafe { CStr::from_ptr(media_type) });

				let subkind = unsafe { ctx.codec.as_ref() }
					.and_then(|codec| unsafe { codec.name.as_ref() })
					.map(|name| unsafe { CStr::from_ptr(name) })
					.and_then(|subkind| {
						if let Some(kind) = kind {
							if kind == subkind {
								return None;
							}
						}

						Some(String::from_utf8_lossy(subkind.to_bytes()).to_string())
					});

				(
					kind.map(|cstr| String::from_utf8_lossy(cstr.to_bytes()).to_string()),
					subkind,
				)
			})
			.unwrap_or((None, None))
	}

	fn name(&self) -> Option<String> {
		unsafe { self.as_ref() }.and_then(|ctx| {
			unsafe { avcodec_get_name(ctx.codec_id).as_ref() }.map(|codec_name| {
				let cstr = unsafe { CStr::from_ptr(codec_name) };
				String::from_utf8_lossy(cstr.to_bytes()).to_string()
			})
		})
	}

	fn profile(&self) -> Option<String> {
		unsafe { self.as_ref() }.and_then(|ctx| {
			if ctx.profile == 0 {
				None
			} else {
				unsafe { avcodec_profile_name(ctx.codec_id, ctx.profile).as_ref() }.map(|profile| {
					let cstr = unsafe { CStr::from_ptr(profile) };
					String::from_utf8_lossy(cstr.to_bytes()).to_string()
				})
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

	fn video_props(&self) -> Option<MediaVideoProps> {
		unsafe { self.as_ref() }.and_then(|ctx| {
			if ctx.codec_type != AVMediaType::AVMEDIA_TYPE_VIDEO {
				return None;
			}

			let pixel_format = if ctx.pix_fmt == AVPixelFormat::AV_PIX_FMT_NONE {
				None
			} else {
				unsafe { av_get_pix_fmt_name(ctx.pix_fmt).as_ref() }.map(|pixel_format| {
					let cstr = unsafe { CStr::from_ptr(pixel_format) };
					String::from_utf8_lossy(cstr.to_bytes()).to_string()
				})
			};

			let bits_per_channel =
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
				};

			let color_range = if ctx.color_range == AVColorRange::AVCOL_RANGE_UNSPECIFIED {
				None
			} else {
				unsafe { av_color_range_name(ctx.color_range).as_ref() }.map(|color_range| {
					let cstr = unsafe { CStr::from_ptr(color_range) };
					String::from_utf8_lossy(cstr.to_bytes()).to_string()
				})
			};

			let (color_space, color_primaries, color_transfer) = if ctx.colorspace
				== AVColorSpace::AVCOL_SPC_UNSPECIFIED
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
				let color_primaries =
					unsafe { av_color_primaries_name(ctx.color_primaries).as_ref() }.map(
						|color_primaries| {
							let cstr = unsafe { CStr::from_ptr(color_primaries) };
							String::from_utf8_lossy(cstr.to_bytes()).to_string()
						},
					);
				let color_transfer = unsafe { av_color_transfer_name(ctx.color_trc).as_ref() }.map(
					|color_transfer| {
						let cstr = unsafe { CStr::from_ptr(color_transfer) };
						String::from_utf8_lossy(cstr.to_bytes()).to_string()
					},
				);

				(color_space, color_primaries, color_transfer)
			};

			// Field Order
			let field_order = if ctx.field_order == AVFieldOrder::AV_FIELD_UNKNOWN {
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
			};

			// Chroma Sample Location
			let chroma_location =
				if ctx.chroma_sample_location == AVChromaLocation::AVCHROMA_LOC_UNSPECIFIED {
					None
				} else {
					unsafe { av_chroma_location_name(ctx.chroma_sample_location).as_ref() }.map(
						|chroma_location| {
							let cstr = unsafe { CStr::from_ptr(chroma_location) };
							String::from_utf8_lossy(cstr.to_bytes()).to_string()
						},
					)
				};

			let width = ctx.width;
			let height = ctx.height;

			let (aspect_ratio_num, aspect_ratio_den) = if ctx.sample_aspect_ratio.num == 0 {
				(None, None)
			} else {
				let mut display_aspect_ratio = AVRational { num: 0, den: 0 };
				let num = (width * ctx.sample_aspect_ratio.num) as i64;
				let den = (height * ctx.sample_aspect_ratio.den) as i64;
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
			};

			let mut properties = vec![];
			if ctx.properties & (FF_CODEC_PROPERTY_LOSSLESS as u32) != 0 {
				properties.push("Closed Captions".to_string());
			}
			if ctx.properties & (FF_CODEC_PROPERTY_CLOSED_CAPTIONS as u32) != 0 {
				properties.push("Film Grain".to_string());
			}
			if ctx.properties & (FF_CODEC_PROPERTY_FILM_GRAIN as u32) != 0 {
				properties.push("lossless".to_string());
			}

			Some(MediaVideoProps {
				pixel_format,
				bits_per_channel,
				color_range,
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
		})
	}

	fn audio_props(&self) -> Option<MediaAudioProps> {
		unsafe { self.as_ref() }.and_then(|ctx| {
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
				av_bprint_init(&mut bprint, 0, u32::MAX /* AV_BPRINT_SIZE_UNLIMITED */)
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

			let sample_format = if ctx.sample_fmt != AVSampleFormat::AV_SAMPLE_FMT_NONE {
				unsafe { av_get_sample_fmt_name(ctx.sample_fmt).as_ref() }.map(|sample_fmt| {
					let cstr = unsafe { CStr::from_ptr(sample_fmt) };
					String::from_utf8_lossy(cstr.to_bytes()).to_string()
				})
			} else {
				None
			};

			let bit_per_sample = if ctx.bits_per_raw_sample > 0
				&& ctx.bits_per_raw_sample != unsafe { av_get_bytes_per_sample(ctx.sample_fmt) } * 8
			{
				Some(ctx.bits_per_raw_sample)
			} else {
				None
			};

			Some(MediaAudioProps {
				delay: ctx.initial_padding,
				padding: ctx.trailing_padding,
				sample_rate,
				sample_format,
				bit_per_sample,
				channel_layout,
			})
		})
	}

	fn subtitle_props(&self) -> Option<MediaSubtitleProps> {
		unsafe { self.as_ref() }.and_then(|ctx| {
			if ctx.codec_type != AVMediaType::AVMEDIA_TYPE_SUBTITLE {
				return None;
			}

			Some(MediaSubtitleProps {
				width: ctx.width,
				height: ctx.height,
			})
		})
	}

	fn props(&self) -> Option<Props> {
		unsafe { self.as_ref() }.and_then(|ctx| match ctx.codec_type {
			AVMediaType::AVMEDIA_TYPE_VIDEO => self.video_props().map(Props::Video),
			AVMediaType::AVMEDIA_TYPE_AUDIO => self.audio_props().map(Props::Audio),
			AVMediaType::AVMEDIA_TYPE_SUBTITLE => self.subtitle_props().map(Props::Subtitle),
			_ => None,
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
	fn from(ctx: FFmpegCodecContext) -> Self {
		let (kind, subkind) = ctx.kind();

		MediaCodec {
			kind,
			subkind,
			name: ctx.name(),
			profile: ctx.profile(),
			tag: ctx.tag(),
			bit_rate: ctx.bit_rate(),
			props: ctx.props(),
		}
	}
}
