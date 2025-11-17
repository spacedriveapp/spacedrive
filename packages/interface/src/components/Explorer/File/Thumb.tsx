import { useState, memo, useEffect } from "react";
import clsx from "clsx";
import { getIcon } from "@sd/assets/util";
import type { File } from "@sd/ts-client/generated/types";
import { ThumbstripScrubber } from "./ThumbstripScrubber";

interface ThumbProps {
	file: File;
	size?: number;
	className?: string;
	frameClassName?: string; // Custom frame styling (border, radius, bg)
	iconScale?: number; // Scale factor for fallback icon (0-1, default 1)
}

// Global cache for thumbnail loaded states (survives component unmount/remount)
const thumbLoadedCache = new Map<string, boolean>();
const thumbErrorCache = new Map<string, boolean>();

export const Thumb = memo(function Thumb({
	file,
	size = 100,
	className,
	frameClassName,
	iconScale = 1,
}: ThumbProps) {
	const cacheKey = `${file.id}-${size}`;

	const [thumbLoaded, setThumbLoaded] = useState(
		() => thumbLoadedCache.get(cacheKey) || false,
	);
	const [thumbError, setThumbError] = useState(
		() => thumbErrorCache.get(cacheKey) || false,
	);

	// Update cache when state changes
	useEffect(() => {
		if (thumbLoaded) thumbLoadedCache.set(cacheKey, true);
	}, [thumbLoaded, cacheKey]);

	useEffect(() => {
		if (thumbError) thumbErrorCache.set(cacheKey, true);
	}, [thumbError, cacheKey]);

	const iconSize = size * iconScale;

	// Check if this is a video with thumbstrip sidecar
	const isVideo = file.content_identity?.kind === "video";
	const hasThumbstrip = file.sidecars?.some((s) => s.kind === "thumbstrip");

	// Get appropriate thumbnail URL from sidecars based on size
	const getThumbnailUrl = (targetSize: number) => {
		const serverUrl = (window as any).__SPACEDRIVE_SERVER_URL__;
		const libraryId = (window as any).__SPACEDRIVE_LIBRARY_ID__;

		if (!serverUrl || !libraryId) {
			return null;
		}

		// Need content_identity to build sidecar URL
		if (!file.content_identity?.uuid) {
			return null;
		}

		// Find thumbnail sidecar closest to requested size
		const thumbnails = file.sidecars.filter((s) => s.kind === "thumb");

		if (thumbnails.length === 0) {
			return null;
		}

		// Prefer 1x (lower resolution) variants for better performance
		// Only use higher resolution for very large sizes (>400px)
		const preferredSize = targetSize <= 400 ? targetSize * 0.6 : targetSize;

		const thumbnail = thumbnails.sort((a, b) => {
			// Parse variant (e.g., "grid@1x", "detail@1x") to get size and scale
			const aSize = parseInt(
				a.variant.split("x")[0]?.replace(/\D/g, "") || "0",
			);
			const bSize = parseInt(
				b.variant.split("x")[0]?.replace(/\D/g, "") || "0",
			);

			// Extract scale factor (1x, 2x, 3x) from variants like "grid@1x" or "detail@2x"
			const aScaleMatch = a.variant.match(/@(\d+)x/);
			const bScaleMatch = b.variant.match(/@(\d+)x/);
			const aScale = aScaleMatch ? parseInt(aScaleMatch[1]) : 1;
			const bScale = bScaleMatch ? parseInt(bScaleMatch[1]) : 1;

			// Strongly prefer 1x variants (add penalty for higher scales)
			const aPenalty = (aScale - 1) * 100;
			const bPenalty = (bScale - 1) * 100;

			// Find closest match to preferred size, with scale penalty
			return (
				Math.abs(aSize - preferredSize) +
				aPenalty -
				(Math.abs(bSize - preferredSize) + bPenalty)
			);
		})[0];

		const contentUuid = file.content_identity.uuid;
		const url = `${serverUrl}/sidecar/${libraryId}/${contentUuid}/${thumbnail.kind}/${thumbnail.variant}.${thumbnail.format}`;

		return url;
	};

	const thumbnailSrc = getThumbnailUrl(size);

	// Get Spacedrive asset icon (dark theme)
	const kindCapitalized = file.content_identity?.kind
		? file.content_identity.kind.charAt(0).toUpperCase() +
			file.content_identity.kind.slice(1)
		: "Document";

	const icon = getIcon(
		kindCapitalized,
		true, // Dark theme
		file.extension,
		file.kind === "Directory",
	);

	return (
		<div
			className={clsx(
				"relative flex shrink-0 grow-0 items-center justify-center",
				className,
			)}
			style={{
				width: size,
				height: size,
				minWidth: size,
				minHeight: size,
				maxWidth: size,
				maxHeight: size,
			}}
		>
			{/* Always show icon first (instant), then thumbnail loads over it */}
			<img
				src={icon}
				alt=""
				className={clsx(
					"object-contain transition-opacity",
					thumbLoaded && "opacity-0",
				)}
				style={{
					width: iconSize,
					height: iconSize,
					maxWidth: "100%",
					maxHeight: "100%",
				}}
			/>

			{/* Load thumbnail if available */}
			{thumbnailSrc && !thumbError && (
				<img
					src={thumbnailSrc}
					alt={file.name}
					className={clsx(
						"absolute inset-0 m-auto max-h-full max-w-full object-contain transition-opacity",
						// Default frame styling (can be overridden)
						frameClassName ||
							"rounded-lg border border-app-line/50 bg-app-box/30",
						!thumbLoaded && "opacity-0",
					)}
					onLoad={() => setThumbLoaded(true)}
					onError={() => setThumbError(true)}
				/>
			)}

			{/* Thumbstrip scrubber overlay (for videos with thumbstrips) */}
			{isVideo && hasThumbstrip && thumbLoaded && (
				<ThumbstripScrubber 
					file={file} 
					size={size}
					squareMode={false} // Could be passed as prop based on view mode
				/>
			)}
		</div>
	);
});

export function Icon({
	file,
	size = 24,
	className,
}: {
	file: File;
	size?: number;
	className?: string;
}) {
	const kindCapitalized = file.content_identity?.kind
		? file.content_identity.kind.charAt(0).toUpperCase() +
			file.content_identity.kind.slice(1)
		: "Document";

	const icon = getIcon(
		kindCapitalized,
		true, // Dark theme
		file.extension,
		file.kind === "Directory",
	);

	return (
		<img
			src={icon}
			alt=""
			className={className}
			style={{ width: size, height: size }}
		/>
	);
}
