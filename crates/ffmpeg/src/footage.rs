//! # Footage processing
//!
//! Probing, scene scoring, single frame extraction and transcoding, run on
//! the `FFmpeg` libraries this crate links. The desktop app ships those
//! libraries and no `ffmpeg` executable, so footage work that called a
//! binary on the PATH failed on a machine without a system `FFmpeg`.
//!
//! Each step follows what the `ffmpeg` command line does with the same
//! options, so a recipe written as command line flags keeps its output:
//! input timestamps start at zero, video is rotated by its display matrix
//! before the caller's filters, and a packet the decoder rejects is skipped
//! rather than failing the file.
//!
//! ## Example
//! ```rust,no_run
//! use std::{path::Path, sync::atomic::AtomicBool};
//!
//! let cancel = AtomicBool::new(false);
//! let scores = sd_ffmpeg::footage::scene_scores(
//!     Path::new("/footage/a.mov"),
//!     "scale=-2:270,select='gte(scene,0)'",
//!     &cancel,
//! )?;
//! # Ok::<(), sd_ffmpeg::Error>(())
//! ```

#![allow(
	// Every cast here moves a value between FFmpeg's C integer types and
	// the Rust ones it was read from, within the ranges FFmpeg defines.
	clippy::cast_possible_truncation,
	clippy::cast_possible_wrap,
	clippy::cast_precision_loss,
	clippy::cast_sign_loss,
	clippy::borrow_as_ptr,
	// FFmpeg allocates side data with av_malloc, aligned for any type.
	clippy::cast_ptr_alignment
)]

use std::{
	cell::Cell,
	ffi::{c_char, c_int, c_void, CStr, CString},
	fmt::Write,
	fs,
	path::Path,
	ptr,
	sync::atomic::{AtomicBool, AtomicU64, Ordering},
	time::{Duration, Instant},
};

use ffmpeg_sys_next as ff;
use serde_json::{json, Map, Value};

use crate::{
	error::{Error, FFmpegError},
	utils::from_path,
};

/// The scene score of one frame that came out of a scoring filter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneScore {
	/// Presentation time in seconds, counted from the start of the file.
	pub time: f64,
	/// The `lavfi.scene_score` the `select` filter set, 0 to 1.
	pub score: f32,
}

/// A video encode: the filters, the encoder and its options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoEncode {
	/// Filter chain for the first video stream, as `-vf` takes it.
	pub filter: String,
	pub codec: String,
	/// Encoder options such as `preset` or `crf`.
	pub options: Vec<(String, String)>,
	/// Force a keyframe whenever this many seconds have passed since the
	/// last forced one, as `-force_key_frames expr:gte(t,n_forced*N)`.
	pub keyframe_seconds: Option<u32>,
}

/// An audio encode of the first audio stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioEncode {
	pub codec: String,
	pub bit_rate: Option<i64>,
	/// Output channel count, as `-ac`.
	pub channels: i32,
	/// Output sample rate, as `-ar`. `None` keeps the input rate when the
	/// encoder supports it.
	pub sample_rate: Option<i32>,
}

/// One output file from one input: its container, streams and tags.
///
/// Input metadata is never copied, as with `-map_metadata -1`, and subtitle
/// and data streams are dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcode {
	/// Muxer name, as `-f`.
	pub format: String,
	pub video: Option<VideoEncode>,
	/// Encoded when the input has an audio stream, skipped when it has none.
	pub audio: Option<AudioEncode>,
	/// Container tags, such as `timecode`.
	pub metadata: Vec<(String, String)>,
	/// Muxer options, such as `movflags`.
	pub muxer_options: Vec<(String, String)>,
}

/// Streams, rates, durations and timecode tags of a file, in the shape
/// `ffprobe -of json` prints them.
///
/// Every stream has `codec_type`. Video streams add `width`, `height`,
/// `avg_frame_rate` and `r_frame_rate`, audio streams `sample_rate` and
/// `channels`, and any stream `nb_frames` when the container counts them.
/// A `timecode` tag sits under `tags` of the stream or the format, and the
/// format has `duration` when the container knows it. Gives up with
/// [`Error::TimedOut`] after `timeout`.
pub fn probe(path: &Path, timeout: Duration) -> Result<Value, Error> {
	let input = Input::open(path, Interrupt::after(timeout))?;
	let streams: Vec<Value> = input
		.streams()
		.iter()
		.map(|&stream| {
			// SAFETY: streams and their parameters live as long as `input`.
			let (stream, parameters) = unsafe { (&*stream, &*(*stream).codecpar) };
			let mut entry = Map::new();
			// SAFETY: returns a static string or null.
			if let Some(kind) = unsafe { text(ff::av_get_media_type_string(parameters.codec_type)) }
			{
				entry.insert("codec_type".into(), kind.into());
			}
			match parameters.codec_type {
				ff::AVMediaType::AVMEDIA_TYPE_VIDEO => {
					entry.insert("width".into(), parameters.width.into());
					entry.insert("height".into(), parameters.height.into());
					entry.insert("avg_frame_rate".into(), ratio(stream.avg_frame_rate).into());
					entry.insert("r_frame_rate".into(), ratio(stream.r_frame_rate).into());
				}
				ff::AVMediaType::AVMEDIA_TYPE_AUDIO => {
					entry.insert(
						"sample_rate".into(),
						parameters.sample_rate.to_string().into(),
					);
					entry.insert("channels".into(), parameters.ch_layout.nb_channels.into());
				}
				_ => {}
			}
			if stream.nb_frames > 0 {
				entry.insert("nb_frames".into(), stream.nb_frames.to_string().into());
			}
			if let Some(timecode) = tag(stream.metadata, c"timecode") {
				entry.insert("tags".into(), json!({ "timecode": timecode }));
			}
			Value::Object(entry)
		})
		.collect();

	let mut format = Map::new();
	let context = input.context();
	if context.duration != ff::AV_NOPTS_VALUE {
		let seconds = context.duration as f64 / f64::from(ff::AV_TIME_BASE);
		format.insert("duration".into(), format!("{seconds:.6}").into());
	}
	if let Some(timecode) = tag(context.metadata, c"timecode") {
		format.insert("tags".into(), json!({ "timecode": timecode }));
	}
	Ok(json!({ "streams": streams, "format": format }))
}

/// Run every frame of the first video stream through `filter` and return
/// the scene score of each frame it outputs, until done or `cancel` is set.
///
/// `filter` must end in a `select` filter that evaluates `scene`, such as
/// `select='gte(scene,0)'`, which is what sets the score. Frames without
/// one are left out.
pub fn scene_scores(
	path: &Path,
	filter: &str,
	cancel: &AtomicBool,
) -> Result<Vec<SceneScore>, Error> {
	let input = Input::open(path, Interrupt::on(cancel))?;
	let stream = input
		.first(ff::AVMediaType::AVMEDIA_TYPE_VIDEO)
		.ok_or_else(|| Error::Media("the file has no video stream".into()))?;
	let mut lane = Lane::video(&input, stream, filter, "auto")?;
	let time_base = lane.graph.output_time_base();
	let mut scores = Vec::new();
	let mut each = |frame: &mut ff::AVFrame| {
		let Some(score) = tag(frame.metadata, c"lavfi.scene_score") else {
			return Ok(());
		};
		let score = score.parse::<f32>().unwrap_or(0.0);
		let time = if frame.pts == ff::AV_NOPTS_VALUE {
			scores
				.last()
				.map_or(0.0, |previous: &SceneScore| previous.time)
		} else {
			frame.pts as f64 * q2d(time_base)
		};
		scores.push(SceneScore {
			time,
			score: if score.is_finite() { score } else { 0.0 },
		});
		Ok(())
	};
	let packet = Packet::new()?;
	while input.read(&packet)? {
		if packet.stream_index() == lane.index {
			lane.feed(packet.0, &mut each)?;
		}
		packet.unref();
	}
	lane.feed(ptr::null_mut(), &mut each)?;
	Ok(scores)
}

