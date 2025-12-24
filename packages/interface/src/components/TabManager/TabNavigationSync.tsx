import { useEffect, useRef } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { useTabManager } from "./useTabManager";

/**
 * TabNavigationSync - Syncs router navigation with active tab
 *
 * This component runs inside the router context and:
 * 1. Saves the current location to the active tab when navigation occurs
 * 2. Navigates to the saved location when switching to a different tab
 */
export function TabNavigationSync() {
	const location = useLocation();
	const navigate = useNavigate();
	const { activeTabId, tabs, updateTabPath } = useTabManager();

	const activeTab = tabs.find((t) => t.id === activeTabId);
	const currentPath = location.pathname + location.search;

	// Track previous activeTabId to detect tab switches
	const prevActiveTabIdRef = useRef(activeTabId);

	// Save current location to active tab (only for in-tab navigation)
	useEffect(() => {
		// Skip saving during tab switch - currentPath belongs to the old tab
		if (prevActiveTabIdRef.current !== activeTabId) {
			prevActiveTabIdRef.current = activeTabId;
			return;
		}

		if (activeTab && currentPath !== activeTab.savedPath) {
			updateTabPath(activeTabId, currentPath);
		}
	}, [currentPath, activeTab, activeTabId, updateTabPath]);

	// Navigate to saved location when switching tabs
	useEffect(() => {
		if (activeTab && currentPath !== activeTab.savedPath) {
			navigate(activeTab.savedPath, { replace: true });
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [activeTabId]);

	return null;
}
