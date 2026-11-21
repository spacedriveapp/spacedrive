//! FCPXML 1.10, which Final Cut Pro 10.6 and later and DaVinci Resolve import.
//!
//! Every time is written as exact rational seconds at its own rate, so an in
//! point lands on the frame the clip names. Asset starts carry the file's
//! own start timecode, which is how Final Cut matches the XML to the file.

use std::fmt::Write;

use crate::{file_url, timeline::Edit, Rate, Timeline, TimelineError};

pub const VERSION: &str = "1.10";
/// The event a project lands in when Final Cut imports it.
pub const EVENT_NAME: &str = "Spacedrive";

/// Escapes text for a double-quoted XML attribute, dropping characters XML
/// cannot carry at all.
pub(crate) fn escape(text: &str) -> String {
	let mut out = String::with_capacity(text.len());
	for ch in text.chars() {
		match ch {
			'&' => out.push_str("&amp;"),
			'<' => out.push_str("&lt;"),
			'>' => out.push_str("&gt;"),
			'"' => out.push_str("&quot;"),
			'\'' => out.push_str("&apos;"),
			'\t' => out.push_str("&#9;"),
			'\n' => out.push_str("&#10;"),
			'\r' => out.push_str("&#13;"),
			c if (c as u32) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' => {}
			c => out.push(c),
		}
	}
	out
}

fn tc_format(drop_frame: bool, rate: Rate) -> &'static str {
	if drop_frame && rate.supports_drop_frame() {
		"DF"
	} else {
		"NDF"
	}
}

/// FCPXML's sequence audio rate names.
fn audio_rate(hz: u32) -> &'static str {
	match hz {
		32000 => "32k",
		44100 => "44.1k",
		88200 => "88.2k",
		96000 => "96k",
		176400 => "176.4k",
		192000 => "192k",
		_ => "48k",
	}
}

pub fn write(timeline: &Timeline) -> Result<String, TimelineError> {
	let edits = timeline.edits()?;
	let duration = edits.last().map_or(0, |edit| edit.record_out);

	let mut formats: Vec<(Rate, u32, u32)> = vec![(timeline.rate, timeline.width, timeline.height)];
	let mut format_of = Vec::with_capacity(timeline.media.len());
	for media in &timeline.media {
		let key = (media.rate, media.width, media.height);
		let index = formats.iter().position(|f| *f == key).unwrap_or_else(|| {
			formats.push(key);
			formats.len() - 1
		});
		format_of.push(index);
	}
	let format_id = |index: usize| format!("r{}", index + 1);
	let asset_id = |media: usize| format!("r{}", formats.len() + media + 1);

	let mut xml = String::new();
	xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE fcpxml>\n\n");
	let _ = writeln!(xml, "<fcpxml version=\"{VERSION}\">");
	xml.push_str("\t<resources>\n");
	for (index, (rate, width, height)) in formats.iter().enumerate() {
		let _ = writeln!(
			xml,
			"\t\t<format id=\"{}\" frameDuration=\"{}\" width=\"{width}\" height=\"{height}\"/>",
			format_id(index),
			rate.frame_duration()
		);
	}
	for (index, media) in timeline.media.iter().enumerate() {
		let _ = write!(
			xml,
			"\t\t<asset id=\"{}\" name=\"{}\" start=\"{}\" duration=\"{}\" hasVideo=\"1\" format=\"{}\" videoSources=\"1\"",
			asset_id(index),
			escape(&media.name),
			media.rate.rational_seconds(media.start_timecode),
			media.rate.rational_seconds(media.duration),
			format_id(format_of[index]),
		);
		match media.audio {
			Some(audio) => {
				let _ = write!(
					xml,
					" hasAudio=\"1\" audioSources=\"1\" audioChannels=\"{}\" audioRate=\"{}\"",
					audio.channels, audio.sample_rate
				);
			}
			None => xml.push_str(" hasAudio=\"0\""),
		}
		xml.push_str(">\n");
		let _ = writeln!(
			xml,
			"\t\t\t<media-rep kind=\"original-media\" src=\"{}\"/>",
			escape(&file_url(&media.path))
		);
		xml.push_str("\t\t</asset>\n");
	}
	xml.push_str("\t</resources>\n");
	xml.push_str("\t<library>\n");
	let _ = writeln!(xml, "\t\t<event name=\"{}\">", escape(EVENT_NAME));
	let _ = writeln!(xml, "\t\t\t<project name=\"{}\">", escape(&timeline.name));
	let sample_rate = timeline
		.media
		.iter()
		.find_map(|media| media.audio)
		.map_or(48000, |audio| audio.sample_rate);
	let _ = writeln!(
		xml,
		"\t\t\t\t<sequence format=\"{}\" duration=\"{}\" tcStart=\"0s\" tcFormat=\"{}\" audioLayout=\"stereo\" audioRate=\"{}\">",
		format_id(0),
		timeline.rate.rational_seconds(duration),
		tc_format(timeline.drop_frame, timeline.rate),
		audio_rate(sample_rate),
	);
	xml.push_str("\t\t\t\t\t<spine>\n");
	for edit in &edits {
		write_clip(
			&mut xml,
			timeline,
			edit,
			&format_id(format_of[edit.clip.media]),
			&asset_id(edit.clip.media),
		);
	}
	xml.push_str("\t\t\t\t\t</spine>\n");
	xml.push_str("\t\t\t\t</sequence>\n");
	xml.push_str("\t\t\t</project>\n");
	xml.push_str("\t\t</event>\n");
	xml.push_str("\t</library>\n");
	xml.push_str("</fcpxml>\n");
	Ok(xml)
}

