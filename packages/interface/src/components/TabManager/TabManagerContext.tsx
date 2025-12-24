import {
	createContext,
	useState,
	useCallback,
	useMemo,
	type ReactNode,
} from "react";
import { createBrowserRouter, type RouteObject } from "react-router-dom";
import type { Router } from "@remix-run/router";

/**
 * Derives a tab title from the current route pathname and search params
 */
function deriveTitleFromPath(pathname: string, search: string): string {
	const routeTitles: Record<string, string> = {
		"/": "Overview",
		"/favorites": "Favorites",
		"/recents": "Recents",
		"/file-kinds": "File Kinds",
		"/search": "Search",
		"/jobs": "Jobs",
		"/daemon": "Daemon",
	};

	if (routeTitles[pathname]) {
		return routeTitles[pathname];
	}

	if (pathname.startsWith("/tag/")) {
		const tagId = pathname.split("/")[2];
		return tagId ? `Tag: ${tagId.slice(0, 8)}...` : "Tag";
	}

	if (pathname === "/explorer" && search) {
		const params = new URLSearchParams(search);

		const view = params.get("view");
		if (view === "device") {
			return "This Device";
		}

		const pathParam = params.get("path");
		if (pathParam) {
			try {
				const sdPath = JSON.parse(decodeURIComponent(pathParam));
				if (sdPath?.Physical?.path) {
					const fullPath = sdPath.Physical.path as string;
					const parts = fullPath.split("/").filter(Boolean);
					return parts[parts.length - 1] || "Explorer";
				}
			} catch {
				// Fall through
			}
		}
		return "Explorer";
	}

	return "Spacedrive";
}

// ============================================================================
// Types
// ============================================================================

export type ViewMode = "grid" | "list" | "column" | "media" | "size";
export type SortBy =
	| "name"
	| "size"
	| "date_modified"
	| "date_created"
	| "kind";

export interface Tab {
	id: string;
	title: string;
	icon: string | null;
	isPinned: boolean;
	lastActive: number;
	savedPath: string;
}

/**
 * All explorer-related state for a single tab.
 * This is the single source of truth - no sync effects needed.
 */
export interface TabExplorerState {
	// View settings
	viewMode: ViewMode;
	sortBy: SortBy;
	gridSize: number;
	gapSize: number;
	foldersFirst: boolean;

	// Column view state (serialized SdPath[] as JSON strings)
	columnStack: string[];

	// Scroll position
	scrollTop: number;
	scrollLeft: number;
}

/** Default explorer state for new tabs */
const DEFAULT_EXPLORER_STATE: TabExplorerState = {
	viewMode: "grid",
	sortBy: "name",
	gridSize: 120,
	gapSize: 16,
	foldersFirst: true,
	columnStack: [],
	scrollTop: 0,
	scrollLeft: 0,
};

// ============================================================================
// Context
// ============================================================================

interface TabManagerContextValue {
	// Tab management
	tabs: Tab[];
	activeTabId: string;
	router: Router;
	createTab: (title?: string, path?: string) => void;
	closeTab: (tabId: string) => void;
	switchTab: (tabId: string) => void;
	updateTabTitle: (tabId: string, title: string) => void;
	updateTabPath: (tabId: string, path: string) => void;
	nextTab: () => void;
	previousTab: () => void;
	selectTabAtIndex: (index: number) => void;
	setDefaultNewTabPath: (path: string) => void;

	// Explorer state (per-tab)
	getExplorerState: (tabId: string) => TabExplorerState;
	updateExplorerState: (
		tabId: string,
		updates: Partial<TabExplorerState>,
	) => void;
}

const TabManagerContext = createContext<TabManagerContextValue | null>(null);

// ============================================================================
// Provider
// ============================================================================

interface TabManagerProviderProps {
	children: ReactNode;
	routes: RouteObject[];
}

