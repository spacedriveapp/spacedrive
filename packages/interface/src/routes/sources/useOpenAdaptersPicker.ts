import { useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { useTabManager } from "../../components/TabManager/useTabManager";

const ADAPTERS_PATH = "/sources/adapters";

/**
 * Opens the adapters picker within the current Sources tab.
 * Closes stale adapters-only tabs left from older builds.
 */
export function useOpenAdaptersPicker() {
	const navigate = useNavigate();
	const { tabs, activeTabId, closeTab } = useTabManager();

	return useCallback(() => {
		for (const tab of tabs) {
			if (tab.id !== activeTabId && tab.savedPath === ADAPTERS_PATH) {
				closeTab(tab.id);
			}
		}
		navigate(ADAPTERS_PATH);
	}, [tabs, activeTabId, closeTab, navigate]);
}