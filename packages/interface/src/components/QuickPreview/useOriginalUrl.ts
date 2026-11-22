import { useEffect, useState } from "react";
import type { File } from "@sd/ts-client";
import { usePlatform } from "../../contexts/PlatformContext";
import { useCoreQuery } from "../../contexts/SpacedriveContext";

/** How long a newly shown file waits before its original is asked for. */
const ORIGINAL_DELAY_MS = 50;

/**
 * The URL a viewer reads a file's original bytes from.
 *
 * A file on this device loads through the asset protocol, which reads the disk
 * directly. A file on another device streams from the daemon's share, which
 * answers the byte ranges a viewer asks for and keeps what it fetched in its
 * block cache, so a video plays and seeks without being downloaded first. A
 * platform without file access reads local files from the share too, through
 * the URL its `shareUrl` rewrites; one with neither gets no URL.
 *
 * The URL arrives shortly after a file is shown, so its thumbnail draws first,
 * and is `null` until then, while `enabled` is false, or when neither way can
 * serve the file.
 */
export function useOriginalUrl(file: File, enabled = true): string | null {
	const platform = usePlatform();
	const fileKey = file.content_identity?.uuid ?? file.id;
	const [readyFor, setReadyFor] = useState<string | null>(null);

	useEffect(() => {
		const timer = setTimeout(() => setReadyFor(fileKey), ORIGINAL_DELAY_MS);
		return () => clearTimeout(timer);
	}, [fileKey]);

	const physicalPath = "Physical" in file.sd_path ? file.sd_path.Physical.path : null;
	const wanted =
		enabled &&
		readyFor === fileKey &&
		physicalPath !== null &&
		(!!platform.convertFileSrc || !!platform.shareUrl);
	const fromDisk = file.is_local && !!platform.convertFileSrc;

	const { data: stream } = useCoreQuery(
		{ type: "files.stream_url", input: { path: file.sd_path } },
		{ enabled: wanted && !fromDisk },
	);

	if (!wanted || physicalPath === null) {
		return null;
	}
	if (fromDisk && platform.convertFileSrc) {
		return platform.convertFileSrc(physicalPath);
	}
	if (!stream) {
		return null;
	}
	return platform.shareUrl ? platform.shareUrl(stream.url) : stream.url;
}
