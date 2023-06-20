import { ReactNode, RefObject, createContext, useContext } from 'react';
import { ExplorerItem } from '@sd/client';

export type ExplorerViewSelection = number | number[];

export interface ExplorerViewContext<T extends ExplorerViewSelection = ExplorerViewSelection> {
	items: ExplorerItem[] | null;
	scrollRef: RefObject<HTMLDivElement>;
	selected?: T;
	onSelectedChange?: (selected: ExplorerViewSelectionChange<T>) => void;
	overscan?: number;
	onLoadMore?: () => void;
	rowsBeforeLoadMore?: number;
	top?: number;
	multiSelect?: boolean;
	contextMenu?: ReactNode;
	setIsContextMenuOpen?: (isOpen: boolean) => void;
	isRenaming: boolean;
	setIsRenaming: (isRenaming: boolean) => void;
	selectable?: boolean;
	padding?: number | { x?: number; y?: number };
}

export type ExplorerViewSelectionChange<T extends ExplorerViewSelection> = T extends number[]
	? number[]
	: number | undefined;

export const ViewContext = createContext<ExplorerViewContext | null>(null);

export const useExplorerViewContext = () => {
	const ctx = useContext(ViewContext);

	if (ctx === null) throw new Error('ViewContext.Provider not found!');

	return ctx;
};
