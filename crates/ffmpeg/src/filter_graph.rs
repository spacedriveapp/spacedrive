use std::{ffi::CString, ptr};

use crate::{
	codec_ctx::FFmpegCodecContext,
	error::FFmpegError,
	frame_decoder::ThumbnailSize,
	utils::{check_error, CSTRING_ERROR_MSG},
	Error,
};
use ffmpeg_sys_next::{
	avfilter_get_by_name, avfilter_graph_alloc, avfilter_graph_config,
	avfilter_graph_create_filter, avfilter_graph_free, avfilter_link, AVFilterContext,
	AVFilterGraph, AVRational,
};

pub(crate) struct FFmpegFilterGraph(*mut AVFilterGraph);

impl<'a> FFmpegFilterGraph {
	pub(crate) fn new() -> Result<Self, FFmpegError> {
		let ptr = unsafe { avfilter_graph_alloc() };
		if ptr.is_null() {
			return Err(FFmpegError::FrameAllocation);
		}
		Ok(Self(ptr))
	}

	fn link(
		src: *mut AVFilterContext,
		srcpad: u32,
		dst: *mut AVFilterContext,
		dstpad: u32,
		error: &str,
	) -> Result<(), Error> {
		check_error(unsafe { avfilter_link(src, srcpad, dst, dstpad) }, error)?;
		Ok(())
	}

	pub(crate) fn thumbnail_graph(
		size: Option<ThumbnailSize>,
		time_base: &AVRational,
		codec_ctx: &FFmpegCodecContext,
		aspect_ratio: &AVRational,
		rotation_angle: f64,
		interlaced_frame: bool,
		maintain_aspect_ratio: bool,
	) -> Result<(Self, &'a mut AVFilterContext, &'a mut AVFilterContext), Error> {
		let mut filter_graph = Self::new()?;

		let args = format!(
			"video_size={}x{}:pix_fmt={}:time_base={}/{}:pixel_aspect={}/{}",
			codec_ctx.as_ref().width,
			codec_ctx.as_ref().height,
			codec_ctx.as_ref().pix_fmt as i32,
			time_base.num,
			time_base.den,
			codec_ctx.as_ref().sample_aspect_ratio.num,
			i32::max(codec_ctx.as_ref().sample_aspect_ratio.den, 1)
		);

		let filter_source = ptr::null_mut();
		filter_graph.setup_filter(
			filter_source,
			CString::new("buffer").expect(CSTRING_ERROR_MSG),
			CString::new("thumb_buffer").expect(CSTRING_ERROR_MSG),
			Some(CString::new(args)?),
			"Failed to create filter source",
		)?;
		let filter_source_ctx = unsafe { filter_source.as_mut() }
			.as_deref()
			.and_then(|ptr| unsafe { ptr.as_mut() })
			.ok_or(FFmpegError::NullError)?;

		let filter_sink = ptr::null_mut();
		filter_graph.setup_filter(
			filter_sink,
			CString::new("buffersink").expect(CSTRING_ERROR_MSG),
			CString::new("thumb_buffersink").expect(CSTRING_ERROR_MSG),
			None,
			"Failed to create filter sink",
		)?;
		let filter_sink_ctx = unsafe { filter_sink.as_mut() }
			.as_deref()
			.and_then(|ptr| unsafe { ptr.as_mut() })
			.ok_or(FFmpegError::NullError)?;

		let mut yadif_filter = ptr::null_mut();
		if interlaced_frame {
			filter_graph.setup_filter(
				&mut yadif_filter,
				CString::new("yadif").expect(CSTRING_ERROR_MSG),
				CString::new("thumb_deint").expect(CSTRING_ERROR_MSG),
				Some(CString::new("deint=1").expect(CSTRING_ERROR_MSG)),
				"Failed to create de-interlace filter",
			)?;
		}

		let mut scale_filter = ptr::null_mut();
		filter_graph.setup_filter(
			&mut scale_filter,
			CString::new("scale").expect(CSTRING_ERROR_MSG),
			CString::new("thumb_scale").expect(CSTRING_ERROR_MSG),
			Some(CString::new(thumb_scale_filter_args(
				size,
				codec_ctx,
				aspect_ratio,
				maintain_aspect_ratio,
			)?)?),
			"Failed to create scale filter",
		)?;

		let mut format_filter = ptr::null_mut();
		filter_graph.setup_filter(
			&mut format_filter,
			CString::new("format").expect(CSTRING_ERROR_MSG),
			CString::new("thumb_format").expect(CSTRING_ERROR_MSG),
			Some(CString::new("pix_fmts=rgb24").expect(CSTRING_ERROR_MSG)),
			"Failed to create format filter",
		)?;

		let mut rotate_filter = ptr::null_mut();
		if rotation_angle < -135.0 {
			filter_graph.setup_filter(
				&mut rotate_filter,
				CString::new("rotate").expect(CSTRING_ERROR_MSG),
				CString::new("thumb_rotate").expect(CSTRING_ERROR_MSG),
				Some(CString::new("PI").expect(CSTRING_ERROR_MSG)),
				"Failed to create rotate filter",
			)?;
		} else if rotation_angle > 45.0 && rotation_angle < 135.0 {
			filter_graph.setup_filter(
				&mut rotate_filter,
				CString::new("transpose").expect(CSTRING_ERROR_MSG),
				CString::new("thumb_transpose").expect(CSTRING_ERROR_MSG),
				Some(CString::new("2").expect(CSTRING_ERROR_MSG)),
				"Failed to create transpose filter",
			)?;
		} else if rotation_angle < -45.0 && rotation_angle > -135.0 {
			filter_graph.setup_filter(
				&mut rotate_filter,
				CString::new("transpose").expect(CSTRING_ERROR_MSG),
				CString::new("thumb_transpose").expect(CSTRING_ERROR_MSG),
				Some(CString::new("1").expect(CSTRING_ERROR_MSG)),
				"Failed to create transpose filter",
			)?;
		}

		Self::link(
			if rotate_filter.is_null() {
				format_filter
			} else {
				rotate_filter
			},
			0,
			filter_sink_ctx,
			0,
			"Failed to link final filter",
		)?;

		if !rotate_filter.is_null() {
			Self::link(
				format_filter,
				0,
				rotate_filter,
				0,
				"Failed to link format filter",
			)?;
		}

		Self::link(
			scale_filter,
			0,
			format_filter,
			0,
			"Failed to link scale filter",
		)?;

		if !yadif_filter.is_null() {
			Self::link(
				yadif_filter,
				0,
				scale_filter,
				0,
				"Failed to link yadif filter",
			)?;
		}

		Self::link(
			filter_source_ctx,
			0,
			if yadif_filter.is_null() {
				scale_filter
			} else {
				yadif_filter
			},
			0,
			"Failed to link source filter",
		)?;

		filter_graph.config()?;

		Ok((filter_graph, filter_source_ctx, filter_sink_ctx))
	}

	pub(crate) fn as_mut(&mut self) -> &mut AVFilterGraph {
		unsafe { self.0.as_mut() }.expect("initialized on struct creation")
	}

	fn setup_filter(
		&mut self,
		filter_ctx: *mut *mut AVFilterContext,
		filter_name: CString,
		filter_setup_name: CString,
		args: Option<CString>,
		error_message: &str,
	) -> Result<(), Error> {
		check_error(
			unsafe {
				avfilter_graph_create_filter(
					filter_ctx,
					avfilter_get_by_name(filter_name.as_ptr()),
					filter_setup_name.as_ptr(),
					args.map(|cstring| cstring.as_ptr()).unwrap_or(ptr::null()),
					ptr::null_mut(),
					self.as_mut(),
				)
			},
			error_message,
		)
	}

	fn config(&mut self) -> Result<&mut Self, Error> {
		check_error(
			unsafe { avfilter_graph_config(self.as_mut(), ptr::null_mut()) },
			"Failed to configure filter graph",
		)?;

		Ok(self)
	}
}

