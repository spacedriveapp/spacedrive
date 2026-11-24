import { useEffect, type RefObject } from "react";

/**
 * Seeks a media element to `startSeconds` once its metadata is known.
 *
 * Keyed on the file and the time rather than the source URL, so a player that
 * swaps to another rendition of the same file keeps its own position instead
 * of jumping back to the start time.
 */
export function useStartSeconds(
	ref: RefObject<HTMLMediaElement | null>,
	fileId: string,
	startSeconds: number | undefined,
) {
	useEffect(() => {
		const media = ref.current;
		if (!media || startSeconds === undefined) return;
		const seek = () => {
			media.currentTime = startSeconds;
		};
		if (media.readyState >= HTMLMediaElement.HAVE_METADATA) {
			seek();
			return;
		}
		media.addEventListener("loadedmetadata", seek, { once: true });
		return () => media.removeEventListener("loadedmetadata", seek);
	}, [ref, fileId, startSeconds]);
}
