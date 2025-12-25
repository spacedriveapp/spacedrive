import {
	createContext,
	useContext,
	useReducer,
	useMemo,
	useEffect,
	useCallback,
	type ReactNode,
} from "react";
import { useNavigate, useLocation } from "react-router-dom";
import { useNormalizedQuery } from "../../context";
import { useTabManager } from "../TabManager/useTabManager";
import type {
	ViewMode as TabViewMode,
	SortBy as TabSortBy,
} from "../TabManager/TabManagerContext";

import type {
	SdPath,
	File,
	Device,
	ListLibraryDevicesInput,
	DirectorySortBy,
	MediaSortBy,
} from "@sd/ts-client";
import {
	useViewPreferencesStore,
	useSortPreferencesStore,
} from "@sd/ts-client";

export type SortBy = DirectorySortBy | MediaSortBy;
export type ViewMode =
	| "grid"
	| "list"
	| "media"
	| "column"
	| "size"
	| "knowledge";

export interface ViewSettings {
	gridSize: number;
	gapSize: number;
	showFileSize: boolean;
	columnWidth: number;
	foldersFirst: boolean;
}

export type NavigationTarget =
	| { type: "path"; path: SdPath }
	| {
			type: "view";
			view: string;
			id?: string;
			params?: Record<string, string>;
	  };

function targetToKey(target: NavigationTarget): string {
	if (target.type === "path") {
		const p = target.path;
		if ("Physical" in p && p.Physical) {
			return `path:${p.Physical.device_slug}:${p.Physical.path}`;
		}
		if ("Virtual" in p && p.Virtual) {
			return `path:virtual:${p.Virtual}`;
		}
		return `path:${JSON.stringify(p)}`;
	}
	return `view:${target.view}:${target.id || ""}`;
}

function targetsEqual(
	a: NavigationTarget | null,
	b: NavigationTarget | null,
): boolean {
	if (a === null || b === null) return a === b;
	return targetToKey(a) === targetToKey(b);
}

const MAX_HISTORY_SIZE = 100;

interface NavigationState {
	history: NavigationTarget[];
	index: number;
}

type NavigationAction =
	| { type: "NAVIGATE"; target: NavigationTarget }
	| { type: "GO_BACK" }
	| { type: "GO_FORWARD" }
	| { type: "SYNC"; target: NavigationTarget };

function navigationReducer(
	state: NavigationState,
	action: NavigationAction,
): NavigationState {
	switch (action.type) {
		case "NAVIGATE": {
			const current = state.history[state.index];
			if (current && targetsEqual(current, action.target)) {
				return state;
			}

			const newHistory = state.history.slice(0, state.index + 1);
			newHistory.push(action.target);

			const trimmedHistory = newHistory.slice(-MAX_HISTORY_SIZE);
			const indexAdjustment = newHistory.length - trimmedHistory.length;

			return {
				history: trimmedHistory,
				index: state.index + 1 - indexAdjustment,
			};
		}

		case "GO_BACK": {
			if (state.index <= 0) return state;
			return { ...state, index: state.index - 1 };
		}

		case "GO_FORWARD": {
			if (state.index >= state.history.length - 1) return state;
			return { ...state, index: state.index + 1 };
		}

		case "SYNC": {
			const current = state.history[state.index];
			if (current && targetsEqual(current, action.target)) {
				return state;
			}

			const newHistory = [
				...state.history.slice(0, state.index + 1),
				action.target,
			];
			const trimmedHistory = newHistory.slice(-MAX_HISTORY_SIZE);
			const indexAdjustment = newHistory.length - trimmedHistory.length;

			return {
				history: trimmedHistory,
				index: state.index + 1 - indexAdjustment,
			};
		}

		default:
			return state;
	}
}

const initialNavigationState: NavigationState = {
	history: [],
	index: -1,
};

interface UIState {
	viewMode: ViewMode;
	sortBy: SortBy;
	viewSettings: ViewSettings;
	sidebarVisible: boolean;
	inspectorVisible: boolean;
	quickPreviewFileId: string | null;
	tagModeActive: boolean;
}

