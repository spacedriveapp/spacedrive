import { useEffect, useRef } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { useTabManager } from "./useTabManager";

/**
 * Derives a tab title from the current route pathname and search params
 */
function deriveTitleFromPath(pathname: string, search: string): string {
	// Static route mappings
	const routeTitles: Record<string, string> = {
		"/": "Overview",
		"/favorites": "Favorites",
		"/recents": "Recents",
		"/file-kinds": "File Kinds",
		"/search": "Search",
		"/jobs": "Jobs",
		"/daemon": "Daemon",
	};

	// Check static routes first
	if (routeTitles[pathname]) {
		return routeTitles[pathname];
	}

	// Handle tag routes: /tag/:tagId
	if (pathname.startsWith("/tag/")) {
		const tagId = pathname.split("/")[2];
		return tagId ? `Tag: ${tagId.slice(0, 8)}...` : "Tag";
	}

	// Handle explorer routes
	if (pathname === "/explorer" && search) {
		const params = new URLSearchParams(search);

		// Handle virtual views: /explorer?view=device&id=abc123
		const view = params.get("view");
		if (view === "device") {
			return "This Device";
		}

		// Handle path-based navigation
		const pathParam = params.get("path");
		if (pathParam) {
			try {
				const sdPath = JSON.parse(decodeURIComponent(pathParam));
				// Extract the last component of the path for the title
				if (sdPath?.Physical?.path) {
					const fullPath = sdPath.Physical.path as string;
					const parts = fullPath.split("/").filter(Boolean);
					return parts[parts.length - 1] || "Explorer";
				}
			} catch {
				// Fall through to default
			}
		}
		return "Explorer";
	}

	// Default fallback
	return "Spacedrive";
}

/**
 * TabNavigationSync - Syncs router navigation with active tab
 *
 * This component runs inside the router context and:
 * 1. Saves the current location to the active tab when navigation occurs
 * 2. Updates the tab title based on the current route
 * 3. Navigates to the saved location when switching to a different tab
 */
export function TabNavigationSync() {
	const location = useLocation();
	const navigate = useNavigate();
	const { activeTabId, tabs, updateTabPath, updateTabTitle } = useTabManager();

	const activeTab = tabs.find((t) => t.id === activeTabId);
	const currentPath = location.pathname + location.search;

	// Track previous activeTabId to detect tab switches
	const prevActiveTabIdRef = useRef(activeTabId);

	// Save current location and update title for active tab (only for in-tab navigation)
	useEffect(() => {
		// Skip saving during tab switch - currentPath belongs to the old tab
		if (prevActiveTabIdRef.current !== activeTabId) {
			prevActiveTabIdRef.current = activeTabId;
			return;
		}

		if (activeTab && currentPath !== activeTab.savedPath) {
			updateTabPath(activeTabId, currentPath);
		}

		// Always update title based on current location
		const newTitle = deriveTitleFromPath(location.pathname, location.search);
		if (activeTab && newTitle !== activeTab.title) {
			updateTabTitle(activeTabId, newTitle);
		}
	}, [currentPath, activeTab, activeTabId, updateTabPath, updateTabTitle, location.pathname, location.search]);

	// Navigate to saved location when switching tabs
	useEffect(() => {
		if (activeTab && currentPath !== activeTab.savedPath) {
			navigate(activeTab.savedPath, { replace: true });
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [activeTabId]);

	return null;
}