impl Drop for FFmpegFilterGraph {
	fn drop(&mut self) {
		if !self.0.is_null() {
			unsafe { avfilter_graph_free(&mut self.0) };
			self.0 = ptr::null_mut();
		}
	}
}

fn thumb_scale_filter_args(
	size: Option<ThumbnailSize>,
	codec_ctx: &FFmpegCodecContext,
	aspect_ratio: &AVRational,
	maintain_aspect_ratio: bool,
) -> Result<String, Error> {
	let (mut width, mut height) = match size {
		Some(ThumbnailSize::Dimensions { width, height }) => (width as i32, height as i32),
		Some(ThumbnailSize::Size(width)) => (width as i32, -1),
		None => return Ok("w=0:h=0".to_string()),
	};

	if width <= 0 {
		width = -1;
	}

	if height <= 0 {
		height = -1;
	}

	let mut scale = String::new();

	if width != -1 && height != -1 {
		scale.push_str(&format!("w={}:h={}", width, height));
		if maintain_aspect_ratio {
			scale.push_str(":force_original_aspect_ratio=decrease");
		}
	} else if !maintain_aspect_ratio {
		let size = if width == -1 { height } else { width };
		scale.push_str(&format!("w={}:h={}", size, size));
	} else {
		let size = if height == -1 { width } else { height };
		width = codec_ctx.as_ref().width;
		height = codec_ctx.as_ref().height;

		// if the pixel aspect ratio is defined and is not 1, we have an anamorphic stream
		if aspect_ratio.num != 0 && aspect_ratio.num != aspect_ratio.den {
			width = width * aspect_ratio.num / aspect_ratio.den;

			if size != 0 {
				if height > width {
					width = width * size / height;
					height = size;
				} else {
					height = height * size / width;
					width = size;
				}
			}

			scale.push_str(&format!("w={}:h={}", width, height));
		} else if height > width {
			scale.push_str(&format!("w=-1:h={}", if size == 0 { height } else { size }));
		} else {
			scale.push_str(&format!("h=-1:w={}", if size == 0 { width } else { size }));
		}
	}

	Ok(scale)
}
