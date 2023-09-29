import { createContext, useContext, type ReactNode, type RefObject } from 'react';

import { ExplorerViewPadding } from './View';

export interface ExplorerViewContext {
	ref: RefObject<HTMLDivElement>;
	top?: number;
	bottom?: number;
	contextMenu?: ReactNode;
	isContextMenuOpen?: boolean;
	setIsContextMenuOpen?: (isOpen: boolean) => void;
	isRenaming: boolean;
	setIsRenaming: (isRenaming: boolean) => void;
	padding?: Omit<ExplorerViewPadding, 'x' | 'y'>;
	gap?: number | { x?: number; y?: number };
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
