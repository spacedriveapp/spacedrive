import {
	createContext,
	useContext,
	useState,
	useMemo,
	useEffect,
	useCallback,
	type ReactNode,
} from "react";
import { useNavigate } from "react-router-dom";
import { useNormalizedQuery } from "../../context";
import { usePlatform } from "../../platform";

import type {
	SdPath,
	File,
	ListLibraryDevicesInput,
	DirectorySortBy,
	MediaSortBy,
} from "@sd/ts-client";
import {
	useViewPreferencesStore,
	useSortPreferencesStore,
} from "@sd/ts-client";

interface ViewSettings {
	gridSize: number; // 80-400px
	gapSize: number; // 1-32px
	showFileSize: boolean;
	columnWidth: number; // 200-400px for column view
	foldersFirst: boolean;
}

export type NavigationEntry =
	| { type: "path"; path: SdPath }
	| {
			type: "view";
			view: string;
			id?: string;
			params?: Record<string, string>;
	  };

export interface VirtualView {
	view: string;
	id?: string;
	params?: Record<string, string>;
}

function getSpaceItemKeyFromRoute(pathname: string, search: string): string {
	if (pathname === "/") return "overview";
	if (pathname === "/recents") return "recents";
	if (pathname === "/favorites") return "favorites";
	if (pathname === "/file-kinds") return "file-kinds";
	if (pathname.startsWith("/tag/")) {
		const tagId = pathname.replace("/tag/", "");
		return `tag:${tagId}`;
	}
	if (pathname === "/explorer" && search) {
		return `explorer:${search}`;
	}
	return pathname;
}

function getPathKey(sdPath: SdPath | null): string {
	if (!sdPath) return "null";
	return JSON.stringify(sdPath);
}

interface ExplorerState {
	currentPath: SdPath | null;
	currentView: VirtualView | null;
	setCurrentPath: (path: SdPath | null) => void;
	navigateToView: (
		view: string,
		id?: string,
		params?: Record<string, string>,
	) => void;
	syncPathFromUrl: (path: SdPath | null) => void;
	syncViewFromUrl: (view: VirtualView | null) => void;

	history: NavigationEntry[];
	historyIndex: number;
	goBack: () => void;
	goForward: () => void;
	canGoBack: boolean;
	canGoForward: boolean;

	viewMode: "grid" | "list" | "media" | "column" | "size" | "knowledge";
	setViewMode: (
		mode: "grid" | "list" | "media" | "column" | "size" | "knowledge",
	) => void;

	sortBy: DirectorySortBy | MediaSortBy;
	setSortBy: (sort: DirectorySortBy | MediaSortBy) => void;

	viewSettings: ViewSettings;
	setViewSettings: (settings: Partial<ViewSettings>) => void;

	sidebarVisible: boolean;
	setSidebarVisible: (visible: boolean) => void;
	inspectorVisible: boolean;
	setInspectorVisible: (visible: boolean) => void;

	quickPreviewFileId: string | null;
	setQuickPreviewFileId: (fileId: string | null) => void;
	openQuickPreview: (fileId: string) => void;
	closeQuickPreview: () => void;

	currentFiles: File[];
	setCurrentFiles: (files: File[]) => void;

	tagModeActive: boolean;
	setTagModeActive: (active: boolean) => void;

	devices: Map<string, any>;

	setSpaceItemId: (id: string) => void;
}

const ExplorerContext = createContext<ExplorerState | null>(null);

interface ExplorerProviderProps {
	children: ReactNode;
	spaceItemId?: string;
}

