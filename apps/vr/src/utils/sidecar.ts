/**
 * Utility for building sidecar URLs to fetch thumbnails, thumbstrips, etc.
 * from the daemon's HTTP server.
 *
 * The daemon exposes a `/sidecar/` endpoint at http://localhost:9420
 * that serves generated sidecar files (thumbnails, thumbstrips, transcripts, etc.)
 *
 * Since VR headsets can't reach localhost on the laptop, we proxy these requests
 * through our WebSocket proxy server which forwards them to the daemon's HTTP server.
 */

import { PROXY_HTTP_URL } from "../config";

/**
 * Build a URL to fetch a sidecar file from the daemon
 *
 * @param libraryId - Current library UUID
 * @param contentUuid - Content identity UUID from file.content_identity.uuid
 * @param kind - Sidecar type: "thumb", "thumbstrip", "transcript", etc.
 * @param variant - Size/quality variant: "grid@1x", "detail@2x", etc.
 * @param format - File format: "webp", "jpg", "vtt", etc.
 * @returns Full HTTP URL to the sidecar file
 *
 * @example
 * ```ts
 * const thumbUrl = buildSidecarUrl(
 *   libraryId,
 *   file.content_identity.uuid,
 *   "thumb",
 *   "grid@1x",
 *   "webp"
 * );
 * // Returns: "http://localhost:9420/sidecar/519fd.../abc123.../thumb/grid@1x.webp"
 * ```
 */
export function buildSidecarUrl(
	libraryId: string,
	contentUuid: string,
	kind: string,
	variant: string,
	format: string,
): string {
	return `${PROXY_HTTP_URL}/sidecar/${libraryId}/${contentUuid}/${kind}/${variant}.${format}`;
}

/**
 * Get the best thumbnail URL for a file based on requested size
 *
 * @param file - File object with sidecars array
 * @param libraryId - Current library UUID
 * @param targetSize - Desired thumbnail size in pixels
 * @returns Thumbnail URL, or null if no thumbnails available
 *
 * @example
 * ```ts
 * const thumbUrl = getThumbnailUrl(file, libraryId, 100);
 * if (thumbUrl) {
 *   // Use thumb URL...
 * }
 * ```
 */
export function getThumbnailUrl(
	file: any, // TODO: Import proper File type from @sd/ts-client
	libraryId: string,
	targetSize: number,
): string | null {
	// Need content_identity to build sidecar URL
	if (!file.content_identity?.uuid) {
		return null;
	}

	// Find thumbnail sidecars
	const thumbnails =
		file.sidecars?.filter((s: any) => s.kind === "thumb") || [];

	if (thumbnails.length === 0) {
		return null;
	}

	// Prefer 1x (lower resolution) variants for better performance in VR
	// Only use higher resolution for very large sizes (>400px)
	const preferredSize = targetSize <= 400 ? targetSize * 0.6 : targetSize;

	// Sort thumbnails to find best match
	const thumbnail = thumbnails.sort((a: any, b: any) => {
		// Parse variant (e.g., "grid@1x", "detail@1x") to get size and scale
		const aSize = parseInt(
			a.variant.split("x")[0]?.replace(/\D/g, "") || "0",
		);
		const bSize = parseInt(
			b.variant.split("x")[0]?.replace(/\D/g, "") || "0",
		);

		// Extract scale factor (1x, 2x, 3x)
		const aScaleMatch = a.variant.match(/@(\d+)x/);
		const bScaleMatch = b.variant.match(/@(\d+)x/);
		const aScale = aScaleMatch ? parseInt(aScaleMatch[1]) : 1;
		const bScale = bScaleMatch ? parseInt(bScaleMatch[1]) : 1;

		// Prefer 1x variants (add penalty for higher scales)
		const aPenalty = (aScale - 1) * 100;
		const bPenalty = (bScale - 1) * 100;

		// Find closest match to preferred size, with scale penalty
		return (
			Math.abs(aSize - preferredSize) +
			aPenalty -
			(Math.abs(bSize - preferredSize) + bPenalty)
		);
	})[0];

	if (!thumbnail) {
		return null;
	}

	return buildSidecarUrl(
		libraryId,
		file.content_identity.uuid,
		thumbnail.kind,
		thumbnail.variant,
		thumbnail.format,
	);
}
