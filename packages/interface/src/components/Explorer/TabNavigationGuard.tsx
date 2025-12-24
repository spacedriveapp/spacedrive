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
 * the old tab's data. The guard blocks rendering until navigation completes.
 */
export function TabNavigationGuard({
	children,
	fallback,
}: TabNavigationGuardProps) {
	const { activeTabId } = useExplorer();
	const { tabs } = useTabManager();
	const location = useLocation();

	const activeTab = tabs.find((t) => t.id === activeTabId);
	const currentUrlPath = location.pathname + location.search;

	// If URL doesn't match the tab's savedPath, navigation is in progress
	const isNavigating = activeTab && currentUrlPath !== activeTab.savedPath;

	if (isNavigating) {
		return fallback ?? <div className="h-full overflow-auto" />;
	}

	return <>{children}</>;
}
