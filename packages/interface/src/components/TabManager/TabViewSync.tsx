import { useEffect, useRef } from "react";
import { useTabManager } from "./useTabManager";
import { useExplorer } from "../Explorer/context";

/**
 * TabViewSync - Preserves view settings (viewMode, sortBy, gridSize) per tab
 *
 * Saves view state when switching away from a tab,
 * restores it when switching back.
 */
export function TabViewSync() {
	const { activeTabId, saveViewState, getViewState } = useTabManager();
	const { viewMode, sortBy, viewSettings, setViewMode, setSortBy, setViewSettings } =
		useExplorer();

	// Track previous tab to detect switches
	const prevTabIdRef = useRef<string>(activeTabId);
	const isRestoringRef = useRef<boolean>(false);

	// Save view state when switching away from tab
	useEffect(() => {
		const prevTabId = prevTabIdRef.current;

		// Tab changed - save view state for the previous tab
		if (prevTabId !== activeTabId && !isRestoringRef.current) {
			// Preserve existing columnPaths when saving
			const existingState = getViewState(prevTabId);
			saveViewState(prevTabId, {
				viewMode,
				sortBy,
				gridSize: viewSettings.gridSize,
				gapSize: viewSettings.gapSize,
				columnPaths: existingState?.columnPaths,
			});
		}

		prevTabIdRef.current = activeTabId;
	}, [activeTabId, saveViewState, getViewState, viewMode, sortBy, viewSettings.gridSize, viewSettings.gapSize]);

	// Restore view state when tab becomes active
	useEffect(() => {
		const savedState = getViewState(activeTabId);
		if (savedState) {
			isRestoringRef.current = true;

			// Restore view mode
			if (savedState.viewMode !== viewMode) {
				setViewMode(savedState.viewMode as any);
			}

			// Restore sort
			if (savedState.sortBy !== sortBy) {
				setSortBy(savedState.sortBy as any);
			}

			// Restore view settings
			if (
				savedState.gridSize !== viewSettings.gridSize ||
				savedState.gapSize !== viewSettings.gapSize
			) {
				setViewSettings({
					gridSize: savedState.gridSize,
					gapSize: savedState.gapSize,
				});
			}

			// Reset flag after a tick to allow state updates to propagate
			requestAnimationFrame(() => {
				isRestoringRef.current = false;
			});
		}
		// Only run when activeTabId changes
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [activeTabId]);

	return null;
}

