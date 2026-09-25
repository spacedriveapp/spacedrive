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
 * block cache, so a video plays and seeks without being downloaded first. Both
 * need a client on the daemon's machine, so a platform without file access
 * gets no URL.
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
		enabled && readyFor === fileKey && physicalPath !== null && !!platform.convertFileSrc;

	const { data: stream } = useCoreQuery(
		{ type: "files.stream_url", input: { path: file.sd_path } },
		{ enabled: wanted && !file.is_local },
	);

	if (!wanted || !platform.convertFileSrc || physicalPath === null) {
		return null;
	}
	if (file.is_local) {
		return platform.convertFileSrc(physicalPath);
	}
	return stream?.url ?? null;
}