/// Write the first frame at or after `seconds` of the first video stream as
/// a JPEG, after `filter`, as `-ss seconds -i input -vf filter -frames:v 1
/// -c:v mjpeg -q:v quality` does.
///
/// `filter` must leave the frame in a pixel format the MJPEG encoder takes,
/// such as `yuvj420p`. Gives up with [`Error::TimedOut`] after `timeout`.
pub fn write_jpeg(
	input: &Path,
	seconds: f64,
	filter: &str,
	quality: i32,
	output: &Path,
	timeout: Duration,
) -> Result<(), Error> {
	let (frame, time_base) = first_frame(input, Some(seconds), filter, "auto", timeout)?
		.ok_or_else(|| Error::Media(format!("no frame at {seconds:.3}s, the video is shorter")))?;
	let codec = find_encoder("mjpeg")?;
	let encoder = Codec::alloc(codec)?;
	// SAFETY: the context was just allocated and the frame holds a picture.
	unsafe {
		let context = &mut *encoder.0;
		let picture = &mut *frame.0;
		context.width = picture.width;
		context.height = picture.height;
		context.pix_fmt = pixel_format(picture.format);
		context.sample_aspect_ratio = picture.sample_aspect_ratio;
		context.time_base = time_base;
		context.flags |= ff::AV_CODEC_FLAG_QSCALE as c_int;
		context.global_quality = ff::FF_QP2LAMBDA * quality;
		picture.quality = context.global_quality;
		picture.pict_type = ff::AVPictureType::AV_PICTURE_TYPE_NONE;
	}
	encoder.open(codec, &[])?;
	let mut jpeg = Vec::new();
	let packet = Packet::new()?;
	for picture in [frame.0, ptr::null_mut()] {
		// SAFETY: the encoder is open; a null frame flushes it.
		check(
			unsafe { ff::avcodec_send_frame(encoder.0, picture) },
			"encode the JPEG",
		)?;
		while encoder.receive(&packet)? {
			jpeg.extend_from_slice(packet.data());
			packet.unref();
		}
	}
	if jpeg.is_empty() {
		return Err(Error::Media("the JPEG encoder wrote nothing".into()));
	}
	fs::write(output, jpeg).map_err(|error| Error::Media(format!("write the JPEG: {error}")))
}

/// A decoded picture, 8-bit RGBA, rows packed without padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
	pub width: u32,
	pub height: u32,
	pub rgba: Vec<u8>,
}

/// The first frame `filter` outputs from the first video stream, as RGBA.
///
/// The decoder runs on one thread: callers make thumbnails of many files at
/// once, so parallel work belongs across files rather than inside one
/// decode. Returns `None` when the filter outputs no frame, and gives up
/// with [`Error::TimedOut`] after `timeout`.
pub fn read_rgba(input: &Path, filter: &str, timeout: Duration) -> Result<Option<Picture>, Error> {
	let chain = if filter.is_empty() {
		"format=rgba".to_string()
	} else {
		format!("{filter},format=rgba")
	};
	let Some((frame, _)) = first_frame(input, None, &chain, "1", timeout)? else {
		return Ok(None);
	};
	// SAFETY: the frame holds an RGBA picture whose first plane has
	// `height` rows of `linesize[0]` bytes.
	let picture = unsafe { &*frame.0 };
	let (width, height) = (picture.width as usize, picture.height as usize);
	// Signed: a flipped picture walks its rows backwards from `data[0]`.
	let stride = picture.linesize[0] as isize;
	let mut rgba = Vec::with_capacity(width * height * 4);
	for row in 0..height {
		// SAFETY: as above, each row starts `stride` bytes after the last,
		// within the frame's buffer whichever way the rows run.
		let line = unsafe {
			std::slice::from_raw_parts(picture.data[0].offset(row as isize * stride), width * 4)
		};
		rgba.extend_from_slice(line);
	}
	Ok(Some(Picture {
		width: width as u32,
		height: height as u32,
		rgba,
	}))
}

/// The first frame `filter` outputs, from `seconds` when set, with the time
/// base it is timed in.
fn first_frame(
	input: &Path,
	seconds: Option<f64>,
	filter: &str,
	threads: &str,
	timeout: Duration,
) -> Result<Option<(Frame, ff::AVRational)>, Error> {
	let source = Input::open(input, Interrupt::after(timeout))?;
	let stream = source
		.first(ff::AVMediaType::AVMEDIA_TYPE_VIDEO)
		.ok_or_else(|| Error::Media("the file has no video stream".into()))?;
	let mut lane = Lane::video(&source, stream, filter, threads)?;
	if let Some(seconds) = seconds {
		// As `-ss` does: seek to the keyframe before the time, then drop
		// every frame before it, measuring from the start of the file.
		let target = (seconds * f64::from(ff::AV_TIME_BASE)).round() as i64 + source.start_time();
		// SAFETY: the context is open. A failed seek leaves it at the
		// start, from where decoding reaches the time anyway.
		unsafe { ff::avformat_seek_file(source.context, -1, i64::MIN, target, target, 0) };
		lane.shift(-target);
		lane.drop_before_zero = true;
	}

	let frame = Frame::new()?;
	let captured = Cell::new(false);
	let mut each = |filtered: &mut ff::AVFrame| {
		if !captured.get() {
			// SAFETY: both frames are allocated; the reference is released
			// when `frame` drops.
			check(
				unsafe { ff::av_frame_ref(frame.0, filtered) },
				"keep the frame",
			)?;
			captured.set(true);
		}
		Ok(())
	};
	let packet = Packet::new()?;
	while !captured.get() && source.read(&packet)? {
		if packet.stream_index() == lane.index {
			lane.feed(packet.0, &mut each)?;
		}
		packet.unref();
	}
	if !captured.get() {
		lane.feed(ptr::null_mut(), &mut each)?;
	}
	let time_base = lane.graph.output_time_base();
	Ok(captured.get().then_some((frame, time_base)))
}

