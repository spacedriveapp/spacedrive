import { useEffect } from "react";
import { useExplorer } from "../Explorer";
import { useSelection } from "../Explorer/SelectionContext";

/**
 * QuickPreviewSyncer - Syncs selection changes to QuickPreview
 *
 * Isolated component so selection changes only re-render this tiny component,
 * not the entire layout. When selection changes while QuickPreview is open,
 * we update the preview to show the newly selected file.
 */
export function QuickPreviewSyncer() {
	const { quickPreviewFileId, openQuickPreview } = useExplorer();
	const { selectedFiles } = useSelection();

	useEffect(() => {
		if (!quickPreviewFileId) return;

		// When selection changes and QuickPreview is open, update preview to match selection
		if (
			selectedFiles.length === 1 &&
			selectedFiles[0].id !== quickPreviewFileId
		) {
			openQuickPreview(selectedFiles[0].id);
		}
	}, [selectedFiles, quickPreviewFileId, openQuickPreview]);

	return null;
}