fn write_clip(xml: &mut String, timeline: &Timeline, edit: &Edit<'_>, format: &str, asset: &str) {
	let media = edit.media;
	let name = edit.clip.label.as_deref().unwrap_or(&media.name);
	let _ = write!(
		xml,
		"\t\t\t\t\t\t<asset-clip ref=\"{asset}\" offset=\"{}\" name=\"{}\" start=\"{}\" duration=\"{}\"",
		timeline.rate.rational_seconds(edit.record_in),
		escape(name),
		media.rate.rational_seconds(media.start_timecode + edit.clip.source_in),
		timeline.rate.rational_seconds(edit.record_out - edit.record_in),
	);
	if format != "r1" {
		let _ = write!(xml, " format=\"{format}\"");
	}
	let _ = writeln!(
		xml,
		" tcFormat=\"{}\"/>",
		tc_format(media.drop_frame, media.rate)
	);
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::timeline::tests::sample;

	#[test]
	fn in_points_are_exact_rational_times() {
		let xml = write(&sample()).unwrap();
		// 14:01:20:00 at 23.976 is frame 1,211,520; the clip starts 1230 later.
		assert!(xml.contains("<asset-clip ref=\"r3\" offset=\"0s\" name=\"A001_C004.mov\" start=\"1213962750/24000s\" duration=\"72072/24000s\" tcFormat=\"NDF\"/>"), "{xml}");
		assert!(xml.contains("start=\"313/25s\""), "{xml}");
		assert!(xml.contains("offset=\"72072/24000s\""), "{xml}");
		assert!(xml.contains("format=\"r2\" tcFormat"), "{xml}");
		assert!(xml.contains("<project name=\"Train &lt;rough&gt; cut\">"));
		assert!(xml.contains("name=\"A003_C011 &amp; co.mov\""));
		assert!(xml.contains("src=\"file:///Volumes/Footage/Japan%20Trip/A001_C004.mov\""));
		assert!(xml.contains("<sequence format=\"r1\" duration=\"251251/24000s\""));
	}

	#[test]
	fn escape_drops_what_xml_cannot_hold() {
		assert_eq!(escape("a\u{1}b\"c'"), "ab&quot;c&apos;");
	}
}
