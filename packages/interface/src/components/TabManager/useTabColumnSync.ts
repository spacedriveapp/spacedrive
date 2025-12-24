import { useEffect, useRef, useCallback } from "react";
import { useTabManager } from "./useTabManager";
import type { SdPath } from "@sd/ts-client";

/**
 * useTabColumnSync - Preserves column drill-down state per tab
 *
 * Saves the column stack when switching away from a tab in ColumnView,
 * restores it when switching back.
 *
 * @returns Object with savedColumnPaths and saveColumnPaths function
 */
export function useTabColumnSync() {
	const { activeTabId, saveViewState, getViewState } = useTabManager();

	// Track previous tab to detect switches
	const prevTabIdRef = useRef<string>(activeTabId);
	const currentColumnPathsRef = useRef<string[]>([]);

	// Get saved column paths for current tab
	const savedState = getViewState(activeTabId);
	const savedColumnPaths = savedState?.columnPaths;

	// Save column paths (called by ColumnView when columnStack changes)
	const saveColumnPaths = useCallback(
		(paths: SdPath[]) => {
			// Convert SdPaths to string keys for storage
			const pathStrings = paths.map((p) => {
				if ("Physical" in p) {
					return `${p.Physical.device_slug}:${p.Physical.path}`;
				}
				return JSON.stringify(p);
			});

			currentColumnPathsRef.current = pathStrings;

			// Get current view state and merge with column paths
			const currentState = getViewState(activeTabId);
			saveViewState(activeTabId, {
				viewMode: currentState?.viewMode ?? "column",
				sortBy: currentState?.sortBy ?? "name",
				gridSize: currentState?.gridSize ?? 120,
				gapSize: currentState?.gapSize ?? 16,
				columnPaths: pathStrings,
			});
		},
		[activeTabId, getViewState, saveViewState],
	);

	// Parse saved column paths back to SdPaths
	const parseSavedPaths = useCallback((): SdPath[] | null => {
		if (!savedColumnPaths || savedColumnPaths.length === 0) return null;

		return savedColumnPaths.map((pathStr) => {
			if (pathStr.includes(":")) {
				const [deviceSlug, ...pathParts] = pathStr.split(":");
				return {
					Physical: {
						device_slug: deviceSlug,
						path: pathParts.join(":"),
					},
				};
			}
			try {
				return JSON.parse(pathStr);
			} catch {
				return { Physical: { device_slug: "unknown", path: pathStr } };
			}
		});
	}, [savedColumnPaths]);

	// Detect tab switch and clear ref
	useEffect(() => {
		if (prevTabIdRef.current !== activeTabId) {
			currentColumnPathsRef.current = [];
			prevTabIdRef.current = activeTabId;
		}
	}, [activeTabId]);

	return {
		savedColumnPaths: parseSavedPaths(),
		saveColumnPaths,
		activeTabId,
	};
}