export function ExplorerProvider({
	children,
	spaceItemId: initialSpaceItemId,
}: ExplorerProviderProps) {
	const navigate = useNavigate();
	const platform = usePlatform();
	const viewPrefs = useViewPreferencesStore();
	const sortPrefs = useSortPreferencesStore();

	const [spaceItemIdInternal, setSpaceItemIdInternal] = useState(
		initialSpaceItemId || "default",
	);
	const [currentPath, setCurrentPathInternal] = useState<SdPath | null>(null);
	const [currentView, setCurrentView] = useState<VirtualView | null>(null);
	const [history, setHistory] = useState<NavigationEntry[]>([]);
	const [historyIndex, setHistoryIndex] = useState(-1);
	const [viewMode, setViewModeInternal] = useState<
		"grid" | "list" | "media" | "column" | "size" | "knowledge"
	>("grid");
	const [sortByInternal, setSortByInternal] = useState<
		DirectorySortBy | MediaSortBy
	>("name");
	const [viewSettings, setViewSettingsInternal] = useState<ViewSettings>({
		gridSize: 120,
		gapSize: 16,
		showFileSize: true,
		columnWidth: 256,
		foldersFirst: false,
	});
	const [sidebarVisible, setSidebarVisible] = useState(true);
	const [inspectorVisible, setInspectorVisible] = useState(true);
	const [quickPreviewFileId, setQuickPreviewFileId] = useState<string | null>(
		null,
	);
	const [currentFiles, setCurrentFiles] = useState<File[]>([]);
	const [tagModeActive, setTagModeActive] = useState(false);

	const spaceItemKey = spaceItemIdInternal;
	const pathKey = getPathKey(currentPath);

	// Load view preferences when space item changes
	useEffect(() => {
		const prefs = viewPrefs.getPreferences(spaceItemKey);
		if (prefs) {
			setViewModeInternal(prefs.viewMode);
			if (prefs.viewSettings) {
				setViewSettingsInternal((prev) => ({
					...prev,
					...prefs.viewSettings,
				}));
			}
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [spaceItemKey]);

	// Load sort preferences when path changes
	useEffect(() => {
		const sortPref = sortPrefs.getPreferences(pathKey);
		if (sortPref) {
			setSortByInternal(sortPref as DirectorySortBy | MediaSortBy);
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [pathKey]);

	// Wrapper for setViewMode that persists to store
	const setViewMode = useCallback(
		(mode: "grid" | "list" | "media" | "column" | "size" | "knowledge") => {
			setViewModeInternal(mode);
			viewPrefs.setPreferences(spaceItemKey, { viewMode: mode });
			// eslint-disable-next-line react-hooks/exhaustive-deps
		},
		[spaceItemKey],
	);

	// Wrapper for setSortBy that persists to store
	const setSortBy = useCallback(
		(sort: DirectorySortBy | MediaSortBy) => {
			setSortByInternal(sort);
			sortPrefs.setPreferences(pathKey, sort);
			// eslint-disable-next-line react-hooks/exhaustive-deps
		},
		[pathKey],
	);

	// Update sort when switching to media view
	useEffect(() => {
		if (viewMode === "media" && sortByInternal === "type") {
			setSortByInternal("datetaken");
			sortPrefs.setPreferences(pathKey, "datetaken");
		} else if (viewMode !== "media" && sortByInternal === "datetaken") {
			setSortByInternal("modified");
			sortPrefs.setPreferences(pathKey, "modified");
		}
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [viewMode, sortByInternal, pathKey]);

	const setViewSettings = useCallback(
		(settings: Partial<ViewSettings>) => {
			setViewSettingsInternal((prev) => {
				const updated = { ...prev, ...settings };
				viewPrefs.setPreferences(spaceItemKey, {
					viewSettings: updated,
				});
				return updated;
			});
			// eslint-disable-next-line react-hooks/exhaustive-deps
		},
		[spaceItemKey],
	);

	// Use normalized query for automatic updates when device events are emitted
	const devicesQuery = useNormalizedQuery<ListLibraryDevicesInput, any[]>({
		wireMethod: "query:devices.list",
		input: { include_offline: true, include_details: false },
		resourceType: "device",
	});

	const devices = useMemo(() => {
		const deviceList = devicesQuery.data || [];
		return new Map(deviceList.map((d) => [d.id, d]));
	}, [devicesQuery.data]);

	const goBack = useCallback(() => {
		if (historyIndex > 0) {
			const newIndex = historyIndex - 1;
			const entry = history[newIndex];
			setHistoryIndex(newIndex);

			if (entry.type === "path") {
				setCurrentPathInternal(entry.path);
				setCurrentView(null);
				const encodedPath = encodeURIComponent(
					JSON.stringify(entry.path),
				);
				navigate(`/explorer?path=${encodedPath}`, { replace: true });
			} else {
				setCurrentPathInternal(null);
				setCurrentView({
					view: entry.view,
					id: entry.id,
					params: entry.params,
				});
				const params = new URLSearchParams({
					view: entry.view,
					...(entry.id && { id: entry.id }),
					...(entry.params || {}),
				});
				navigate(`/explorer?${params.toString()}`, { replace: true });
			}
		}
	}, [historyIndex, history, navigate]);

	const goForward = useCallback(() => {
		if (historyIndex < history.length - 1) {
			const newIndex = historyIndex + 1;
			const entry = history[newIndex];
			setHistoryIndex(newIndex);

			if (entry.type === "path") {
				setCurrentPathInternal(entry.path);
				setCurrentView(null);
				const encodedPath = encodeURIComponent(
					JSON.stringify(entry.path),
				);
				navigate(`/explorer?path=${encodedPath}`, { replace: true });
			} else {
				setCurrentPathInternal(null);
				setCurrentView({
					view: entry.view,
					id: entry.id,
					params: entry.params,
				});
				const params = new URLSearchParams({
					view: entry.view,
					...(entry.id && { id: entry.id }),
					...(entry.params || {}),
				});
				navigate(`/explorer?${params.toString()}`, { replace: true });
			}
		}
	}, [historyIndex, history, navigate]);

	const canGoBack = historyIndex > 0;
	const canGoForward = historyIndex < history.length - 1;

	const navigateToPath = useCallback(
		(path: SdPath | null) => {
			if (!path) {
				setCurrentPathInternal(null);
				return;
			}

			// Clear view state
			setCurrentView(null);

			// Update history
			setHistory((prev) => {
				const newHistory = prev.slice(0, historyIndex + 1);
				newHistory.push({ type: "path", path });
				return newHistory;
			});
			setHistoryIndex((prev) => prev + 1);
			setCurrentPathInternal(path);

			// Update URL to match
			const encodedPath = encodeURIComponent(JSON.stringify(path));
			navigate(`/explorer?path=${encodedPath}`, { replace: false });
		},
		[historyIndex, navigate],
	);

	const navigateToView = useCallback(
		(view: string, id?: string, params?: Record<string, string>) => {
			// Clear path state
			setCurrentPathInternal(null);

			// Set view state
			setCurrentView({ view, id, params });

			// Update history
			setHistory((prev) => {
				const newHistory = prev.slice(0, historyIndex + 1);
				newHistory.push({ type: "view", view, id, params });
				return newHistory;
			});
			setHistoryIndex((prev) => prev + 1);

			// Update URL
			const queryParams = new URLSearchParams({
				view,
				...(id && { id }),
				...(params || {}),
			});
			navigate(`/explorer?${queryParams.toString()}`, { replace: false });
		},
		[historyIndex, navigate],
	);

	const syncPathFromUrl = useCallback((path: SdPath | null) => {
		// Update internal state without navigating - used when URL changes externally
		setCurrentPathInternal(path);
		setCurrentView(null); // Clear view when syncing path
	}, []);

	const syncViewFromUrl = useCallback((view: VirtualView | null) => {
		// Update internal state without navigating - used when URL changes externally
		setCurrentView(view);
		setCurrentPathInternal(null); // Clear path when syncing view
	}, []);

	const openQuickPreview = useCallback((fileId: string) => {
		setQuickPreviewFileId(fileId);
	}, []);

	const closeQuickPreview = useCallback(() => {
		setQuickPreviewFileId(null);
	}, []);

	const value: ExplorerState = useMemo(
		() => ({
			currentPath,
			currentView,
			setCurrentPath: navigateToPath,
			navigateToView,
			syncPathFromUrl,
			syncViewFromUrl,
			history,
			historyIndex,
			goBack,
			goForward,
			canGoBack,
			canGoForward,
			viewMode,
			setViewMode,
			sortBy: sortByInternal,
			setSortBy,
			viewSettings,
			setViewSettings,
			sidebarVisible,
			setSidebarVisible,
			inspectorVisible,
			setInspectorVisible,
			quickPreviewFileId,
			setQuickPreviewFileId,
			openQuickPreview,
			closeQuickPreview,
			currentFiles,
			setCurrentFiles,
			tagModeActive,
			setTagModeActive,
			devices,
			setSpaceItemId: setSpaceItemIdInternal,
		}),
		[
			currentPath,
			currentView,
			navigateToPath,
			navigateToView,
			syncPathFromUrl,
			syncViewFromUrl,
			history,
			historyIndex,
			goBack,
			goForward,
			canGoBack,
			canGoForward,
			viewMode,
			setViewMode,
			sortByInternal,
			setSortBy,
			viewSettings,
			setViewSettings,
			sidebarVisible,
			inspectorVisible,
			quickPreviewFileId,
			openQuickPreview,
			closeQuickPreview,
			currentFiles,
			tagModeActive,
			devices,
		],
	);

	return (
		<ExplorerContext.Provider value={value}>
			{children}
		</ExplorerContext.Provider>
	);
}

export function useExplorer() {
	const context = useContext(ExplorerContext);
	if (!context)
		throw new Error("useExplorer must be used within ExplorerProvider");
	return context;
}

export { getSpaceItemKeyFromRoute };
