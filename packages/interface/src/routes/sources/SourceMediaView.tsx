import { useLayoutEffect } from "react";
import { useExplorer } from "../explorer/context";
import { ExplorerView } from "../explorer/ExplorerView";

/**
 * Media sources (photos) render through the standard explorer: source mode
 * projects the source's records as Files via sources.media_listing, and the
 * real MediaView — date headers, selection, quick preview — does the rest.
 */
export function SourceMediaView({ sourceId }: { sourceId: string }) {
	const { enterSourceMode, exitSourceMode, viewMode, setViewMode } =
		useExplorer();

	useLayoutEffect(() => {
		enterSourceMode(sourceId);
		return () => {
			exitSourceMode();
		};
	}, [sourceId, enterSourceMode, exitSourceMode]);

	useLayoutEffect(() => {
		if (viewMode !== "media") {
			setViewMode("media");
		}
		// Once per source visit; the user's later view switches stick.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [sourceId]);

	return <ExplorerView />;
}