/// Encode `input` to `output` as `recipe` says, until done or `cancel` is
/// set.
///
/// `written_us` follows the time written so far in microseconds, as
/// `-progress` reports `out_time_us`. On an error or a cancel the output is
/// left partial for the caller to remove.
pub fn transcode(
	input: &Path,
	output: &Path,
	recipe: &Transcode,
	cancel: &AtomicBool,
	written_us: &AtomicU64,
) -> Result<(), Error> {
	let source = Input::open(input, Interrupt::on(cancel))?;
	let mut muxer = Output::new(output, &recipe.format)?;
	let mut lanes = Vec::new();
	if let Some(video) = &recipe.video {
		let stream = source
			.first(ff::AVMediaType::AVMEDIA_TYPE_VIDEO)
			.ok_or_else(|| Error::Media("the file has no video stream".into()))?;
		lanes.push(Encoding::video(&source, stream, video, &mut muxer)?);
	}
	if let Some(audio) = &recipe.audio {
		if let Some(stream) = source.first(ff::AVMediaType::AVMEDIA_TYPE_AUDIO) {
			lanes.push(Encoding::audio(&source, stream, audio, &mut muxer)?);
		}
	}
	if lanes.is_empty() {
		return Err(Error::Media("the file has no stream to encode".into()));
	}
	muxer.write_header(&recipe.metadata, &recipe.muxer_options)?;

	let packet = Packet::new()?;
	let written = Packet::new()?;
	while source.read(&packet)? {
		let index = packet.stream_index();
		if let Some(position) = lanes.iter().position(|lane| lane.lane.index == index) {
			let progress = (position == 0).then_some(written_us);
			lanes[position].feed(packet.0, &written, &muxer, progress)?;
		}
		packet.unref();
	}
	for (position, lane) in lanes.iter_mut().enumerate() {
		let progress = (position == 0).then_some(written_us);
		lane.feed(ptr::null_mut(), &written, &muxer, progress)?;
		lane.flush(&written, &muxer)?;
	}
	muxer.finish()
}

/// Encode what a source filter such as `testsrc2` or `sine` produces to a
/// file, which makes footage for tests without shipping any.
///
/// The stream is encoded with `codec` and `options` in the format and at
/// the time base the filter gives it, and muxed as `format` with `metadata`
/// tags.
pub fn render(
	source: &str,
	codec: &str,
	options: &[(String, String)],
	format: &str,
	metadata: &[(String, String)],
	output: &Path,
) -> Result<(), Error> {
	let first = source.split([',', '=']).next().unwrap_or_default();
	let first = CString::new(first)?;
	// SAFETY: looks up a static filter description; a source filter has
	// one output pad whose type is the stream it makes.
	let video = unsafe {
		let filter = ff::avfilter_get_by_name(first.as_ptr());
		if filter.is_null() {
			return Err(FFmpegError::FilterNotFound.into());
		}
		ff::avfilter_pad_get_type((*filter).outputs, 0) == ff::AVMediaType::AVMEDIA_TYPE_VIDEO
	};
	let graph = Graph::build(None, source, video)?;
	let mut muxer = Output::new(output, format)?;
	let codec = find_encoder(codec)?;
	let encoder = Codec::alloc(codec)?;
	if video {
		graph.configure_video_encoder(&encoder, None, muxer.global_header());
	} else {
		graph.configure_audio_encoder(&encoder, muxer.global_header())?;
	}
	encoder.open(codec, options)?;
	let stream = muxer.add_stream(&encoder, graph.output_frame_rate())?;
	muxer.write_header(metadata, &[])?;

	let frame = Frame::new()?;
	let packet = Packet::new()?;
	let time_base = graph.output_time_base();
	loop {
		let more = graph.pull(&frame)?;
		if more {
			// SAFETY: the frame holds media from the sink.
			unsafe { (*frame.0).pict_type = ff::AVPictureType::AV_PICTURE_TYPE_NONE };
		}
		let media = if more { frame.0 } else { ptr::null_mut() };
		// SAFETY: the encoder is open; a null frame flushes it.
		check(
			unsafe { ff::avcodec_send_frame(encoder.0, media) },
			"encode the stream",
		)?;
		frame.unref();
		while encoder.receive(&packet)? {
			muxer.write(&packet, stream, time_base)?;
		}
		if !more {
			break;
		}
	}
	muxer.finish()
}

/// Rotation and flips for a display matrix, as the command line's
/// autorotation puts them ahead of the caller's filters.
fn autorotation(stream: &ff::AVStream) -> Option<String> {
	// SAFETY: side data, when present, is a 3x3 matrix of i32 that lives as
	// long as the stream.
	let matrix = unsafe {
		ff::av_stream_get_side_data(
			stream,
			ff::AVPacketSideDataType::AV_PKT_DATA_DISPLAYMATRIX,
			ptr::null_mut(),
		)
	}
	.cast::<i32>();
	if matrix.is_null() {
		return None;
	}
	// SAFETY: checked non-null above; the matrix has nine entries.
	let (theta, cells) = unsafe {
		(
			-ff::av_display_rotation_get(matrix).round(),
			std::slice::from_raw_parts(matrix, 9),
		)
	};
	let theta = 360.0f64.mul_add(-(theta / 360.0 + 0.9 / 360.0).floor(), theta);
	let filters: Vec<String> = if (theta - 90.0).abs() < 1.0 {
		vec![if cells[3] > 0 {
			"transpose=cclock_flip".into()
		} else {
			"transpose=clock".into()
		}]
	} else if (theta - 180.0).abs() < 1.0 {
		[(cells[0] < 0, "hflip"), (cells[4] < 0, "vflip")]
			.into_iter()
			.filter(|&(apply, _)| apply)
			.map(|(_, flip)| flip.to_string())
			.collect()
	} else if (theta - 270.0).abs() < 1.0 {
		vec![if cells[3] < 0 {
			"transpose=clock_flip".into()
		} else {
			"transpose=cclock".into()
		}]
	} else if theta.abs() > 1.0 {
		vec![format!("rotate={theta:.6}*PI/180")]
	} else if cells[4] < 0 {
		vec!["vflip".into()]
	} else {
		Vec::new()
	};
	(!filters.is_empty()).then(|| filters.join(","))
}

type EachFrame<'a> = dyn FnMut(&mut ff::AVFrame) -> Result<(), Error> + 'a;

/// One input stream decoded and run through a filter graph.
struct Lane {
	index: c_int,
	time_base: ff::AVRational,
	decoder: Codec,
	graph: Graph,
	/// Added to every packet timestamp, in the stream's time base, so the
	/// file starts at zero as on the command line.
	offset: i64,
	/// Leave out decoded frames before zero, as an input seek does.
	drop_before_zero: bool,
	decoded: Frame,
	filtered: Frame,
}

