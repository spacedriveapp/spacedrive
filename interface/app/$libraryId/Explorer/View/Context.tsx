import { createContext, useContext, type ReactNode, type RefObject } from 'react';

import { useActiveItem } from './useActiveItem';

export interface ExplorerViewContextProps extends ReturnType<typeof useActiveItem> {
	ref: RefObject<HTMLDivElement>;
	/**
	 * Padding to apply when scrolling to an item.
	 */
	scrollPadding?: { top?: number; bottom?: number };
	contextMenu?: ReactNode;
	selectable: boolean;
	listViewOptions?: {
		hideHeaderBorder?: boolean;
	};
}

export const ExplorerViewContext = createContext<ExplorerViewContextProps | null>(null);

export const useExplorerViewContext = () => {
	const ctx = useContext(ExplorerViewContext);

	if (ctx === null) throw new Error('ViewContext.Provider not found!');

	return ctx;
};
