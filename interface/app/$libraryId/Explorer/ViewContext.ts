import { ReactNode, RefObject, createContext, useContext } from 'react';

export type ExplorerViewSelection = number | Set<number>;

export interface ExplorerViewContext {
	ref: RefObject<HTMLDivElement>;
	overscan?: number;
	top?: number;
	contextMenu?: ReactNode;
	setIsContextMenuOpen?: (isOpen: boolean) => void;
	isRenaming: boolean;
	setIsRenaming: (isRenaming: boolean) => void;
	selectable?: boolean;
	padding?: number | { x?: number; y?: number };
	gap?: number | { x?: number; y?: number };
}

export const ViewContext = createContext<ExplorerViewContext | null>(null);

export const useExplorerViewContext = () => {
	const ctx = useContext(ViewContext);

	if (ctx === null) throw new Error('ViewContext.Provider not found!');

	return ctx;
};