type UIAction =
	| { type: "SET_VIEW_MODE"; mode: ViewMode }
	| { type: "SET_SORT_BY"; sort: SortBy }
	| { type: "SET_VIEW_SETTINGS"; settings: Partial<ViewSettings> }
	| { type: "SET_SIDEBAR_VISIBLE"; visible: boolean }
	| { type: "SET_INSPECTOR_VISIBLE"; visible: boolean }
	| { type: "SET_QUICK_PREVIEW"; fileId: string | null }
	| { type: "SET_TAG_MODE"; active: boolean }
	| {
			type: "LOAD_PREFERENCES";
			viewMode: ViewMode;
			viewSettings?: Partial<ViewSettings>;
	  };

const defaultViewSettings: ViewSettings = {
	gridSize: 120,
	gapSize: 16,
	showFileSize: true,
	columnWidth: 256,
	foldersFirst: false,
};

function uiReducer(state: UIState, action: UIAction): UIState {
	switch (action.type) {
		case "SET_VIEW_MODE":
			return { ...state, viewMode: action.mode };

		case "SET_SORT_BY":
			return { ...state, sortBy: action.sort };

		case "SET_VIEW_SETTINGS":
			return {
				...state,
				viewSettings: { ...state.viewSettings, ...action.settings },
			};

		case "SET_SIDEBAR_VISIBLE":
			return { ...state, sidebarVisible: action.visible };

		case "SET_INSPECTOR_VISIBLE":
			return { ...state, inspectorVisible: action.visible };

		case "SET_QUICK_PREVIEW":
			return { ...state, quickPreviewFileId: action.fileId };

		case "SET_TAG_MODE":
			return { ...state, tagModeActive: action.active };

		case "LOAD_PREFERENCES":
			return {
				...state,
				viewMode: action.viewMode,
				viewSettings: action.viewSettings
					? { ...state.viewSettings, ...action.viewSettings }
					: state.viewSettings,
			};

		default:
			return state;
	}
}

const initialUIState: UIState = {
	viewMode: "grid",
	sortBy: "name",
	viewSettings: defaultViewSettings,
	sidebarVisible: true,
	inspectorVisible: true,
	quickPreviewFileId: null,
	tagModeActive: false,
};

function targetToUrl(target: NavigationTarget): string {
	if (target.type === "path") {
		const encoded = encodeURIComponent(JSON.stringify(target.path));
		return `/explorer?path=${encoded}`;
	}

	const params = new URLSearchParams({ view: target.view });
	if (target.id) params.set("id", target.id);
	if (target.params) {
		Object.entries(target.params).forEach(([k, v]) => params.set(k, v));
	}
	return `/explorer?${params.toString()}`;
}

function urlToTarget(search: string): NavigationTarget | null {
	const params = new URLSearchParams(search);

	const pathParam = params.get("path");
	if (pathParam) {
		try {
			const path = JSON.parse(decodeURIComponent(pathParam)) as SdPath;
			return { type: "path", path };
		} catch {
			return null;
		}
	}

	const view = params.get("view");
	if (view) {
		const id = params.get("id") || undefined;
		const extraParams: Record<string, string> = {};
		params.forEach((v, k) => {
			if (k !== "view" && k !== "id") extraParams[k] = v;
		});
		return {
			type: "view",
			view,
			id,
			params:
				Object.keys(extraParams).length > 0 ? extraParams : undefined,
		};
	}

	return null;
}

function getSpaceItemKey(pathname: string, search: string): string {
	if (pathname === "/") return "overview";
	if (pathname === "/recents") return "recents";
	if (pathname === "/favorites") return "favorites";
	if (pathname === "/file-kinds") return "file-kinds";
	if (pathname.startsWith("/tag/")) return `tag:${pathname.slice(5)}`;
	if (pathname === "/explorer" && search) return `explorer:${search}`;
	return pathname;
}

function getPathKey(target: NavigationTarget | null): string {
	if (!target) return "null";
	return targetToKey(target);
}

interface ExplorerContextValue {
	currentTarget: NavigationTarget | null;
	currentPath: SdPath | null;
	currentView: {
		view: string;
		id?: string;
		params?: Record<string, string>;
	} | null;

