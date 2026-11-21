#!/usr/bin/env bash
# Checks the timeline writers against the FCPXML 1.10 DTD and OpenTimelineIO's
# own reader, on real media FFmpeg makes with known rates and timecode.
#
# usage: scripts/footage/validate-timelines.sh <FCPXMLv1_10.dtd> [work dir]
#
# The DTD ships inside Final Cut Pro
# (Final Cut Pro.app/Contents/Frameworks/Interchange.framework/Resources/FCPXMLv1_10.dtd)
# and is not in this repository. Needs ffmpeg, xmllint and uv.
set -euo pipefail

dtd="$(realpath "$1")"
work="$(realpath -m "${2:-$(mktemp -d)}")"
root="$(cd "$(dirname "$0")/../.." && pwd)"
mkdir -p "$work/media" "$work/out"

ffmpeg -loglevel error -y -f lavfi -i "testsrc2=size=1920x1080:rate=24000/1001" -f lavfi -i "sine=frequency=440:sample_rate=48000" \
	-t 100.1 -ac 2 -c:v libx264 -pix_fmt yuv420p -c:a aac -timecode 14:01:20:00 "$work/media/a.mov"
ffmpeg -loglevel error -y -f lavfi -i "testsrc2=size=1280x720:rate=25" -t 20 -c:v libx264 -pix_fmt yuv420p "$work/media/b.mov"
ffmpeg -loglevel error -y -f lavfi -i "testsrc2=size=1920x1080:rate=30000/1001" -f lavfi -i "sine=frequency=880:sample_rate=48000" \
	-t 90.09 -ac 2 -c:v libx264 -pix_fmt yuv420p -c:a aac -timecode "01:00:00;00" "$work/media/c.mov"

cargo run --quiet --manifest-path "$root/Cargo.toml" -p sd-timeline --example samples -- "$work/out" "$work/media"

for name in "Train rough cut" "Train rough cut single rate"; do
	xmllint --noout --dtdvalid "$dtd" "$work/out/$name.fcpxml"
done
echo "fcpxml: valid against $(basename "$dtd")"

if [ ! -x "$work/venv/bin/python" ]; then
	uv venv -q "$work/venv"
	uv pip install -q --python "$work/venv/bin/python" opentimelineio otio-fcpx-xml-adapter otio-cmx3600-adapter
fi
"$work/venv/bin/python" -I - "$work/out" <<'PY'
import sys
import xml.etree.ElementTree as ET
from fractions import Fraction

import opentimelineio as otio

out = sys.argv[1]


def video_clips(path, adapter=None):
	result = otio.adapters.read_from_file(path, adapter_name=adapter)
	if not isinstance(result, otio.schema.Timeline):
		result = result.find_children(descended_from_type=otio.schema.Timeline)[0]
	clips = []
	for track in result.video_tracks():
		for clip in track.find_clips():
			start = clip.source_range.start_time
			duration = clip.source_range.duration.rescaled_to(start.rate)
			clips.append((start.value, round(duration.value), start.rate))
	return clips


def seconds(value):
	return Fraction(value.rstrip("s"))


def fcpxml_clips(path):
	"""Source ranges from the FCPXML in exact rational time."""
	root = ET.parse(path).getroot()
	formats = {f.get("id"): f for f in root.iter("format")}
	assets = {a.get("id"): a for a in root.iter("asset")}
	sequence = root.find(".//sequence")
	clips = []
	for clip in root.iter("asset-clip"):
		source_format = formats[assets[clip.get("ref")].get("format")]
		rate = 1 / seconds(source_format.get("frameDuration"))
		start = seconds(clip.get("start")) * rate
		assert start.denominator == 1, f"{clip.get('name')} starts between frames"
		timeline_rate = 1 / seconds(formats[sequence.get("format")].get("frameDuration"))
		length = seconds(clip.get("duration")) * timeline_rate
		assert length.denominator == 1, f"{clip.get('name')} ends between timeline frames"
		clips.append((int(start), rate))
	return clips


mixed = f"{out}/Train rough cut"
reference = video_clips(f"{mixed}.otio")
print(f"otio: read by opentimelineio {otio.__version__}")
for clip in reference:
	print(f"    {otio.opentime.to_timecode(otio.opentime.RationalTime(clip[0], clip[2]), clip[2])} +{clip[1]} @ {clip[2]:.3f}")

exact = fcpxml_clips(f"{mixed}.fcpxml")
assert [(s, float(r)) for s, r in exact] == [(s, r) for s, _, r in reference], "FCPXML in points disagree with OTIO"
print("fcpxml: every in point is a whole source frame and matches the OTIO file")

adapter = video_clips(f"{mixed}.fcpxml", "fcpx_xml")
assert len(adapter) == len(reference)
for (start, _, rate), (a_start, _, a_rate) in zip(reference, adapter):
	# The FCPXML adapter truncates rates to whole numbers, so agreement is
	# checked in seconds to within one frame.
	assert abs(start / rate - a_start / a_rate) <= 1 / a_rate, (start, rate, a_start, a_rate)
print("fcpxml: OpenTimelineIO's FCPXML adapter reads the same clips to within its whole-number rates")

single = f"{out}/Train rough cut single rate"
edl = video_clips(f"{single}.edl", "cmx_3600")
assert [(s, d) for s, d, _ in edl] == [(s, d) for s, d, _ in video_clips(f"{single}.otio")], "EDL disagrees with OTIO"
print(f"edl: OpenTimelineIO's CMX 3600 adapter reads the same {len(edl)} source ranges")
PY

echo "outputs in $work/out"
