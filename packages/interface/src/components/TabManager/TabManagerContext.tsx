import {
	createContext,
	useState,
	useCallback,
	useMemo,
	useEffect,
	type ReactNode,
} from "react";
import { createBrowserRouter } from "react-router-dom";
import type { Router } from "@remix-run/router";
import { useNavigate, useLocation } from "react-router-dom";

export interface Tab {
	id: string;
	title: string;
	icon: string | null;
	isPinned: boolean;
	lastActive: number;
	savedPath: string;
}

export interface TabScrollState {
	viewMode: string;
	scrollTop: number;
	scrollLeft: number;
	virtualOffset: number;
}

interface TabManagerContextValue {
	tabs: Tab[];
	activeTabId: string;
	router: Router;
	createTab: (title?: string, path?: string) => void;
	closeTab: (tabId: string) => void;
	switchTab: (tabId: string) => void;
	updateTabTitle: (tabId: string, title: string) => void;
	saveScrollState: (tabId: string, state: TabScrollState) => void;
	getScrollState: (tabId: string) => TabScrollState | null;
	nextTab: () => void;
	previousTab: () => void;
	selectTabAtIndex: (index: number) => void;
	updateTabPath: (tabId: string, path: string) => void;
}

const TabManagerContext = createContext<TabManagerContextValue | null>(null);

interface TabManagerProviderProps {
	children: ReactNode;
	routes: any[];
}

export function TabManagerProvider({
	children,
	routes,
}: TabManagerProviderProps) {
	const router = useMemo(() => createBrowserRouter(routes), [routes]);

	const [tabs, setTabs] = useState<Tab[]>(() => [
		{
			id: crypto.randomUUID(),
			title: "Overview",
			icon: null,
			isPinned: false,
			lastActive: Date.now(),
			savedPath: "/",
		},
	]);

	const [activeTabId, setActiveTabId] = useState<string>(tabs[0].id);
	const [scrollStates, setScrollStates] = useState<
		Map<string, TabScrollState>
	>(new Map());

	const createTab = useCallback((title = "Overview", path = "/") => {
		const newTab: Tab = {
			id: crypto.randomUUID(),
			title,
			icon: null,
			isPinned: false,
			lastActive: Date.now(),
			savedPath: path,
		};

		setTabs((prev) => [...prev, newTab]);
		setActiveTabId(newTab.id);
	}, []);

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
			prev.map((tab) =>
				tab.id === tabId ? { ...tab, title } : tab,
			),
		);
	}, []);

	const saveScrollState = useCallback(
		(tabId: string, state: TabScrollState) => {
			setScrollStates((prev) => new Map(prev).set(tabId, state));
		},
		[],
	);

	const getScrollState = useCallback(
		(tabId: string): TabScrollState | null => {
			return scrollStates.get(tabId) || null;
		},
		[scrollStates],
	);

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

	const updateTabPath = useCallback((tabId: string, path: string) => {
		setTabs((prev) =>
			prev.map((tab) =>
				tab.id === tabId ? { ...tab, savedPath: path } : tab,
			),
		);
	}, []);

	const value = useMemo<TabManagerContextValue>(
		() => ({
			tabs,
			activeTabId,
			router,
			createTab,
			closeTab,
			switchTab,
			updateTabTitle,
			saveScrollState,
			getScrollState,
			nextTab,
			previousTab,
			selectTabAtIndex,
			updateTabPath,
		}),
		[
			tabs,
			activeTabId,
			router,
			createTab,
			closeTab,
			switchTab,
			updateTabTitle,
			saveScrollState,
			getScrollState,
			nextTab,
			previousTab,
			selectTabAtIndex,
			updateTabPath,
		],
	);

	return (
		<TabManagerContext.Provider value={value}>
			{children}
		</TabManagerContext.Provider>
	);
}

export { TabManagerContext };