impl Lane {
	fn video(
		input: &Input<'_>,
		stream: *mut ff::AVStream,
		filter: &str,
		threads: &str,
	) -> Result<Self, Error> {
		// SAFETY: the stream belongs to the open input.
		let stream_ref = unsafe { &*stream };
		let decoder = Codec::decoder(stream_ref)?;
		// SAFETY: as above; the frame argument may be null.
		unsafe {
			(*decoder.0).framerate =
				ff::av_guess_frame_rate(input.context, stream, ptr::null_mut());
		}
		decoder.open_decoder(threads)?;
		// SAFETY: the decoder is open, so its picture parameters are set.
		let context = unsafe { &*decoder.0 };
		if context.pix_fmt == ff::AVPixelFormat::AV_PIX_FMT_NONE {
			return Err(Error::Media("the video stream has no pixel format".into()));
		}
		let aspect = context.sample_aspect_ratio;
		let rate = context.framerate;
		let mut args = format!(
			"video_size={}x{}:pix_fmt={}:time_base={}:pixel_aspect={}/{}",
			context.width,
			context.height,
			context.pix_fmt as c_int,
			ratio(stream_ref.time_base),
			aspect.num,
			aspect.den.max(1),
		);
		if rate.num > 0 && rate.den > 0 {
			let _ = write!(args, ":frame_rate={}", ratio(rate));
		}
		let chain = match (autorotation(stream_ref), filter.is_empty()) {
			(Some(rotate), true) => rotate,
			(Some(rotate), false) => format!("{rotate},{filter}"),
			(None, true) => "null".into(),
			(None, false) => filter.into(),
		};
		let graph = Graph::build(Some(("buffer", args)), &chain, true)?;
		Self::new(input, stream_ref, decoder, graph)
	}

	fn audio(input: &Input<'_>, stream: *mut ff::AVStream, filter: &str) -> Result<Self, Error> {
		// SAFETY: the stream belongs to the open input.
		let stream_ref = unsafe { &*stream };
		let decoder = Codec::decoder(stream_ref)?;
		decoder.open_decoder("auto")?;
		// SAFETY: the decoder is open, so its sample parameters are set.
		let context = unsafe { &*decoder.0 };
		// SAFETY: returns a static name or null.
		let sample_format = unsafe { text(ff::av_get_sample_fmt_name(context.sample_fmt)) }
			.ok_or_else(|| Error::Media("the audio stream has no sample format".into()))?;
		let args = format!(
			"time_base={}:sample_rate={}:sample_fmt={sample_format}:channel_layout={}",
			ratio(stream_ref.time_base),
			context.sample_rate,
			layout_name(&context.ch_layout)?,
		);
		let graph = Graph::build(Some(("abuffer", args)), filter, false)?;
		Self::new(input, stream_ref, decoder, graph)
	}

	fn new(
		input: &Input<'_>,
		stream: &ff::AVStream,
		decoder: Codec,
		graph: Graph,
	) -> Result<Self, Error> {
		let mut lane = Self {
			index: stream.index,
			time_base: stream.time_base,
			decoder,
			graph,
			offset: 0,
			drop_before_zero: false,
			decoded: Frame::new()?,
			filtered: Frame::new()?,
		};
		lane.shift(-input.start_time());
		Ok(lane)
	}

	/// Set the packet offset from a time in `AV_TIME_BASE` units.
	fn shift(&mut self, offset: i64) {
		// SAFETY: pure arithmetic.
		self.offset = unsafe { ff::av_rescale_q(offset, time_base_q(), self.time_base) };
	}

	/// Decode one packet, or flush the decoder and the graph when `packet`
	/// is null, and hand every filtered frame to `each`.
	fn feed(&mut self, packet: *mut ff::AVPacket, each: &mut EachFrame<'_>) -> Result<(), Error> {
		if !packet.is_null() {
			// SAFETY: a packet read from this lane's stream.
			let packet = unsafe { &mut *packet };
			if packet.pts != ff::AV_NOPTS_VALUE {
				packet.pts += self.offset;
			}
			if packet.dts != ff::AV_NOPTS_VALUE {
				packet.dts += self.offset;
			}
		}
		// A packet the decoder rejects is skipped, as the command line
		// skips it, and the frames around it still decode.
		// SAFETY: the decoder is open; a null packet starts draining.
		unsafe { ff::avcodec_send_packet(self.decoder.0, packet) };
		loop {
			// SAFETY: the decoder is open and the frame allocated.
			if unsafe { ff::avcodec_receive_frame(self.decoder.0, self.decoded.0) } < 0 {
				break;
			}
			// SAFETY: the decoder just filled the frame.
			let frame = unsafe { &mut *self.decoded.0 };
			frame.pts = frame.best_effort_timestamp;
			if self.drop_before_zero && frame.pts != ff::AV_NOPTS_VALUE && frame.pts < 0 {
				self.decoded.unref();
				continue;
			}
			self.graph.push(self.decoded.0)?;
			self.decoded.unref();
			self.drain(each)?;
		}
		if packet.is_null() {
			self.graph.push(ptr::null_mut())?;
			self.drain(each)?;
		}
		Ok(())
	}

	fn drain(&self, each: &mut EachFrame<'_>) -> Result<(), Error> {
		while self.graph.pull(&self.filtered)? {
			// SAFETY: the sink just filled the frame.
			let result = each(unsafe { &mut *self.filtered.0 });
			self.filtered.unref();
			result?;
		}
		Ok(())
	}
}

/// A lane whose frames go through an encoder to a stream of the output.
struct Encoding {
	lane: Lane,
	encoder: Codec,
	stream: c_int,
	keyframe_seconds: Option<u32>,
	keyframes_forced: u32,
}

impl Encoding {
	fn video(
		input: &Input<'_>,
		stream: *mut ff::AVStream,
		recipe: &VideoEncode,
		muxer: &mut Output,
	) -> Result<Self, Error> {
		let lane = Lane::video(input, stream, &recipe.filter, "auto")?;
		let codec = find_encoder(&recipe.codec)?;
		let encoder = Codec::alloc(codec)?;
		lane.graph
			.configure_video_encoder(&encoder, Some(&lane.decoder), muxer.global_header());
		encoder.open(codec, &recipe.options)?;
		let stream = muxer.add_stream(&encoder, lane.graph.output_frame_rate())?;
		Ok(Self {
			lane,
			encoder,
			stream,
			keyframe_seconds: recipe.keyframe_seconds,
			keyframes_forced: 0,
		})
	}

	fn audio(
		input: &Input<'_>,
		stream: *mut ff::AVStream,
		recipe: &AudioEncode,
		muxer: &mut Output,
	) -> Result<Self, Error> {
		let codec = find_encoder(&recipe.codec)?;
		// As the command line does: convert to a format and rate the encoder
		// takes, with the requested channel count and rate.
		// SAFETY: the codec is a static description whose lists are
		// terminated, by AV_SAMPLE_FMT_NONE and by 0.
		let mut constraints = Vec::new();
		unsafe {
			let mut cursor = (*codec).sample_fmts;
			let mut names = Vec::new();
			while !cursor.is_null() && *cursor != ff::AVSampleFormat::AV_SAMPLE_FMT_NONE {
				names.extend(text(ff::av_get_sample_fmt_name(*cursor)));
				cursor = cursor.add(1);
			}
			if !names.is_empty() {
				constraints.push(format!("sample_fmts={}", names.join("|")));
			}
			let mut cursor = (*codec).supported_samplerates;
			let mut rates = Vec::new();
			while !cursor.is_null() && *cursor != 0 {
				rates.push((*cursor).to_string());
				cursor = cursor.add(1);
			}
			match recipe.sample_rate {
				Some(rate) => constraints.push(format!("sample_rates={rate}")),
				None if !rates.is_empty() => {
					constraints.push(format!("sample_rates={}", rates.join("|")));
				}
				None => {}
			}
		}
		let layout = ChannelLayout::default_for(recipe.channels);
		constraints.push(format!("channel_layouts={}", layout_name(&layout.0)?));
		let lane = Lane::audio(input, stream, &format!("aformat={}", constraints.join(":")))?;

		let encoder = Codec::alloc(codec)?;
		lane.graph
			.configure_audio_encoder(&encoder, muxer.global_header())?;
		if let Some(bit_rate) = recipe.bit_rate {
			// SAFETY: the context is allocated and not yet open.
			unsafe { (*encoder.0).bit_rate = bit_rate };
		}
		encoder.open(codec, &[])?;
		let sink = lane.graph.sink;
		// SAFETY: the encoder is open and the sink configured.
		unsafe {
			if (*codec).capabilities & ff::AV_CODEC_CAP_VARIABLE_FRAME_SIZE as c_int == 0 {
				ff::av_buffersink_set_frame_size(sink, (*encoder.0).frame_size as u32);
			}
		}
		let stream = muxer.add_stream(&encoder, ff::AVRational { num: 0, den: 1 })?;
		Ok(Self {
			lane,
			encoder,
			stream,
			keyframe_seconds: None,
			keyframes_forced: 0,
		})
	}

