pub struct MediaDataImage {
	width: i32,
	height: i32,
	lat: f64, // not sure if 32 or 64
	long: f64,
	fps: i32,
	device_make: String,
	device_model: String,
	device_software: String,
}

pub struct MediaDataVideo {
	width: i32,
	height: i32,
	lat: f64, // not sure if 32 or 64
	long: f64,
	fps: i32,
	device_make: String,
	device_model: String,
	device_software: String,
	duration: i32,
	video_codec: String, // enum thse
	audio_codec: String, // enum these
	stream_count: i32,   // we'll need to ues the ffmpeg crate for this one
}
