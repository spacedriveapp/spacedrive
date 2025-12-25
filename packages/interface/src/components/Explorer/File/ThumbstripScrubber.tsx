import { useState, useRef, useEffect, memo } from "react";
import clsx from "clsx";
import type { File } from "@sd/ts-client";
import { useServer } from "../../../ServerContext";

interface ThumbstripScrubberProps {
	file: File;
	size: number;
	onMouseEnter?: () => void;
	onMouseLeave?: () => void;
	/** Whether thumbnail is cropped to square (media view) or maintains aspect ratio */
	squareMode?: boolean;
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
	squareMode = false,
}: ThumbstripScrubberProps) {
	const [hoverProgress, setHoverProgress] = useState(0);
	const [isHovering, setIsHovering] = useState(false);
	const containerRef = useRef<HTMLDivElement>(null);
	const { buildSidecarUrl } = useServer();

	// Find thumbstrip sidecar
	const thumbstripSidecar = file.sidecars?.find(
		(s) => s.kind === "thumbstrip",
	);

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

	// Calculate dimensions based on mode
	let scrubberWidth = size;
	let scrubberHeight = size;
	let backgroundSizeWidth = grid.columns * 100;
	let backgroundSizeHeight = grid.rows * 100;

	if (squareMode) {
		// Square mode (media view): Each frame maintains aspect ratio and crops to fill square
		scrubberWidth = size;
		scrubberHeight = size;

		// Calculate background size so each frame fills the square with object-fit: cover behavior
		// Each frame should maintain its aspect ratio while filling the square container
		if (videoAspectRatio > 1) {
			// Landscape video: width must scale up to fill square height
			// If frame is 16:9 in 100x100 square, frame becomes 178x100 to fill height
			// Background for 5x5 grid: 5 * 178 = 890% wide, 5 * 100 = 500% tall
			backgroundSizeWidth = grid.columns * 100 * videoAspectRatio;
			backgroundSizeHeight = grid.rows * 100;
		} else {
			// Portrait video: height must scale up to fill square width
			// If frame is 9:16 in 100x100 square, frame becomes 100x178 to fill width
			// Background for 5x5 grid: 5 * 100 = 500% wide, 5 * 178 = 890% tall
			backgroundSizeWidth = grid.columns * 100;
			backgroundSizeHeight = (grid.rows * 100) / videoAspectRatio;
		}
	} else {
		// Aspect ratio mode: Maintain video aspect ratio within container
		if (videoAspectRatio > 1) {
			// Landscape video - constrain by width
			scrubberHeight = size / videoAspectRatio;
		} else {
			// Portrait video - constrain by height
			scrubberWidth = size * videoAspectRatio;
		}
	}

	// Build thumbstrip URL
	if (!file.content_identity?.uuid) {
		return null;
	}

	const thumbstripUrl = buildSidecarUrl(
		file.content_identity.uuid,
		thumbstripSidecar.kind,
		thumbstripSidecar.variant,
		thumbstripSidecar.format,
	);

	if (!thumbstripUrl) {
		return null;
	}

	// Calculate which frame to show based on hover position
	const frameIndex = Math.min(
		Math.floor(hoverProgress * totalFrames),
		totalFrames - 1,
	);

	const row = Math.floor(frameIndex / grid.columns);
	const col = frameIndex % grid.columns;

	// Calculate sprite position (as percentages for responsive sizing)
	// CSS backgroundPosition percentage = (container - background) * percentage
	// For uniform scaling (500% x 500%): standard formula works
	// For non-uniform scaling: need to adjust for actual background dimensions
	let spriteX: number;
	let spriteY: number;

	if (grid.columns > 1) {
		// How much we need to offset: col * (100% / columns) of background size
		// backgroundPosition % = offset / (container - background)
		const offsetXPercent = (col / grid.columns) * backgroundSizeWidth;
		spriteX = (offsetXPercent / (backgroundSizeWidth - 100)) * 100;
	} else {
		spriteX = 0;
	}

	if (grid.rows > 1) {
		const offsetYPercent = (row / grid.rows) * backgroundSizeHeight;
		spriteY = (offsetYPercent / (backgroundSizeHeight - 100)) * 100;
	} else {
		spriteY = 0;
	}

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
			className="absolute inset-0 flex items-center justify-center z-10 pointer-events-auto"
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
						// Use calculated background size for proper sprite sheet scaling
						backgroundSize: `${backgroundSizeWidth}% ${backgroundSizeHeight}%`,
						// Always use sprite coordinates for positioning
						backgroundPosition: `${spriteX}% ${spriteY}%`,
						backgroundRepeat: "no-repeat",
						imageRendering: "crisp-edges",
					}}
				>
					{/* Progress indicator */}
					<div className="absolute bottom-1 left-1 right-1 h-0.5 bg-black/50 rounded-full overflow-hidden">
						<div
							className="h-full bg-accent"
							style={{ width: `${hoverProgress * 100}%` }}
						/>
					</div>
				</div>
			)}
		</div>
	);
});