	fn feed(
		&mut self,
		packet: *mut ff::AVPacket,
		written: &Packet,
		muxer: &Output,
		progress: Option<&AtomicU64>,
	) -> Result<(), Error> {
		let sink_base = self.lane.graph.output_time_base();
		let (encoder, stream) = (&self.encoder, self.stream);
		// SAFETY: the encoder is open.
		let encoder_base = unsafe { (*encoder.0).time_base };
		let keyframe_seconds = self.keyframe_seconds;
		let forced = &mut self.keyframes_forced;
		self.lane.feed(packet, &mut |frame| {
			frame.pict_type = ff::AVPictureType::AV_PICTURE_TYPE_NONE;
			if frame.pts != ff::AV_NOPTS_VALUE {
				// SAFETY: pure arithmetic.
				frame.pts = unsafe { ff::av_rescale_q(frame.pts, sink_base, encoder_base) };
				let seconds = frame.pts as f64 * q2d(encoder_base);
				if let Some(progress) = progress {
					progress.store((seconds.max(0.0) * 1_000_000.0) as u64, Ordering::Relaxed);
				}
				if let Some(interval) = keyframe_seconds {
					if seconds >= f64::from(*forced) * f64::from(interval) {
						frame.pict_type = ff::AVPictureType::AV_PICTURE_TYPE_I;
						*forced += 1;
					}
				}
			}
			// SAFETY: the encoder is open and the frame holds media.
			check(
				unsafe { ff::avcodec_send_frame(encoder.0, frame) },
				"encode a frame",
			)?;
			while encoder.receive(written)? {
				muxer.write(written, stream, encoder_base)?;
			}
			Ok(())
		})
	}

	fn flush(&self, written: &Packet, muxer: &Output) -> Result<(), Error> {
		// SAFETY: the encoder is open; a null frame flushes it.
		check(
			unsafe { ff::avcodec_send_frame(self.encoder.0, ptr::null()) },
			"flush the encoder",
		)?;
		// SAFETY: as above.
		let time_base = unsafe { (*self.encoder.0).time_base };
		while self.encoder.receive(written)? {
			muxer.write(written, self.stream, time_base)?;
		}
		Ok(())
	}
}

/// What stops reading a file: a cancel flag set by the caller, or a
/// deadline. `FFmpeg` polls it inside blocking reads too, so a stalled file
/// or volume cannot hold a worker forever.
struct Interrupt<'a> {
	cancel: Option<&'a AtomicBool>,
	deadline: Option<Instant>,
}

impl<'a> Interrupt<'a> {
	const fn on(cancel: &'a AtomicBool) -> Self {
		Self {
			cancel: Some(cancel),
			deadline: None,
		}
	}

	fn after(timeout: Duration) -> Self {
		Self {
			cancel: None,
			deadline: Some(Instant::now() + timeout),
		}
	}

	fn reason(&self) -> Option<Error> {
		if self
			.cancel
			.is_some_and(|cancel| cancel.load(Ordering::Relaxed))
		{
			return Some(Error::Cancelled);
		}
		if self
			.deadline
			.is_some_and(|deadline| Instant::now() >= deadline)
		{
			return Some(Error::TimedOut);
		}
		None
	}

	/// Turn an error from an interrupted call into the reason it stopped.
	fn explain(&self, error: Error) -> Error {
		self.reason().unwrap_or(error)
	}
}

unsafe extern "C" fn interrupted(opaque: *mut c_void) -> c_int {
	// SAFETY: the opaque pointer is the boxed §Interrupt§ the input owns,
	// which outlives its format context.
	let interrupt = unsafe { &*opaque.cast::<Interrupt<'_>>() };
	c_int::from(interrupt.reason().is_some())
}

struct Input<'a> {
	context: *mut ff::AVFormatContext,
	interrupt: Box<Interrupt<'a>>,
}

impl<'a> Input<'a> {
	fn open(path: &Path, interrupt: Interrupt<'a>) -> Result<Self, Error> {
		quiet();
		let name = from_path(path)?;
		let interrupt = Box::new(interrupt);
		// SAFETY: allocates an empty context to carry the callback.
		let mut context = unsafe { ff::avformat_alloc_context() };
		if context.is_null() {
			return Err(FFmpegError::ContextAllocation.into());
		}
		// SAFETY: the context is allocated; the box stays at one address
		// for as long as the input holds it.
		unsafe {
			(*context).interrupt_callback = ff::AVIOInterruptCB {
				callback: Some(interrupted),
				opaque: ptr::from_ref::<Interrupt<'_>>(&interrupt).cast_mut().cast(),
			};
		}
		// SAFETY: on failure FFmpeg frees the context it was given.
		check(
			unsafe {
				ff::avformat_open_input(&mut context, name.as_ptr(), ptr::null(), ptr::null_mut())
			},
			"open the file",
		)
		.map_err(|error| interrupt.explain(error))?;
		let input = Self { context, interrupt };
		// SAFETY: the context is open.
		check(
			unsafe { ff::avformat_find_stream_info(input.context, ptr::null_mut()) },
			"read the stream information",
		)
		.map_err(|error| input.interrupt.explain(error))?;
		Ok(input)
	}

	const fn context(&self) -> &ff::AVFormatContext {
		// SAFETY: open for as long as `self`.
		unsafe { &*self.context }
	}

	const fn streams(&self) -> &[*mut ff::AVStream] {
		let context = self.context();
		if context.streams.is_null() {
			return &[];
		}
		// SAFETY: FFmpeg keeps `nb_streams` pointers at `streams`.
		unsafe { std::slice::from_raw_parts(context.streams, context.nb_streams as usize) }
	}

	/// The first stream of a kind, as `0:v:0` or `0:a:0` selects it.
	fn first(&self, kind: ff::AVMediaType) -> Option<*mut ff::AVStream> {
		self.streams()
			.iter()
			.copied()
			// SAFETY: streams and their parameters live as long as `self`.
			.find(|&stream| unsafe { (*(*stream).codecpar).codec_type } == kind)
	}

