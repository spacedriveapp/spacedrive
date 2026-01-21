import { memo } from "react";
import { useExplorer } from "../../routes/explorer";
import { useSelection } from "../../routes/explorer/SelectionContext";
import { QuickPreviewFullscreen } from "./QuickPreviewFullscreen";

/**
 * QuickPreviewController - Handles QuickPreview with navigation
 *
 * Isolated component that reads selection state for prev/next navigation.
 * Only re-renders when quickPreviewFileId changes, not on every selection change.
 */
export const QuickPreviewController = memo(function QuickPreviewController({
	sidebarWidth,
	inspectorWidth,
}: {
	sidebarWidth: number;
	inspectorWidth: number;
}) {
	const { quickPreviewFileId, closeQuickPreview, currentFiles } =
		useExplorer();
	const { selectFile } = useSelection();

	// Early return if no preview - this component won't re-render on selection changes
	// because it's memoized and doesn't read selectedFiles directly
	if (!quickPreviewFileId) return null;

	const currentIndex = currentFiles.findIndex(
		(f) => f.id === quickPreviewFileId,
	);
	const hasPrevious = currentIndex > 0;
	const hasNext = currentIndex < currentFiles.length - 1;

	const handleNext = () => {
		if (hasNext && currentFiles[currentIndex + 1]) {
			selectFile(
				currentFiles[currentIndex + 1],
				currentFiles,
				false,
				false,
			);
		}
	};

	const handlePrevious = () => {
		if (hasPrevious && currentFiles[currentIndex - 1]) {
			selectFile(
				currentFiles[currentIndex - 1],
				currentFiles,
				false,
				false,
			);
		}
	};

	return (
		<QuickPreviewFullscreen
			fileId={quickPreviewFileId}
			isOpen={!!quickPreviewFileId}
			onClose={closeQuickPreview}
			onNext={handleNext}
			onPrevious={handlePrevious}
			hasPrevious={hasPrevious}
			hasNext={hasNext}
			sidebarWidth={sidebarWidth}
			inspectorWidth={inspectorWidth}
		/>
	);
});