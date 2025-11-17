import { useState, useRef, useEffect, memo } from "react";
import clsx from "clsx";
import type { File } from "@sd/ts-client/generated/types";

interface ThumbstripScrubberProps {
	file: File;
	size: number;
	onMouseEnter?: () => void;
	onMouseLeave?: () => void;
}

/**
 * ThumbstripScrubber - Interactive video preview using thumbstrip sprite sheet
 *
 * Shows different frames from a video thumbstrip as the user moves their mouse
 * across the thumbnail. Uses CSS sprite technique for efficient rendering.
 *
 * Only renders when:
 * - File is a video
 * - Thumbstrip sidecar exists
 * - User is hovering over the thumbnail
 */
export const ThumbstripScrubber = memo(function ThumbstripScrubber({
	file,
	size,
	onMouseEnter,
	onMouseLeave,
}: ThumbstripScrubberProps) {
	const [hoverProgress, setHoverProgress] = useState(0);
	const [isHovering, setIsHovering] = useState(false);
	const containerRef = useRef<HTMLDivElement>(null);
	console.log("file in thumbstrip scrubber", file);
	// Find thumbstrip sidecar
	const thumbstripSidecar = file.sidecars?.find(
		(s) => s.kind === "thumbstrip",
	);
	console.log("thumbstripSidecar in thumbstrip scrubber", thumbstripSidecar);

	if (!thumbstripSidecar) {
		return null;
	}

	// Parse grid dimensions from variant name (e.g., "thumbstrip_preview" = 5x5)
	// Default to 5x5 if we can't parse
	const getGridDimensions = (variant: string) => {
		if (variant.includes("detailed")) return { columns: 10, rows: 10 };
		if (variant.includes("mobile")) return { columns: 3, rows: 3 };
		return { columns: 5, rows: 5 }; // preview or default
	};

	const grid = getGridDimensions(thumbstripSidecar.variant);
	const totalFrames = grid.columns * grid.rows;

	// Calculate aspect ratio from video metadata (default to 16:9)
	const videoAspectRatio =
		file.video_media_data?.width && file.video_media_data?.height
			? file.video_media_data.width / file.video_media_data.height
			: 16 / 9;

	// Calculate dimensions to maintain aspect ratio within square container
	let scrubberWidth = size;
	let scrubberHeight = size;

	if (videoAspectRatio > 1) {
		// Landscape video - constrain by width
		scrubberHeight = size / videoAspectRatio;
	} else {
		// Portrait video - constrain by height
		scrubberWidth = size * videoAspectRatio;
	}

	// Build thumbstrip URL
	const serverUrl = (window as any).__SPACEDRIVE_SERVER_URL__;
	const libraryId = (window as any).__SPACEDRIVE_LIBRARY_ID__;

	if (!serverUrl || !libraryId || !file.content_identity?.uuid) {
		return null;
	}

	const thumbstripUrl = `${serverUrl}/sidecar/${libraryId}/${file.content_identity.uuid}/${thumbstripSidecar.kind}/${thumbstripSidecar.variant}.${thumbstripSidecar.format}`;

	console.log("thumbstripUrl in thumbstrip scrubber", thumbstripUrl);

	// Calculate which frame to show based on hover position
	const frameIndex = Math.min(
		Math.floor(hoverProgress * totalFrames),
		totalFrames - 1,
	);

	const row = Math.floor(frameIndex / grid.columns);
	const col = frameIndex % grid.columns;

	// Calculate sprite position (as percentages for responsive sizing)
	// Avoid division by zero for 1x1 grids
	const spriteX = grid.columns > 1 ? (col / (grid.columns - 1)) * 100 : 0;
	const spriteY = grid.rows > 1 ? (row / (grid.rows - 1)) * 100 : 0;

	// Handle mouse move to update hover progress
	const handleMouseMove = (e: React.MouseEvent<HTMLDivElement>) => {
		if (!containerRef.current) return;

		const rect = containerRef.current.getBoundingClientRect();
		const x = e.clientX - rect.left;
		const progress = Math.max(0, Math.min(1, x / rect.width));

		setHoverProgress(progress);
	};

	const handleMouseEnter = () => {
		setIsHovering(true);
		onMouseEnter?.();
	};

	const handleMouseLeave = () => {
		setIsHovering(false);
		setHoverProgress(0);
		onMouseLeave?.();
	};

	return (
		<div
			ref={containerRef}
			className="absolute inset-0 flex items-center justify-center z-10"
			onMouseMove={handleMouseMove}
			onMouseEnter={handleMouseEnter}
			onMouseLeave={handleMouseLeave}
		>
			{/* Thumbstrip sprite background (only visible on hover) */}
			{isHovering && (
				<div
					className="rounded-lg bg-app-darkBox/95 backdrop-blur-sm overflow-hidden shadow-lg relative"
					style={{
						width: scrubberWidth,
						height: scrubberHeight,
						backgroundImage: `url(${thumbstripUrl})`,
						backgroundSize: `${grid.columns * 100}% ${grid.rows * 100}%`,
						backgroundPosition: `${spriteX}% ${spriteY}%`,
						backgroundRepeat: "no-repeat",
						imageRendering: "crisp-edges",
					}}
				>
					{/* Progress indicator */}
					<div className="absolute bottom-1 left-1 right-1 h-0.5 bg-black/50 rounded-full overflow-hidden">
						<div
							className="h-full bg-accent transition-all duration-75"
							style={{ width: `${hoverProgress * 100}%` }}
						/>
					</div>
				</div>
			)}
		</div>
	);
});
