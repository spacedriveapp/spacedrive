import { createContext, useContext } from 'react';

interface SidebarContextProps {
	show: boolean;
	locked: boolean;
	collapsed: boolean;
	onLockedChange: (val: boolean) => void;
}

export const SidebarContext = createContext<SidebarContextProps | null>(null);

export const useSidebarContext = () => {
	const ctx = useContext(SidebarContext);

	if (ctx === null) throw new Error('SidebarContext.Provider not found!');

	return ctx;
};
