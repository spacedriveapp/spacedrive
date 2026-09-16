import { memo } from "react";
import type { File } from "@sd/ts-client";
import { useServer } from "../../contexts/ServerContext";
import { useHotThumbstrip } from "../../routes/explorer/hooks/useHotThumbstrip";

interface TimelineScrubberProps {
	file: File;
	hoverPercent: number;
	duration: number;
}

/**
 * TimelineScrubber - Shows video frame preview when hovering over timeline
 *
 * Uses thumbstrip sprite sheet to display the frame at the hovered position
 * Similar to YouTube's timeline preview feature
 */
export const TimelineScrubber = memo(function TimelineScrubber({
	file,
	hoverPercent,
	duration,
}: TimelineScrubberProps) {
	const { buildSidecarUrl } = useServer();

	// Find thumbstrip sidecar
	const thumbstripSidecar = file.sidecars?.find(
		(s) => s.kind === "thumbstrip",
	);
	const hot = useHotThumbstrip(file.sd_path, !thumbstripSidecar);

	// Parse grid dimensions
	const getGridDimensions = (variant: string) => {
		if (variant.includes("detailed")) return { columns: 10, rows: 10 };
		if (variant.includes("mobile")) return { columns: 3, rows: 3 };
		return { columns: 5, rows: 5 };
	};

	const grid = thumbstripSidecar
		? getGridDimensions(thumbstripSidecar.variant)
		: { columns: hot.columns, rows: hot.rows };
	const totalFrames = grid.columns * grid.rows;

	// Build thumbstrip URL
	const sidecarUrl =
		thumbstripSidecar && file.content_identity?.uuid
			? buildSidecarUrl(
					file.content_identity.uuid,
					thumbstripSidecar.kind,
					thumbstripSidecar.variant,
					thumbstripSidecar.format,
					thumbstripSidecar.version,
				)
			: null;
	const thumbstripUrl = sidecarUrl ?? hot.url;

	if (!thumbstripUrl) {
		return null;
	}

	// Calculate which frame to show
	const frameIndex = Math.min(
		Math.floor(hoverPercent * totalFrames),
		totalFrames - 1,
	);

	const row = Math.floor(frameIndex / grid.columns);
	const col = frameIndex % grid.columns;

	// Calculate sprite position
	const spriteX = grid.columns > 1 ? (col / (grid.columns - 1)) * 100 : 0;
	const spriteY = grid.rows > 1 ? (row / (grid.rows - 1)) * 100 : 0;

	// Preview dimensions (fixed width, 16:9 aspect ratio)
	const previewWidth = 160;
	const previewHeight = 90;

	// The timeline sits inside the controls' 1rem padding on either side.
	const percent = Math.max(0, Math.min(1, hoverPercent));
	const leftPosition = `clamp(10px, calc(${percent * 100}% + ${1 - 2 * percent}rem - ${previewWidth / 2}px), calc(100% - ${previewWidth + 10}px))`;

	// Format timestamp
	const timestamp = formatTime(hoverPercent * duration);

	return (
		<div
			className="absolute z-50 pointer-events-none"
			style={{
				left: leftPosition,
				bottom: 80, // Just above the timeline
				width: previewWidth,
			}}
		>
			{/* Preview frame */}
			<div
				className="rounded-lg bg-black border-2 border-white overflow-hidden shadow-2xl"
				style={{
					width: previewWidth,
					height: previewHeight,
					backgroundImage: `url(${thumbstripUrl})`,
					backgroundSize: `${grid.columns * 100}% ${grid.rows * 100}%`,
					backgroundPosition: `${spriteX}% ${spriteY}%`,
					backgroundRepeat: "no-repeat",
					imageRendering: "crisp-edges",
				}}
			/>

			{/* Timestamp below preview */}
			<div className="mt-1 flex justify-center">
				<div className="rounded bg-black/90 px-2 py-0.5 text-xs font-mono text-white">
					{timestamp}
				</div>
			</div>

			{/* Pointer arrow */}
			<div className="absolute left-1/2 top-full -translate-x-1/2">
				<div className="size-0 border-l-4 border-r-4 border-t-4 border-l-transparent border-r-transparent border-t-white/20" />
			</div>
		</div>
	);
});

function formatTime(seconds: number): string {
	const hours = Math.floor(seconds / 3600);
	const mins = Math.floor((seconds % 3600) / 60);
	const secs = Math.floor(seconds % 60);

	if (hours > 0) {
		return `${hours}:${mins.toString().padStart(2, "0")}:${secs.toString().padStart(2, "0")}`;
	}
	return `${mins}:${secs.toString().padStart(2, "0")}`;
}
