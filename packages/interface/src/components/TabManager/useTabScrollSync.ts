import { useEffect, useRef, useCallback, type RefObject } from "react";
import { useTabManager } from "./useTabManager";
import { useExplorer } from "../Explorer/context";

/**
 * useTabScrollSync - Preserves scroll position per tab
 *
 * Saves scroll position continuously while scrolling,
 * restores it when switching back to a tab (after content loads).
 *
 * @param scrollRef - Ref to the scrollable container element
 */
export function useTabScrollSync(scrollRef: RefObject<HTMLElement | null>) {
	const { activeTabId, saveScrollState, getScrollState } = useTabManager();
	const { viewMode } = useExplorer();

	// Track if we've restored for this tab+path combination
	const restoredKeyRef = useRef<string>("");
	const lastSavedRef = useRef<{ top: number; left: number }>({
		top: 0,
		left: 0,
	});

	// Create a stable key for the current tab+path+viewMode
	const stateKey = `${activeTabId}:${viewMode}`;

	// Save scroll position on every scroll event
	const handleScroll = useCallback(() => {
		const element = scrollRef.current;
		if (!element) return;

		// Debounce by checking if position actually changed
		const { scrollTop, scrollLeft } = element;
		if (
			lastSavedRef.current.top === scrollTop &&
			lastSavedRef.current.left === scrollLeft
		) {
			return;
		}

		lastSavedRef.current = { top: scrollTop, left: scrollLeft };
		saveScrollState(activeTabId, {
			viewMode,
			scrollTop,
			scrollLeft,
			virtualOffset: 0,
		});
	}, [activeTabId, saveScrollState, scrollRef, viewMode]);

	// Attach scroll listener
	useEffect(() => {
		const element = scrollRef.current;
		if (!element) return;

		element.addEventListener("scroll", handleScroll, { passive: true });
		return () => {
			element.removeEventListener("scroll", handleScroll);
		};
	}, [handleScroll, scrollRef]);

	// Restore scroll position when tab/path changes
	useEffect(() => {
		// Already restored for this key
		if (restoredKeyRef.current === stateKey) return;

		const savedState = getScrollState(activeTabId);
		if (!savedState || savedState.viewMode !== viewMode) {
			restoredKeyRef.current = stateKey;
			return;
		}

		// Try to restore with increasing delays to handle async content loading
		const tryRestore = (attempt: number) => {
			const element = scrollRef.current;
			if (!element) return;

			// Check if content has loaded (scrollHeight > clientHeight means there's scrollable content)
			const hasContent = element.scrollHeight > element.clientHeight + 50;

			if (hasContent || attempt >= 5) {
				element.scrollTop = savedState.scrollTop;
				element.scrollLeft = savedState.scrollLeft;
				lastSavedRef.current = {
					top: savedState.scrollTop,
					left: savedState.scrollLeft,
				};
				restoredKeyRef.current = stateKey;
			} else {
				// Content not ready, try again
				setTimeout(() => tryRestore(attempt + 1), 50 * (attempt + 1));
			}
		};

		// Start restore attempts after a brief delay
		const timeoutId = setTimeout(() => tryRestore(0), 50);
		return () => clearTimeout(timeoutId);
	}, [activeTabId, getScrollState, scrollRef, stateKey, viewMode]);
}
