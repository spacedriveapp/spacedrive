import { useEffect, useRef } from "react";
import { useCoreMutation, type SdPath } from "@sd/ts-client";
import { useServer } from "../../../contexts/ServerContext";

/** Label identifying this window in the focus rows it publishes. */
const ORIGIN = "spacedrive-explorer";

/**
 * Publish the directory this window is browsing so other windows can follow
 * it. The Photos app is the first subscriber: navigating here re-renders it as
 * the media view of the same folder.
 *
 * A view with no path (search, tags, a collection) publishes null, which tells
 * followers there is nothing to follow rather than leaving them on a stale
 * folder.
 *
 * Focus is presence, so a failed publish is not retried: the next navigation
 * supersedes it, and a follower re-reads the position when it reconnects. It
 * is reported once, because the failure a person actually hits is a daemon too
 * old to know the op, and silence there looks exactly like a follower that is
 * simply not listening.
 */
export function usePublishNavigationFocus(currentPath: SdPath | null): void {
	const { libraryId } = useServer();
	const reported = useRef(false);
	const { mutate } = useCoreMutation("navigation.set_focus", {
		onError: (error) => {
			if (reported.current) return;
			reported.current = true;
			console.warn(
				"Navigation focus is not being published, so other windows cannot follow this one:",
				error,
			);
		},
	});

	// Held in a ref so only a navigation republishes, never an unrelated
	// render of the provider.
	const publish = useRef(mutate);
	publish.current = mutate;

	useEffect(() => {
		publish.current({
			group: null,
			path: currentPath,
			library_id: libraryId,
			origin: ORIGIN,
		});
	}, [currentPath, libraryId]);
}