	/// The container's start time in `AV_TIME_BASE` units, zero when unknown.
	const fn start_time(&self) -> i64 {
		match self.context().start_time {
			ff::AV_NOPTS_VALUE => 0,
			start => start,
		}
	}

	/// Read the next packet; false at the end of the file. Checked between
	/// packets as well, since decoding a packet can take longer than
	/// reading it.
	fn read(&self, packet: &Packet) -> Result<bool, Error> {
		if let Some(reason) = self.interrupt.reason() {
			return Err(reason);
		}
		// SAFETY: the context is open and the packet allocated and empty.
		match unsafe { ff::av_read_frame(self.context, packet.0) } {
			ff::AVERROR_EOF => Ok(false),
			status => check(status, "read the file")
				.map(|_| true)
				.map_err(|error| self.interrupt.explain(error)),
		}
	}
}

impl Drop for Input<'_> {
	fn drop(&mut self) {
		// SAFETY: opened by avformat_open_input; this nulls the pointer.
		unsafe { ff::avformat_close_input(&mut self.context) };
	}
}

struct Output {
	context: *mut ff::AVFormatContext,
	opened: bool,
}

impl Output {
	fn new(path: &Path, format: &str) -> Result<Self, Error> {
		quiet();
		let name = from_path(path)?;
		let format = CString::new(format)?;
		let mut context = ptr::null_mut();
		// SAFETY: a null output format makes FFmpeg look up `format`.
		check(
			unsafe {
				ff::avformat_alloc_output_context2(
					&mut context,
					ptr::null(),
					format.as_ptr(),
					name.as_ptr(),
				)
			},
			"set up the output",
		)?;
		let mut output = Self {
			context,
			opened: false,
		};
		// SAFETY: the context was just allocated with its output format.
		unsafe {
			if (*(*output.context).oformat).flags & ff::AVFMT_NOFILE == 0 {
				check(
					ff::avio_open(
						&mut (*output.context).pb,
						name.as_ptr(),
						ff::AVIO_FLAG_WRITE,
					),
					"create the output file",
				)?;
				output.opened = true;
			}
		}
		Ok(output)
	}

	fn global_header(&self) -> bool {
		// SAFETY: the output format is static and set at allocation.
		unsafe { (*(*self.context).oformat).flags & ff::AVFMT_GLOBALHEADER != 0 }
	}

	fn add_stream(&mut self, encoder: &Codec, frame_rate: ff::AVRational) -> Result<c_int, Error> {
		// SAFETY: the context is allocated and the encoder open.
		unsafe {
			let stream = ff::avformat_new_stream(self.context, ptr::null());
			if stream.is_null() {
				return Err(FFmpegError::ContextAllocation.into());
			}
			check(
				ff::avcodec_parameters_from_context((*stream).codecpar, encoder.0),
				"describe the output stream",
			)?;
			(*stream).time_base = (*encoder.0).time_base;
			if frame_rate.num > 0 && frame_rate.den > 0 {
				(*stream).avg_frame_rate = frame_rate;
			}
			Ok((*stream).index)
		}
	}

	fn write_header(
		&mut self,
		metadata: &[(String, String)],
		options: &[(String, String)],
	) -> Result<(), Error> {
		for (key, value) in metadata {
			let (key, value) = (CString::new(key.as_str())?, CString::new(value.as_str())?);
			// SAFETY: the dictionary belongs to the context.
			check(
				unsafe {
					ff::av_dict_set(
						&mut (*self.context).metadata,
						key.as_ptr(),
						value.as_ptr(),
						0,
					)
				},
				"set a container tag",
			)?;
		}
		let mut dictionary = Dictionary::new(options)?;
		// SAFETY: the context has its streams and an open file.
		check(
			unsafe { ff::avformat_write_header(self.context, &mut dictionary.0) },
			"write the container header",
		)?;
		dictionary.reject_leftovers("muxer")
	}

	/// Write an written packet, timed in `time_base`, to `stream`.
	fn write(
		&self,
		packet: &Packet,
		stream: c_int,
		time_base: ff::AVRational,
	) -> Result<(), Error> {
		// SAFETY: the stream index came from `add_stream`; writing takes
		// the packet's data and leaves it empty.
		unsafe {
			(*packet.0).stream_index = stream;
			let target = (**(*self.context).streams.add(stream as usize)).time_base;
			ff::av_packet_rescale_ts(packet.0, time_base, target);
			check(
				ff::av_interleaved_write_frame(self.context, packet.0),
				"write a packet",
			)?;
		}
		Ok(())
	}

	fn finish(self) -> Result<(), Error> {
		// SAFETY: the header was written.
		check(
			unsafe { ff::av_write_trailer(self.context) },
			"finish the output file",
		)?;
		Ok(())
	}
}

impl Drop for Output {
	fn drop(&mut self) {
		// SAFETY: the file was opened by avio_open when `opened` is set, and
		// the context was allocated by avformat_alloc_output_context2.
		unsafe {
			if self.opened {
				ff::avio_closep(&mut (*self.context).pb);
			}
			ff::avformat_free_context(self.context);
		}
	}
}

struct Codec(*mut ff::AVCodecContext);

impl Codec {
	fn alloc(codec: *const ff::AVCodec) -> Result<Self, Error> {
		// SAFETY: allocates a context for a static codec description.
		let context = unsafe { ff::avcodec_alloc_context3(codec) };
		if context.is_null() {
			return Err(FFmpegError::VideoCodecAllocation.into());
		}
		Ok(Self(context))
	}

	fn decoder(stream: &ff::AVStream) -> Result<Self, Error> {
		// SAFETY: the parameters live as long as the stream.
		let parameters = unsafe { &*stream.codecpar };
		// SAFETY: looks up a static codec description.
		let codec = unsafe { ff::avcodec_find_decoder(parameters.codec_id) };
		if codec.is_null() {
			return Err(FFmpegError::DecoderNotFound.into());
		}
		let decoder = Self::alloc(codec)?;
		// SAFETY: both are allocated.
		check(
			unsafe { ff::avcodec_parameters_to_context(decoder.0, parameters) },
			"set up the decoder",
		)?;
		// SAFETY: the context is allocated.
		unsafe { (*decoder.0).pkt_timebase = stream.time_base };
		Ok(decoder)
	}

	fn open_decoder(&self, threads: &str) -> Result<(), Error> {
		// SAFETY: the context was allocated for this codec.
		let codec = unsafe { (*self.0).codec };
		self.open(codec, &[("threads".to_string(), threads.to_string())])
	}

	fn open(&self, codec: *const ff::AVCodec, options: &[(String, String)]) -> Result<(), Error> {
		let mut settings = vec![("threads".to_string(), "auto".to_string())];
		settings.extend_from_slice(options);
		let mut dictionary = Dictionary::new(&settings)?;
		// SAFETY: the context is allocated and configured.
		check(
			unsafe { ff::avcodec_open2(self.0, codec, &mut dictionary.0) },
			"open the codec",
		)?;
		dictionary.reject_leftovers("codec")
	}

