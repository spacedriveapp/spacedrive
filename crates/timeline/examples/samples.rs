//! Writes a sample rough cut in every format, for checking the writers
//! against the FCPXML DTD and OpenTimelineIO's reader.
//!
//! `cargo run -p sd-timeline --example samples -- <out dir> [<media dir>]`
//!
//! With a media dir, the clips point at `a.mov` (23.976, timecode
//! 14:01:20:00), `b.mov` (25, no audio) and `c.mov` (29.97 drop-frame)
//! there, which `scripts/footage/validate-timelines.sh` makes with FFmpeg.

use std::path::PathBuf;

use sd_timeline::{timecode, Audio, Clip, Media, Rate, Timeline};

fn main() {
	let mut args = std::env::args().skip(1);
	let out = PathBuf::from(args.next().expect("usage: samples <out dir> [<media dir>]"));
	let media_dir = args
		.next()
		.map(PathBuf::from)
		.unwrap_or_else(|| PathBuf::from("/Volumes/Footage/Japan Trip"));
	let ntsc = Rate::new(24000, 1001).unwrap();
	let pal = Rate::new(25, 1).unwrap();
	let ntsc30 = Rate::new(30000, 1001).unwrap();
	let stereo = Some(Audio {
		sample_rate: 48000,
		channels: 2,
	});
	let media = vec![
		Media {
			path: media_dir.join("a.mov"),
			name: "a.mov".into(),
			rate: ntsc,
			width: 1920,
			height: 1080,
			start_timecode: timecode::parse("14:01:20:00", ntsc).unwrap().0,
			drop_frame: false,
			duration: 2400,
			audio: stereo,
		},
		Media {
			path: media_dir.join("b.mov"),
			name: "b.mov".into(),
			rate: pal,
			width: 1280,
			height: 720,
			start_timecode: 0,
			drop_frame: false,
			duration: 500,
			audio: None,
		},
		Media {
			path: media_dir.join("c.mov"),
			name: "c.mov".into(),
			rate: ntsc30,
			width: 1920,
			height: 1080,
			start_timecode: timecode::parse("01:00:00;00", ntsc30).unwrap().0,
			drop_frame: true,
			duration: 2700,
			audio: stereo,
		},
	];
	let timeline = Timeline {
		name: "Train rough cut".into(),
		rate: ntsc,
		width: 1920,
		height: 1080,
		drop_frame: false,
		media,
		clips: vec![
			Clip {
				media: 0,
				source_in: 1230,
				source_out: 1302,
				label: None,
			},
			Clip {
				media: 1,
				source_in: 313,
				source_out: 450,
				label: Some("platform wide".into()),
			},
			Clip {
				media: 2,
				source_in: 1795,
				source_out: 1890,
				label: None,
			},
			Clip {
				media: 0,
				source_in: 2000,
				source_out: 2048,
				label: None,
			},
		],
	};
	// An EDL has one frame rate, so it is checked on a cut from one camera.
	let mut single = timeline.clone();
	single.name = "Train rough cut single rate".into();
	single.clips.retain(|clip| clip.media == 0);

	std::fs::create_dir_all(&out).expect("create the out dir");
	for timeline in [&timeline, &single] {
		for (extension, text) in [
			("fcpxml", sd_timeline::fcpxml::write(timeline)),
			("edl", sd_timeline::edl::write(timeline)),
			("otio", sd_timeline::otio::write(timeline)),
		] {
			let path = out.join(format!("{}.{extension}", timeline.name));
			std::fs::write(&path, text.expect("the sample timeline is valid")).expect("write");
		}
	}
}
