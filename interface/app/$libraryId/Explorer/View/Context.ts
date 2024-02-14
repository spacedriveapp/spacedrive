import { createContext, useContext, type ReactNode, type RefObject } from 'react';

export interface ExplorerViewContext {
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

export const ViewContext = createContext<ExplorerViewContext | null>(null);

export const useExplorerViewContext = () => {
	const ctx = useContext(ViewContext);

	if (ctx === null) throw new Error('ViewContext.Provider not found!');

	return ctx;
};