	/// Take the next written packet; false when the encoder needs input or
	/// has finished.
	fn receive(&self, packet: &Packet) -> Result<bool, Error> {
		// SAFETY: the encoder is open and the packet allocated.
		match unsafe { ff::avcodec_receive_packet(self.0, packet.0) } {
			status if status == ff::AVERROR(ff::EAGAIN) || status == ff::AVERROR_EOF => Ok(false),
			status => check(status, "encode").map(|_| true),
		}
	}
}

impl Drop for Codec {
	fn drop(&mut self) {
		// SAFETY: allocated by avcodec_alloc_context3; this nulls the pointer.
		unsafe { ff::avcodec_free_context(&mut self.0) };
	}
}

struct Graph {
	raw: *mut ff::AVFilterGraph,
	source: *mut ff::AVFilterContext,
	sink: *mut ff::AVFilterContext,
}

impl Graph {
	/// A graph from `source`, a buffer filter and its arguments, through
	/// `chain` to a sink. Without a source, `chain` starts with a source
	/// filter of its own.
	fn build(source: Option<(&str, String)>, chain: &str, video: bool) -> Result<Self, Error> {
		// SAFETY: allocates an empty graph.
		let graph = unsafe { ff::avfilter_graph_alloc() };
		if graph.is_null() {
			return Err(FFmpegError::FilterGraphAllocation.into());
		}
		let mut built = Self {
			raw: graph,
			source: ptr::null_mut(),
			sink: ptr::null_mut(),
		};
		// The command line scales with bicubic unless told otherwise.
		// SAFETY: the graph owns and frees this string.
		unsafe { (*graph).scale_sws_opts = ff::av_strdup(c"flags=bicubic".as_ptr()) };

		if let Some((name, args)) = source {
			built.source = built.create(name, c"in", Some(&args))?;
		}
		built.sink = built.create(
			if video { "buffersink" } else { "abuffersink" },
			c"out",
			None,
		)?;

		let chain = CString::new(chain)?;
		// SAFETY: the in-out lists are allocated here and freed on every
		// path; their names are duplicated because FFmpeg frees them.
		unsafe {
			let mut inputs = inout(c"out", built.sink);
			let mut outputs = if built.source.is_null() {
				ptr::null_mut()
			} else {
				inout(c"in", built.source)
			};
			let parsed = ff::avfilter_graph_parse_ptr(
				graph,
				chain.as_ptr(),
				&mut inputs,
				&mut outputs,
				ptr::null_mut(),
			);
			ff::avfilter_inout_free(&mut inputs);
			ff::avfilter_inout_free(&mut outputs);
			check(parsed, "parse the filters")?;
			check(
				ff::avfilter_graph_config(graph, ptr::null_mut()),
				"configure the filters",
			)?;
		}
		Ok(built)
	}

	fn create(
		&self,
		name: &str,
		label: &CStr,
		args: Option<&str>,
	) -> Result<*mut ff::AVFilterContext, Error> {
		let name = CString::new(name)?;
		let args = args.map(CString::new).transpose()?;
		let mut context = ptr::null_mut();
		// SAFETY: the graph is allocated; FFmpeg copies the strings.
		check(
			unsafe {
				ff::avfilter_graph_create_filter(
					&mut context,
					ff::avfilter_get_by_name(name.as_ptr()),
					label.as_ptr(),
					args.as_ref().map_or(ptr::null(), |args| args.as_ptr()),
					ptr::null_mut(),
					self.raw,
				)
			},
			"create a filter",
		)?;
		Ok(context)
	}

	fn push(&self, frame: *mut ff::AVFrame) -> Result<(), Error> {
		// SAFETY: graphs fed by a decoder have a source; a null frame marks
		// the end of the input.
		check(
			unsafe {
				ff::av_buffersrc_add_frame_flags(
					self.source,
					frame,
					ff::AV_BUFFERSRC_FLAG_KEEP_REF as c_int,
				)
			},
			"filter a frame",
		)?;
		Ok(())
	}

	/// Take the next filtered frame; false when the graph needs input or
	/// has finished.
	fn pull(&self, frame: &Frame) -> Result<bool, Error> {
		// SAFETY: the sink is configured and the frame allocated and empty.
		match unsafe { ff::av_buffersink_get_frame(self.sink, frame.0) } {
			status if status == ff::AVERROR(ff::EAGAIN) || status == ff::AVERROR_EOF => Ok(false),
			status => check(status, "filter").map(|_| true),
		}
	}

	fn output_time_base(&self) -> ff::AVRational {
		// SAFETY: the sink is configured.
		unsafe { ff::av_buffersink_get_time_base(self.sink) }
	}

	fn output_frame_rate(&self) -> ff::AVRational {
		// SAFETY: the sink is configured.
		unsafe { ff::av_buffersink_get_frame_rate(self.sink) }
	}

	/// Sample format, rate and channels of the filtered audio, timed in
	/// samples.
	fn configure_audio_encoder(&self, encoder: &Codec, global_header: bool) -> Result<(), Error> {
		// SAFETY: the sink is configured and the encoder allocated.
		unsafe {
			let context = &mut *encoder.0;
			context.sample_fmt = sample_format(ff::av_buffersink_get_format(self.sink));
			context.sample_rate = ff::av_buffersink_get_sample_rate(self.sink);
			let mut layout = ChannelLayout(std::mem::zeroed());
			check(
				ff::av_buffersink_get_ch_layout(self.sink, &mut layout.0),
				"read the channel layout",
			)?;
			check(
				ff::av_channel_layout_copy(&mut context.ch_layout, &layout.0),
				"set the channel layout",
			)?;
			context.time_base = ff::AVRational {
				num: 1,
				den: context.sample_rate,
			};
			if global_header {
				context.flags |= ff::AV_CODEC_FLAG_GLOBAL_HEADER as c_int;
			}
		}
		Ok(())
	}

	/// Size, format and timing of the filtered video, and the colour tags
	/// of the decoded one, as the command line hands them to the encoder.
	fn configure_video_encoder(
		&self,
		encoder: &Codec,
		decoder: Option<&Codec>,
		global_header: bool,
	) {
		// SAFETY: the sink is configured and the encoder allocated.
		unsafe {
			let context = &mut *encoder.0;
			context.width = ff::av_buffersink_get_w(self.sink);
			context.height = ff::av_buffersink_get_h(self.sink);
			context.pix_fmt = pixel_format(ff::av_buffersink_get_format(self.sink));
			context.sample_aspect_ratio = ff::av_buffersink_get_sample_aspect_ratio(self.sink);
			context.time_base = self.output_time_base();
			context.framerate = self.output_frame_rate();
			if let Some(decoder) = decoder {
				let source = &*decoder.0;
				context.color_range = source.color_range;
				context.color_primaries = source.color_primaries;
				context.color_trc = source.color_trc;
				context.colorspace = source.colorspace;
				context.chroma_sample_location = source.chroma_sample_location;
			}
			if global_header {
				context.flags |= ff::AV_CODEC_FLAG_GLOBAL_HEADER as c_int;
			}
		}
	}
}

impl Drop for Graph {
	fn drop(&mut self) {
		// SAFETY: allocated by avfilter_graph_alloc; frees its filters too.
		unsafe { ff::avfilter_graph_free(&mut self.raw) };
	}
}

