import { useDraggable } from "@dnd-kit/core";
import type { File } from "@sd/ts-client";

interface UseDraggableFileProps {
	file: File;
	selectedFiles?: File[];
	gridSize?: number;
}

/**
 * Wrapper around useDraggable that filters out right-clicks (including Ctrl+Click on macOS)
 * to prevent drag from starting when opening context menus
 */
export function useDraggableFile({ file, selectedFiles, gridSize }: UseDraggableFileProps) {
	const { attributes, listeners, setNodeRef, isDragging } = useDraggable({
		id: file.id,
		data: {
			type: "explorer-file",
			sdPath: file.sd_path,
			name: file.name,
			file: file,
			gridSize,
			selectedFiles,
		},
	});

	// Filter listeners to prevent drag on right-click
	const filteredListeners = listeners
		? {
				...listeners,
				onPointerDown: (e: React.PointerEvent) => {
					// Block right-click (button 2) OR control+click (macOS right-click)
					if (e.button === 2 || (e.button === 0 && e.ctrlKey)) {
						e.preventDefault();
						e.stopPropagation();
						return;
					}
					// Call original listener for normal left-click
					listeners.onPointerDown?.(e);
				},
		  }
		: undefined;

	return {
		attributes,
		listeners: filteredListeners,
		setNodeRef,
		isDragging,
	};
}
