import type { TranscriptSegment } from "@sd/ts-client";
import { activeSegmentIndex, useMediaTimeMs } from "./TranscriptPanel";

export interface SubtitleSettings {
	fontSize: number; // 0.8 to 2.0
	position: "bottom" | "top";
	backgroundOpacity: number; // 0 to 1
}

interface SubtitlesProps {
	segments: TranscriptSegment[];
	videoElement: HTMLVideoElement | null;
	settings: SubtitleSettings;
}

/** The transcript line being spoken, drawn over the video. */
export function Subtitles({
	segments,
	videoElement,
	settings,
}: SubtitlesProps) {
	const timeMs = useMediaTimeMs(videoElement);
	const segment = segments[activeSegmentIndex(segments, timeMs)];

	if (!segment) {
		return null;
	}

	const positionClass = settings.position === "top" ? "top-16" : "bottom-16";

	return (
		<div
			className={`pointer-events-none absolute ${positionClass} left-0 right-0 flex justify-center px-8`}
		>
			<div
				className="max-w-4xl rounded-lg px-6 py-3 text-center backdrop-blur-sm"
				style={{
					backgroundColor: `rgba(0, 0, 0, ${settings.backgroundOpacity})`,
				}}
			>
				<p
					className="leading-relaxed text-white"
					style={{
						fontSize: `${settings.fontSize}rem`,
					}}
				>
					{segment.text}
				</p>
			</div>
		</div>
	);
}