	navigateToPath: (path: SdPath) => void;
	navigateToView: (
		view: string,
		id?: string,
		params?: Record<string, string>,
	) => void;
	goBack: () => void;
	goForward: () => void;
	canGoBack: boolean;
	canGoForward: boolean;

	viewMode: ViewMode;
	setViewMode: (mode: ViewMode) => void;
	sortBy: SortBy;
	setSortBy: (sort: SortBy) => void;
	viewSettings: ViewSettings;
	setViewSettings: (settings: Partial<ViewSettings>) => void;

	// Column view state (per-tab, stored in TabManager)
	columnStack: SdPath[];
	setColumnStack: (columns: SdPath[]) => void;

	// Scroll position (per-tab, stored in TabManager)
	scrollPosition: { top: number; left: number };
	setScrollPosition: (pos: { top: number; left: number }) => void;

	sidebarVisible: boolean;
	setSidebarVisible: (visible: boolean) => void;
	inspectorVisible: boolean;
	setInspectorVisible: (visible: boolean) => void;

	quickPreviewFileId: string | null;
	openQuickPreview: (fileId: string) => void;
	closeQuickPreview: () => void;

	currentFiles: File[];
	setCurrentFiles: (files: File[]) => void;

	tagModeActive: boolean;
	setTagModeActive: (active: boolean) => void;

	devices: Map<string, Device>;

	loadPreferencesForSpaceItem: (id: string) => void;

	// Tab info
	activeTabId: string;
}

const ExplorerContext = createContext<ExplorerContextValue | null>(null);

interface ExplorerProviderProps {
	children: ReactNode;
	/** Reserved for Phase 2: Will control whether this tab's context should process events/updates */
	isActiveTab?: boolean;
}

