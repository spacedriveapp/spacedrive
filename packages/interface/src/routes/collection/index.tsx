import { useLayoutEffect } from "react";
import { useParams } from "react-router-dom";
import { useExplorer } from "../explorer/context";
import { ExplorerView } from "../explorer/ExplorerView";

/**
 * Collection view — files identified at index time (screenshots, screen
 * recordings) rendered through the standard explorer. Activates collection
 * mode so useExplorerFiles fetches from files.collection_listing, and
 * defaults to the media grid since collections are visual by nature.
 */
// Collections that are visual by nature open in the media grid; mixed-kind
// collections (recents) keep the user's current view.
const MEDIA_SLUGS = new Set(["screenshots", "screen-recordings"]);

export function CollectionView() {
	const { slug } = useParams<{ slug: string }>();
	const { enterCollectionMode, exitCollectionMode, viewMode, setViewMode } =
		useExplorer();

	useLayoutEffect(() => {
		if (slug) {
			enterCollectionMode(slug);
		}
		return () => {
			exitCollectionMode();
		};
	}, [slug, enterCollectionMode, exitCollectionMode]);

	// Collections open in the media grid; the user can switch views and the
	// per-surface preference system remembers it from then on.
	useLayoutEffect(() => {
		if (slug && MEDIA_SLUGS.has(slug) && viewMode !== "media") {
			setViewMode("media");
		}
		// Intentionally keyed on slug only: run once per collection visit,
		// not every time the user switches views while here.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [slug]);

	return <ExplorerView />;
}
