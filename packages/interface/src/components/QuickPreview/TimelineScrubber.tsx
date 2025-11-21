import { memo } from 'react';
import type { File } from '@sd/ts-client';

interface TimelineScrubberProps {
	file: File;
	hoverPercent: number;
	mouseX: number;
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
	mouseX,
	duration,
}: TimelineScrubberProps) {
	// Find thumbstrip sidecar
	const thumbstripSidecar = file.sidecars?.find(
		(s) => s.kind === 'thumbstrip'
	);

	if (!thumbstripSidecar) {
		return null;
	}

	// Parse grid dimensions
	const getGridDimensions = (variant: string) => {
		if (variant.includes('detailed')) return { columns: 10, rows: 10 };
		if (variant.includes('mobile')) return { columns: 3, rows: 3 };
		return { columns: 5, rows: 5 };
	};

	const grid = getGridDimensions(thumbstripSidecar.variant);
	const totalFrames = grid.columns * grid.rows;

	// Build thumbstrip URL
	const serverUrl = (window as any).__SPACEDRIVE_SERVER_URL__;
	const libraryId = (window as any).__SPACEDRIVE_LIBRARY_ID__;

	if (!serverUrl || !libraryId || !file.content_identity?.uuid) {
		return null;
	}

	const thumbstripUrl = `${serverUrl}/sidecar/${libraryId}/${file.content_identity.uuid}/${thumbstripSidecar.kind}/${thumbstripSidecar.variant}.${thumbstripSidecar.format}`;

	// Calculate which frame to show
	const frameIndex = Math.min(
		Math.floor(hoverPercent * totalFrames),
		totalFrames - 1
	);

	const row = Math.floor(frameIndex / grid.columns);
	const col = frameIndex % grid.columns;

	// Calculate sprite position
	const spriteX = grid.columns > 1 ? (col / (grid.columns - 1)) * 100 : 0;
	const spriteY = grid.rows > 1 ? (row / (grid.rows - 1)) * 100 : 0;

	// Preview dimensions (fixed width, 16:9 aspect ratio)
	const previewWidth = 160;
	const previewHeight = 90;

	// Position horizontally following mouse, clamped to screen bounds
	const leftPosition = Math.max(
		10,
		Math.min(mouseX - previewWidth / 2, window.innerWidth - previewWidth - 10)
	);

	// Format timestamp
	const timestamp = formatTime(hoverPercent * duration);

	return (
		<div
			className="fixed z-50 pointer-events-none"
			style={{
				left: leftPosition,
				bottom: 160, // Well above the timeline
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
					backgroundRepeat: 'no-repeat',
					imageRendering: 'crisp-edges',
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
		return `${hours}:${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
	}
	return `${mins}:${secs.toString().padStart(2, '0')}`;
}

