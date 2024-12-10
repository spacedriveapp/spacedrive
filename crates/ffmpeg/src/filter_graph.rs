use std::{
	ffi::{CStr, CString},
	ptr,
};

use crate::{
	codec_ctx::FFmpegCodecContext, error::FFmpegError, frame_decoder::ThumbnailSize,
	utils::check_error, Error,
};
use ffmpeg_sys_next::{
	avfilter_get_by_name, avfilter_graph_alloc, avfilter_graph_config,
	avfilter_graph_create_filter, avfilter_graph_free, avfilter_link, AVFilterContext,
	AVFilterGraph, AVRational,
};

pub struct FFmpegFilterGraph(*mut AVFilterGraph);

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
		src_pad: u32,
		dst: *mut AVFilterContext,
		dst_pad: u32,
		error: &str,
	) -> Result<(), Error> {
		check_error(unsafe { avfilter_link(src, src_pad, dst, dst_pad) }, error)?;
		Ok(())
	}

	pub(crate) fn thumbnail_graph(
		size: Option<ThumbnailSize>,
		time_base: &AVRational,
		codec_ctx: &FFmpegCodecContext,
		interlaced_frame: bool,
		pixel_aspect_ratio: AVRational,
		maintain_aspect_ratio: bool,
	) -> Result<(Self, &'a mut AVFilterContext, &'a mut AVFilterContext), Error> {
		let mut filter_graph = Self::new()?;

		let args = format!(
			"video_size={}x{}:pix_fmt={}:time_base={}/{}:pixel_aspect={}/{}",
			codec_ctx.as_ref().width,
			codec_ctx.as_ref().height,
			// AVPixelFormat is an i32 enum, so it's safe to cast it to i32
			codec_ctx.as_ref().pix_fmt as i32,
			time_base.num,
			time_base.den,
			codec_ctx.as_ref().sample_aspect_ratio.num,
			i32::max(codec_ctx.as_ref().sample_aspect_ratio.den, 1)
		);

		let mut filter_source = ptr::null_mut();
		filter_graph.setup_filter(
			&mut filter_source,
			c"buffer",
			c"thumb_buffer",
			Some(CString::new(args)?.as_c_str()),
			"Failed to create filter source",
		)?;
		let filter_source_ctx = unsafe { filter_source.as_mut() }.ok_or(FFmpegError::NullError)?;

		let mut filter_sink = ptr::null_mut();
		filter_graph.setup_filter(
			&mut filter_sink,
			c"buffersink",
			c"thumb_buffersink",
			None,
			"Failed to create filter sink",
		)?;
		let filter_sink_ctx = unsafe { filter_sink.as_mut() }.ok_or(FFmpegError::NullError)?;

		let mut yadif_filter = ptr::null_mut();
		if interlaced_frame {
			filter_graph.setup_filter(
				&mut yadif_filter,
				c"yadif",
				c"thumb_deint",
				Some(c"deint=1"),
				"Failed to create de-interlace filter",
			)?;
		}

		let mut scale_filter = ptr::null_mut();
		filter_graph.setup_filter(
			&mut scale_filter,
			c"scale",
			c"thumb_scale",
			Some(
				CString::new(thumb_scale_filter_args(
					size,
					codec_ctx,
					pixel_aspect_ratio,
					maintain_aspect_ratio,
				))?
				.as_c_str(),
			),
			"Failed to create scale filter",
		)?;

		let mut format_filter = ptr::null_mut();
		filter_graph.setup_filter(
			&mut format_filter,
			c"format",
			c"thumb_format",
			Some(c"pix_fmts=rgb24"),
			"Failed to create format filter",
		)?;

		Self::link(
			format_filter,
			0,
			filter_sink_ctx,
			0,
			"Failed to link final filter",
		)?;

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
		filter_name: &CStr,
		filter_setup_name: &CStr,
		args: Option<&CStr>,
		error_message: &str,
	) -> Result<(), Error> {
		check_error(
			unsafe {
				avfilter_graph_create_filter(
					filter_ctx,
					avfilter_get_by_name(filter_name.as_ptr()),
					filter_setup_name.as_ptr(),
					args.map_or(ptr::null(), CStr::as_ptr),
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
	pixel_aspect_ratio: AVRational,
	maintain_aspect_ratio: bool,
) -> String {
	let (width, height) = match size {
		Some(ThumbnailSize::Dimensions { width, height }) => (width, Some(height)),
		Some(ThumbnailSize::Scale(width)) => (width, None),
		None => return "w=0:h=0".to_string(),
	};

	let mut scale = String::new();

	if let Some(height) = height {
		scale.push_str(&format!("w={width}:h={height}"));
		if maintain_aspect_ratio {
			scale.push_str(":force_original_aspect_ratio=decrease");
		}
	} else if !maintain_aspect_ratio {
		scale.push_str(&format!("w={width}:h={width}"));
	} else {
		let size = width;
		let mut width = codec_ctx.as_ref().width.unsigned_abs();
		let mut height = codec_ctx.as_ref().height.unsigned_abs();

		// if the pixel aspect ratio is defined and is not 1, we have an anamorphic stream
		if pixel_aspect_ratio.num != 0 && pixel_aspect_ratio.num != pixel_aspect_ratio.den {
			match std::panic::catch_unwind(|| {
				width
					.checked_mul(pixel_aspect_ratio.num.unsigned_abs())
					.and_then(|v| v.checked_div(pixel_aspect_ratio.den.unsigned_abs()))
			}) {
				Ok(Some(w)) => width = w,
				Ok(None) | Err(_) => {
					eprintln!("Warning: Failed to calculate width with pixel aspect ratio");
					// Keep the original width as fallback
				}
			};
			if size != 0 {
				if height > width {
					width = (width * size) / height;
					height = size;
				} else {
					height = (height * size) / width;
					width = size;
				}
			}

			scale.push_str(&format!("w={width}:h={height}"));
		} else if height > width {
			scale.push_str(&format!("w=-1:h={}", if size == 0 { height } else { size }));
		} else {
			scale.push_str(&format!("h=-1:w={}", if size == 0 { width } else { size }));
		}
	}

	scale
}
