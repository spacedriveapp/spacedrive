import type { File, MediaSidecarsOutput } from "@sd/ts-client";
import { useServer } from "../../contexts/ServerContext";
import { useLibraryQuery } from "../../contexts/SpacedriveContext";

/**
 * The transcript, proxy and shot list of the file a viewer shows.
 *
 * Media sidecars are keyed by content, and the daemon reads samples of the
 * file to find that key, so they are asked for per previewed file rather than
 * carried on every row of a listing. A file on another device has none here.
 */
export function useMediaSidecars(file: File): MediaSidecarsOutput | null {
	const { data } = useLibraryQuery(
		{ type: "media.sidecars", input: { path: file.sd_path } },
		{ enabled: file.is_local },
	);
	return data ?? null;
}

/** The URL of the file's proxy, or null when none has been made. */
export function useProxyUrl(
	sidecars: MediaSidecarsOutput | null,
): string | null {
	const { buildSidecarUrl } = useServer();
	if (!sidecars?.proxy) return null;
	return buildSidecarUrl(
		sidecars.content_uuid,
		"proxy",
		sidecars.proxy.variant,
		sidecars.proxy.format,
		sidecars.proxy.version,
	);
}