export function TabManagerProvider({
	children,
	routes,
}: TabManagerProviderProps) {
	const router = useMemo(() => createBrowserRouter(routes), [routes]);

	const [tabs, setTabs] = useState<Tab[]>(() => {
		const initialTabId = crypto.randomUUID();
		return [
			{
				id: initialTabId,
				title: "Overview",
				icon: null,
				isPinned: false,
				lastActive: Date.now(),
				savedPath: "/",
			},
		];
	});

	const [activeTabId, setActiveTabId] = useState<string>(tabs[0].id);

	// Initialize explorerStates with the first tab's state
	const [explorerStates, setExplorerStates] = useState<
		Map<string, TabExplorerState>
	>(() => {
		const initialMap = new Map<string, TabExplorerState>();
		initialMap.set(tabs[0].id, { ...DEFAULT_EXPLORER_STATE });
		return initialMap;
	});
	const [defaultNewTabPath, setDefaultNewTabPathState] =
		useState<string>("/");

	// ========================================================================
	// Tab management
	// ========================================================================

	const setDefaultNewTabPath = useCallback((path: string) => {
		setDefaultNewTabPathState(path);
	}, []);

	const createTab = useCallback(
		(title?: string, path?: string) => {
			const tabPath = path ?? defaultNewTabPath;
			const [pathname, search = ""] = tabPath.split("?");
			const derivedTitle =
				title ||
				deriveTitleFromPath(pathname, search ? `?${search}` : "");

			const newTab: Tab = {
				id: crypto.randomUUID(),
				title: derivedTitle,
				icon: null,
				isPinned: false,
				lastActive: Date.now(),
				savedPath: tabPath,
			};

			// Initialize explorer state for the new tab
			setExplorerStates((prev) =>
				new Map(prev).set(newTab.id, { ...DEFAULT_EXPLORER_STATE }),
			);

			setTabs((prev) => [...prev, newTab]);
			setActiveTabId(newTab.id);
		},
		[defaultNewTabPath],
	);

	const closeTab = useCallback(
		(tabId: string) => {
			setTabs((prev) => {
				const filtered = prev.filter((t) => t.id !== tabId);

				if (filtered.length === 0) {
					return prev;
				}

				if (tabId === activeTabId) {
					const currentIndex = prev.findIndex((t) => t.id === tabId);
					const newIndex = Math.max(0, currentIndex - 1);
					const newActiveTab = filtered[newIndex] || filtered[0];
					if (newActiveTab) {
						setActiveTabId(newActiveTab.id);
					}
				}

				return filtered;
			});

			// Clean up explorer state for closed tab
			setExplorerStates((prev) => {
				const next = new Map(prev);
				next.delete(tabId);
				return next;
			});
		},
		[activeTabId],
	);

	const switchTab = useCallback(
		(newTabId: string) => {
			if (newTabId === activeTabId) return;

			setTabs((prev) =>
				prev.map((tab) =>
					tab.id === newTabId
						? { ...tab, lastActive: Date.now() }
						: tab,
				),
			);

			setActiveTabId(newTabId);
		},
		[activeTabId],
	);

	const updateTabTitle = useCallback((tabId: string, title: string) => {
		setTabs((prev) =>
			prev.map((tab) => (tab.id === tabId ? { ...tab, title } : tab)),
		);
	}, []);

	const updateTabPath = useCallback((tabId: string, path: string) => {
		setTabs((prev) =>
			prev.map((tab) =>
				tab.id === tabId ? { ...tab, savedPath: path } : tab,
			),
		);
	}, []);

	const nextTab = useCallback(() => {
		const currentIndex = tabs.findIndex((t) => t.id === activeTabId);
		const nextIndex = (currentIndex + 1) % tabs.length;
		switchTab(tabs[nextIndex].id);
	}, [tabs, activeTabId, switchTab]);

	const previousTab = useCallback(() => {
		const currentIndex = tabs.findIndex((t) => t.id === activeTabId);
		const prevIndex = (currentIndex - 1 + tabs.length) % tabs.length;
		switchTab(tabs[prevIndex].id);
	}, [tabs, activeTabId, switchTab]);

	const selectTabAtIndex = useCallback(
		(index: number) => {
			if (index >= 0 && index < tabs.length) {
				switchTab(tabs[index].id);
			}
		},
		[tabs, switchTab],
	);

	// ========================================================================
	// Explorer state (per-tab)
	// ========================================================================

	const getExplorerState = useCallback(
		(tabId: string): TabExplorerState => {
			return explorerStates.get(tabId) ?? { ...DEFAULT_EXPLORER_STATE };
		},
		[explorerStates],
	);

	const updateExplorerState = useCallback(
		(tabId: string, updates: Partial<TabExplorerState>) => {
			setExplorerStates((prev) => {
				const current = prev.get(tabId) ?? {
					...DEFAULT_EXPLORER_STATE,
				};
				return new Map(prev).set(tabId, { ...current, ...updates });
			});
		},
		[],
	);

	// ========================================================================
	// Context value
	// ========================================================================

	const value = useMemo<TabManagerContextValue>(
		() => ({
			tabs,
			activeTabId,
			router,
			createTab,
			closeTab,
			switchTab,
			updateTabTitle,
			updateTabPath,
			nextTab,
			previousTab,
			selectTabAtIndex,
			setDefaultNewTabPath,
			getExplorerState,
			updateExplorerState,
		}),
		[
			tabs,
			activeTabId,
			router,
			createTab,
			closeTab,
			switchTab,
			updateTabTitle,
			updateTabPath,
			nextTab,
			previousTab,
			selectTabAtIndex,
			setDefaultNewTabPath,
			getExplorerState,
			updateExplorerState,
		],
	);

	return (
		<TabManagerContext.Provider value={value}>
			{children}
		</TabManagerContext.Provider>
	);
}

export { TabManagerContext };
