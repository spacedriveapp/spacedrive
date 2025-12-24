import { useRef, useEffect } from "react";
import { useLocation } from "react-router-dom";
import { useExplorer } from "./context";
import { useTabManager } from "../TabManager";

interface TabNavigationGuardProps {
	children: React.ReactNode;
	fallback?: React.ReactNode;
}

/**
 * TabNavigationGuard prevents rendering stale data during tab switches.
 *
 * When switching tabs, the activeTabId updates immediately but URL navigation
 * is async. This creates a brief window where the new tab's UI would render
 * the old tab's data. The guard blocks rendering ONLY during this window.
 *
 * Regular in-tab navigation (sidebar, breadcrumbs) is NOT blocked.
 */
export function TabNavigationGuard({
	children,
	fallback,
}: TabNavigationGuardProps) {
	const { activeTabId } = useExplorer();
	const { tabs } = useTabManager();
	const location = useLocation();

	// Track when we last switched tabs
	const lastTabIdRef = useRef(activeTabId);
	const tabSwitchedAtRef = useRef<number>(0);

	const activeTab = tabs.find((t) => t.id === activeTabId);
	const currentUrlPath = location.pathname + location.search;

	// Detect tab switch and record timestamp
	if (lastTabIdRef.current !== activeTabId) {
		lastTabIdRef.current = activeTabId;
		tabSwitchedAtRef.current = Date.now();
	}

	// Check if we just switched tabs (within last 50ms)
	const justSwitchedTabs = Date.now() - tabSwitchedAtRef.current < 50;

	// Only block if we JUST switched tabs AND URL hasn't caught up yet
	const isNavigating =
		justSwitchedTabs && activeTab && currentUrlPath !== activeTab.savedPath;

	if (isNavigating) {
		return fallback ?? <div className="h-full overflow-auto" />;
	}

	return <>{children}</>;
}
