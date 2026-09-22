import { useEffect, useRef } from "react";
import { useCoreMutation } from "@sd/ts-client";
import { useServer } from "../../../contexts/ServerContext";
import { useExplorer } from "../context";
import { useSearchInput } from "./useExplorerFiles";

/** Label identifying this window in the focus rows it publishes. */
const ORIGIN = "spacedrive-explorer";

/**
 * Publish where this window is looking so other windows can follow it: the
 * directory it is browsing, and the search it is running when it runs one,
 * exactly as it sends it. The Photos app is the first subscriber: it renders
 * the media in the folder, or in the search's results.
 *
 * A view with no path (tags, a collection) publishes a null path, which tells
 * followers there is no folder to follow rather than leaving them on a stale
 * one.
 *
 * Focus is presence, so a failed publish is not retried: the next navigation
 * supersedes it, and a follower re-reads the position when it reconnects. It
 * is reported once, because the failure a person actually hits is a daemon too
 * old to know the op, and silence there looks exactly like a follower that is
 * simply not listening.
 */
function usePublishNavigationFocus(): void {
	const { currentPath } = useExplorer();
	const search = useSearchInput();
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

	// Held in a ref so only a navigation or a search republishes, never an
	// unrelated render.
	const publish = useRef(mutate);
	publish.current = mutate;

	useEffect(() => {
		publish.current({
			group: null,
			path: currentPath,
			search,
			library_id: libraryId,
			origin: ORIGIN,
		});
	}, [currentPath, search, libraryId]);
}

/** Publishes the explorer's navigation focus. Renders nothing; it sits inside
 * the explorer's provider, which is where what it publishes is read from. */
export function NavigationFocusPublisher(): null {
	usePublishNavigationFocus();
	return null;
}