/// # Safety
/// `filter` belongs to a graph that outlives the returned list.
unsafe fn inout(name: &CStr, filter: *mut ff::AVFilterContext) -> *mut ff::AVFilterInOut {
	let entry = ff::avfilter_inout_alloc();
	(*entry).name = ff::av_strdup(name.as_ptr());
	(*entry).filter_ctx = filter;
	(*entry).pad_idx = 0;
	(*entry).next = ptr::null_mut();
	entry
}

struct Frame(*mut ff::AVFrame);

impl Frame {
	fn new() -> Result<Self, Error> {
		// SAFETY: allocates an empty frame.
		let frame = unsafe { ff::av_frame_alloc() };
		if frame.is_null() {
			return Err(FFmpegError::FrameAllocation.into());
		}
		Ok(Self(frame))
	}

	fn unref(&self) {
		// SAFETY: the frame is allocated.
		unsafe { ff::av_frame_unref(self.0) };
	}
}

impl Drop for Frame {
	fn drop(&mut self) {
		// SAFETY: allocated by av_frame_alloc; this nulls the pointer.
		unsafe { ff::av_frame_free(&mut self.0) };
	}
}

struct Packet(*mut ff::AVPacket);

impl Packet {
	fn new() -> Result<Self, Error> {
		// SAFETY: allocates an empty packet.
		let packet = unsafe { ff::av_packet_alloc() };
		if packet.is_null() {
			return Err(FFmpegError::FrameAllocation.into());
		}
		Ok(Self(packet))
	}

	fn stream_index(&self) -> c_int {
		// SAFETY: the packet is allocated.
		unsafe { (*self.0).stream_index }
	}

	fn data(&self) -> &[u8] {
		// SAFETY: a filled packet holds `size` bytes at `data`.
		unsafe {
			let packet = &*self.0;
			if packet.data.is_null() || packet.size <= 0 {
				return &[];
			}
			std::slice::from_raw_parts(packet.data, packet.size as usize)
		}
	}

	fn unref(&self) {
		// SAFETY: the packet is allocated.
		unsafe { ff::av_packet_unref(self.0) };
	}
}

impl Drop for Packet {
	fn drop(&mut self) {
		// SAFETY: allocated by av_packet_alloc; this nulls the pointer.
		unsafe { ff::av_packet_free(&mut self.0) };
	}
}

struct Dictionary(*mut ff::AVDictionary);

impl Dictionary {
	fn new(entries: &[(String, String)]) -> Result<Self, Error> {
		let mut dictionary = Self(ptr::null_mut());
		for (key, value) in entries {
			let (key, value) = (CString::new(key.as_str())?, CString::new(value.as_str())?);
			// SAFETY: FFmpeg allocates the dictionary and copies the strings.
			check(
				unsafe { ff::av_dict_set(&mut dictionary.0, key.as_ptr(), value.as_ptr(), 0) },
				"set an option",
			)?;
		}
		Ok(dictionary)
	}

	/// Options `FFmpeg` did not consume are misspelled or unsupported, and
	/// the command line refuses them too.
	fn reject_leftovers(&self, kind: &str) -> Result<(), Error> {
		// SAFETY: a null or allocated dictionary.
		let entry = unsafe {
			ff::av_dict_get(self.0, c"".as_ptr(), ptr::null(), ff::AV_DICT_IGNORE_SUFFIX)
		};
		if entry.is_null() {
			return Ok(());
		}
		// SAFETY: a found entry has a key.
		let key = unsafe { text((*entry).key) }.unwrap_or_default();
		Err(Error::Media(format!("the {kind} has no option {key}")))
	}
}

impl Drop for Dictionary {
	fn drop(&mut self) {
		// SAFETY: a null or allocated dictionary; this nulls the pointer.
		unsafe { ff::av_dict_free(&mut self.0) };
	}
}

struct ChannelLayout(ff::AVChannelLayout);

impl ChannelLayout {
	fn default_for(channels: i32) -> Self {
		// SAFETY: a zeroed layout is a valid empty one to fill.
		let mut layout = Self(unsafe { std::mem::zeroed() });
		// SAFETY: fills the layout in place.
		unsafe { ff::av_channel_layout_default(&mut layout.0, channels) };
		layout
	}
}

impl Drop for ChannelLayout {
	fn drop(&mut self) {
		// SAFETY: releases a custom channel map, if any.
		unsafe { ff::av_channel_layout_uninit(&mut self.0) };
	}
}

fn layout_name(layout: &ff::AVChannelLayout) -> Result<String, Error> {
	let mut buffer = [0u8; 128];
	// SAFETY: FFmpeg writes a terminated string within the buffer.
	check(
		unsafe { ff::av_channel_layout_describe(layout, buffer.as_mut_ptr().cast(), buffer.len()) },
		"describe the channel layout",
	)?;
	Ok(CStr::from_bytes_until_nul(&buffer)
		.map_err(|_| Error::Media("the channel layout has no name".into()))?
		.to_string_lossy()
		.into_owned())
}

fn find_encoder(name: &str) -> Result<*const ff::AVCodec, Error> {
	let name = CString::new(name)?;
	// SAFETY: looks up a static codec description.
	let codec = unsafe { ff::avcodec_find_encoder_by_name(name.as_ptr()) };
	if codec.is_null() {
		return Err(FFmpegError::EncoderNotFound.into());
	}
	Ok(codec)
}

fn tag(dictionary: *const ff::AVDictionary, key: &CStr) -> Option<String> {
	// SAFETY: a null or valid dictionary.
	let entry = unsafe { ff::av_dict_get(dictionary, key.as_ptr(), ptr::null(), 0) };
	if entry.is_null() {
		return None;
	}
	// SAFETY: a found entry has a value.
	unsafe { text((*entry).value) }
}

/// # Safety
/// `value` is null or a terminated string.
unsafe fn text(value: *const c_char) -> Option<String> {
	if value.is_null() {
		return None;
	}
	Some(CStr::from_ptr(value).to_string_lossy().into_owned())
}

/// Errors come back as return codes, so the library's own log stays off.
fn quiet() {
	// SAFETY: sets a global log level.
	unsafe { ff::av_log_set_level(ff::AV_LOG_FATAL) };
}

fn ratio(value: ff::AVRational) -> String {
	format!("{}/{}", value.num, value.den)
}

fn q2d(value: ff::AVRational) -> f64 {
	f64::from(value.num) / f64::from(value.den)
}

const fn time_base_q() -> ff::AVRational {
	ff::AVRational {
		num: 1,
		den: ff::AV_TIME_BASE,
	}
}

fn pixel_format(value: c_int) -> ff::AVPixelFormat {
	// SAFETY: FFmpeg hands out values of its own pixel format enum.
	unsafe { std::mem::transmute::<c_int, ff::AVPixelFormat>(value) }
}

fn sample_format(value: c_int) -> ff::AVSampleFormat {
	// SAFETY: FFmpeg hands out values of its own sample format enum.
	unsafe { std::mem::transmute::<c_int, ff::AVSampleFormat>(value) }
}

fn check(status: c_int, action: &str) -> Result<c_int, Error> {
	if status < 0 {
		return Err(Error::FFmpegWithReason(
			FFmpegError::from(status),
			format!("could not {action}"),
		));
	}
	Ok(status)
}
