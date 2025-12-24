import { useCallback, useEffect } from "react";
import type { SelectoEvents } from "react-selecto";
import type { File } from "@sd/ts-client";
import { useSelection } from "../../SelectionContext";
import { getElementFileId } from "./utils";

interface UseDragSelectionProps {
	files: File[];
	scrollRef: React.RefObject<HTMLElement>;
}

export function useDragSelection({ files, scrollRef }: UseDragSelectionProps) {
	const { setSelectedFiles, selectedFiles } = useSelection();

	/**
	 * Get file objects from selected DOM elements
	 */
	const getFilesFromElements = useCallback(
		(elements: Element[]): File[] => {
			const fileIds = new Set(
				elements.map((el) => getElementFileId(el)).filter((id): id is string => id !== null)
			);

			return files.filter((file) => fileIds.has(file.id));
		},
		[files]
	);

	/**
	 * Handle selection event from Selecto
	 */
	const handleSelect = useCallback(
		(e: SelectoEvents["select"]) => {
			const selectedElements = e.selected;
			const newSelectedFiles = getFilesFromElements(selectedElements);
			setSelectedFiles(newSelectedFiles);
		},
		[getFilesFromElements, setSelectedFiles]
	);

	/**
	 * Handle scroll during drag selection
	 */
	const handleScroll = useCallback(
		(e: SelectoEvents["scroll"]) => {
			const container = scrollRef.current;
			if (!container) return;

			container.scrollBy(
				(e.direction[0] || 0) * 10,
				(e.direction[1] || 0) * 10
			);
		},
		[scrollRef]
	);

	return {
		handleSelect,
		handleScroll,
		getFilesFromElements,
	};
}
