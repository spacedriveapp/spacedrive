import { useRef, useEffect, type ReactNode } from "react";
import Selecto from "react-selecto";
import type { File } from "@sd/ts-client";
import { useSelection } from "../../SelectionContext";
import { SELECTABLE_DATA_ATTRIBUTE } from "../../components/DragSelect/utils";

interface DragSelectProps {
	children: ReactNode;
	files: File[];
	scrollRef: React.RefObject<HTMLElement>;
}

const CHROME_REGEX = /Chrome/;

export function DragSelect({ children, files, scrollRef }: DragSelectProps) {
	const selectoRef = useRef<Selecto>(null);
	const { setSelectedFiles, selectedFiles } = useSelection();
	const isDragSelecting = useRef(false);

	const isChrome = CHROME_REGEX.test(navigator.userAgent);
	const isWindows = navigator.platform.toLowerCase().includes("win");

	// Get file from element
	const getFileFromElement = (element: Element): File | null => {
		const fileId = element.getAttribute("data-file-id");
		if (!fileId) return null;
		return files.find((f) => f.id === fileId) || null;
	};

	// Get files from elements
	const getFilesFromElements = (elements: Element[]): File[] => {
		const fileIds = new Set(
			elements
				.map((el) => el.getAttribute("data-file-id"))
				.filter((id): id is string => id !== null)
		);
		return files.filter((file) => fileIds.has(file.id));
	};

	// Handle scroll during drag selection
	useEffect(() => {
		const container = scrollRef.current;
		if (!container) return;

		const handleScroll = () => {
			selectoRef.current?.checkScroll();
			selectoRef.current?.findSelectableTargets();
		};

		container.addEventListener("scroll", handleScroll);
		return () => container.removeEventListener("scroll", handleScroll);
	}, [scrollRef]);

	return (
		<>
			<Selecto
				ref={selectoRef}
				dragContainer={scrollRef.current || undefined}
				selectableTargets={[`[${SELECTABLE_DATA_ATTRIBUTE}]`]}
				selectByClick={false}
				selectFromInside={false}
				continueSelect={false}
				continueSelectWithoutDeselect={false}
				toggleContinueSelect={[["shift"], [isWindows ? "ctrl" : "meta"]]}
				toggleContinueSelectWithoutDeselect={false}
				hitRate={0}
				ratio={0}
				dragCondition={(e) => {
					// Prevent drag selection from starting if clicking on a selected item
					// This allows dnd-kit drag-and-drop to work without interference
					const target = e.inputEvent.target as Element;
					const clickedElement = target.closest(`[${SELECTABLE_DATA_ATTRIBUTE}]`);
					
					if (clickedElement) {
						const file = getFileFromElement(clickedElement);
						const isAlreadySelected = file && selectedFiles.some((f) => f.id === file.id);
						const hasModifiers =
							e.inputEvent.shiftKey ||
							(e.inputEvent as MouseEvent).metaKey ||
							(e.inputEvent as MouseEvent).ctrlKey;
						
						// Don't start drag selection if clicking a selected item without modifiers
						if (isAlreadySelected && !hasModifiers) {
							return false;
						}
					}
					
					return true;
				}}
				scrollOptions={{
					container: scrollRef.current || undefined,
					throttleTime: isChrome ? 30 : 10000,
					threshold: 0,
				}}
				onDragStart={(e) => {
					isDragSelecting.current = true;
				}}
				onSelect={(e) => {
					const inputEvent = e.inputEvent as MouseEvent;
					const isContinueSelect =
						inputEvent.shiftKey || (isWindows ? inputEvent.ctrlKey : inputEvent.metaKey);

					// Handle selection
					if (inputEvent.type === "mousedown" || inputEvent.type === "touchstart") {
						// Single click handling
						if (!isDragSelecting.current || e.selected.length <= 1) {
							return; // Let normal click handlers deal with it
						}
					}

					// Handle drag selection
					if (inputEvent.type === "mousemove" || inputEvent.type === "touchmove") {
						const selectedElements = e.selected;
						const newSelectedFiles = getFilesFromElements(selectedElements);

						if (isContinueSelect) {
							// Add to existing selection
							const existingIds = new Set(selectedFiles.map((f) => f.id));
							const combined = [...selectedFiles];

							for (const file of newSelectedFiles) {
								if (!existingIds.has(file.id)) {
									combined.push(file);
								}
							}

							setSelectedFiles(combined);
						} else {
							// Replace selection
							setSelectedFiles(newSelectedFiles);
						}
					}
				}}
				onSelectEnd={() => {
					isDragSelecting.current = false;
				}}
				onScroll={(e) => {
					const container = scrollRef.current;
					if (!container) return;

					container.scrollBy(
						(e.direction[0] || 0) * 10,
						(e.direction[1] || 0) * 10
					);
				}}
			/>
			{children}
		</>
	);
}