export function ExplorerProvider({
	children,
	isActiveTab: _isActiveTab = true,
}: ExplorerProviderProps) {
	const routerNavigate = useNavigate();
	const location = useLocation();
	const viewPrefs = useViewPreferencesStore();
	const sortPrefs = useSortPreferencesStore();

	// Get per-tab state from TabManager
	const { activeTabId, getExplorerState, updateExplorerState } =
		useTabManager();

	// Memoize tabState to ensure it updates when activeTabId or explorerStates change
	const tabState = useMemo(
		() => getExplorerState(activeTabId),
		[activeTabId, getExplorerState],
	);

	const [navState, navDispatch] = useReducer(
		navigationReducer,
		initialNavigationState,
	);
	const [uiState, uiDispatch] = useReducer(uiReducer, initialUIState);
	const [currentFiles, setCurrentFiles] = useReducer(
		(_: File[], files: File[]) => files,
		[] as File[],
	);

	// Parse columnStack from TabManager (stored as JSON strings)
	// Must depend on activeTabId to recalculate when switching tabs
	const columnStack = useMemo((): SdPath[] => {
		if (!tabState.columnStack || tabState.columnStack.length === 0) {
			return [];
		}
		try {
			return tabState.columnStack.map((s) => JSON.parse(s) as SdPath);
		} catch {
			return [];
		}
	}, [activeTabId, tabState.columnStack]);

	const setColumnStack = useCallback(
		(columns: SdPath[]) => {
			updateExplorerState(activeTabId, {
				columnStack: columns.map((c) => JSON.stringify(c)),
			});
		},
		[activeTabId, updateExplorerState],
	);

	const scrollPosition = useMemo(
		() => ({
			top: tabState.scrollTop,
			left: tabState.scrollLeft,
		}),
		[activeTabId, tabState.scrollTop, tabState.scrollLeft],
	);

	const setScrollPosition = useCallback(
		(pos: { top: number; left: number }) => {
			updateExplorerState(activeTabId, {
				scrollTop: pos.top,
				scrollLeft: pos.left,
			});
		},
		[activeTabId, updateExplorerState],
	);

	const currentTarget = navState.history[navState.index] ?? null;
	const canGoBack = navState.index > 0;
	const canGoForward = navState.index < navState.history.length - 1;

	const currentPath = useMemo(() => {
		if (currentTarget?.type === "path") return currentTarget.path;
		return null;
	}, [currentTarget]);

	const currentView = useMemo(() => {
		if (currentTarget?.type === "view") {
			return {
				view: currentTarget.view,
				id: currentTarget.id,
				params: currentTarget.params,
			};
		}
		return null;
	}, [currentTarget]);

	const devicesQuery = useNormalizedQuery<ListLibraryDevicesInput, Device[]>({
		wireMethod: "query:devices.list",
		input: { include_offline: true, include_details: false },
		resourceType: "device",
	});

	const devices = useMemo(() => {
		const list = devicesQuery.data ?? [];
		return new Map(list.map((d) => [d.id, d]));
	}, [devicesQuery.data]);

	// Exclude currentTarget from deps to prevent infinite sync loops.
	useEffect(() => {
		const target = urlToTarget(location.search);
		if (target && !targetsEqual(target, currentTarget)) {
			navDispatch({ type: "SYNC", target });
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [location.search]);

	const pathKey = getPathKey(currentTarget);

	useEffect(() => {
		const savedSort = sortPrefs.getPreferences(pathKey);
		if (savedSort) {
			uiDispatch({ type: "SET_SORT_BY", sort: savedSort as SortBy });
		}
	}, [pathKey, sortPrefs]);

	// "datetaken" only applies to media view; fall back to "modified" elsewhere.
	useEffect(() => {
		if (uiState.viewMode === "media" && uiState.sortBy === "type") {
			uiDispatch({ type: "SET_SORT_BY", sort: "datetaken" });
			sortPrefs.setPreferences(pathKey, "datetaken");
		} else if (
			uiState.viewMode !== "media" &&
			uiState.sortBy === "datetaken"
		) {
			uiDispatch({ type: "SET_SORT_BY", sort: "modified" });
			sortPrefs.setPreferences(pathKey, "modified");
		}
	}, [uiState.viewMode, uiState.sortBy, pathKey, sortPrefs]);

	const navigateToPath = useCallback(
		(path: SdPath) => {
			const target: NavigationTarget = { type: "path", path };
			navDispatch({ type: "NAVIGATE", target });
			routerNavigate(targetToUrl(target));
		},
		[routerNavigate],
	);

	const navigateToView = useCallback(
		(view: string, id?: string, params?: Record<string, string>) => {
			const target: NavigationTarget = { type: "view", view, id, params };
			navDispatch({ type: "NAVIGATE", target });
			routerNavigate(targetToUrl(target));
		},
		[routerNavigate],
	);

	const goBack = useCallback(() => {
		navDispatch({ type: "GO_BACK" });
		const targetIndex = navState.index - 1;
		if (targetIndex >= 0) {
			const target = navState.history[targetIndex];
			routerNavigate(targetToUrl(target), { replace: true });
		}
	}, [navState.index, navState.history, routerNavigate]);

	const goForward = useCallback(() => {
		navDispatch({ type: "GO_FORWARD" });
		const targetIndex = navState.index + 1;
		if (targetIndex < navState.history.length) {
			const target = navState.history[targetIndex];
			routerNavigate(targetToUrl(target), { replace: true });
		}
	}, [navState.index, navState.history, routerNavigate]);

	const spaceKey = getSpaceItemKey(location.pathname, location.search);

	// View settings from TabManager (per-tab)
	const viewMode = tabState.viewMode as ViewMode;
	const sortByValue = tabState.sortBy as SortBy;
	const viewSettings: ViewSettings = useMemo(
		() => ({
			gridSize: tabState.gridSize,
			gapSize: tabState.gapSize,
			foldersFirst: tabState.foldersFirst,
			showFileSize: true, // Not stored per-tab for now
			columnWidth: 256, // Not stored per-tab for now
		}),
		[
			activeTabId,
			tabState.gridSize,
			tabState.gapSize,
			tabState.foldersFirst,
		],
	);

	const setViewMode = useCallback(
		(mode: ViewMode) => {
			updateExplorerState(activeTabId, {
				viewMode: mode as TabViewMode,
			});
			viewPrefs.setPreferences(spaceKey, { viewMode: mode });
		},
		[activeTabId, updateExplorerState, spaceKey, viewPrefs],
	);

	const setSortBy = useCallback(
		(sort: SortBy) => {
			updateExplorerState(activeTabId, {
				sortBy: sort as TabSortBy,
			});
			sortPrefs.setPreferences(pathKey, sort);
		},
		[activeTabId, updateExplorerState, pathKey, sortPrefs],
	);

	const setViewSettings = useCallback(
		(settings: Partial<ViewSettings>) => {
			updateExplorerState(activeTabId, {
				gridSize: settings.gridSize ?? tabState.gridSize,
				gapSize: settings.gapSize ?? tabState.gapSize,
				foldersFirst: settings.foldersFirst ?? tabState.foldersFirst,
			});
			viewPrefs.setPreferences(spaceKey, {
				viewSettings: { ...viewSettings, ...settings },
			});
		},
		[
			activeTabId,
			updateExplorerState,
			tabState,
			spaceKey,
			viewSettings,
			viewPrefs,
		],
	);

	const setSidebarVisible = useCallback((visible: boolean) => {
		uiDispatch({ type: "SET_SIDEBAR_VISIBLE", visible });
	}, []);

	const setInspectorVisible = useCallback((visible: boolean) => {
		uiDispatch({ type: "SET_INSPECTOR_VISIBLE", visible });
	}, []);

	const openQuickPreview = useCallback((fileId: string) => {
		uiDispatch({ type: "SET_QUICK_PREVIEW", fileId });
	}, []);

	const closeQuickPreview = useCallback(() => {
		uiDispatch({ type: "SET_QUICK_PREVIEW", fileId: null });
	}, []);

	const setTagModeActive = useCallback((active: boolean) => {
		uiDispatch({ type: "SET_TAG_MODE", active });
	}, []);

	const loadPreferencesForSpaceItem = useCallback(
		(id: string) => {
			const prefs = viewPrefs.getPreferences(id);
			if (prefs) {
				uiDispatch({
					type: "LOAD_PREFERENCES",
					viewMode: prefs.viewMode,
					viewSettings: prefs.viewSettings,
				});
			}
		},
		[viewPrefs],
	);

	const value = useMemo<ExplorerContextValue>(
		() => ({
			currentTarget,
			currentPath,
			currentView,
			navigateToPath,
			navigateToView,
			goBack,
			goForward,
			canGoBack,
			canGoForward,
			viewMode,
			setViewMode,
			sortBy: sortByValue,
			setSortBy,
			viewSettings,
			setViewSettings,
			columnStack,
			setColumnStack,
			scrollPosition,
			setScrollPosition,
			sidebarVisible: uiState.sidebarVisible,
			setSidebarVisible,
			inspectorVisible: uiState.inspectorVisible,
			setInspectorVisible,
			quickPreviewFileId: uiState.quickPreviewFileId,
			openQuickPreview,
			closeQuickPreview,
			currentFiles,
			setCurrentFiles,
			tagModeActive: uiState.tagModeActive,
			setTagModeActive,
			devices,
			loadPreferencesForSpaceItem,
			activeTabId,
		}),
		[
			currentTarget,
			currentPath,
			currentView,
			navigateToPath,
			navigateToView,
			goBack,
			goForward,
			canGoBack,
			canGoForward,
			viewMode,
			setViewMode,
			sortByValue,
			setSortBy,
			viewSettings,
			setViewSettings,
			columnStack,
			setColumnStack,
			scrollPosition,
			setScrollPosition,
			uiState.sidebarVisible,
			setSidebarVisible,
			uiState.inspectorVisible,
			setInspectorVisible,
			uiState.quickPreviewFileId,
			openQuickPreview,
			closeQuickPreview,
			currentFiles,
			uiState.tagModeActive,
			setTagModeActive,
			devices,
			loadPreferencesForSpaceItem,
			activeTabId,
		],
	);

	return (
		<ExplorerContext.Provider value={value}>
			{children}
		</ExplorerContext.Provider>
	);
}

export function useExplorer(): ExplorerContextValue {
	const context = useContext(ExplorerContext);
	if (!context) {
		throw new Error("useExplorer must be used within an ExplorerProvider");
	}
	return context;
}

export {
	getSpaceItemKey,
	getSpaceItemKey as getSpaceItemKeyFromRoute,
	targetToKey,
	targetsEqual,
};
