import { createContext, useContext } from 'react';

import { Router } from './';

export const TabsContext = createContext<{
	tabIndex: number;
	setTabIndex: (i: number) => void;
	tabs: { router: Router; title: string }[];
	createTab(redirect?: { pathname: string; search: string | undefined }): void;
	removeTab(index: number): void;
	duplicateTab(): void;
} | null>(null);

export function useTabsContext() {
	const ctx = useContext(TabsContext);

	return ctx;
}
