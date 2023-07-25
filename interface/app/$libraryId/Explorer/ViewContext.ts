import { ReactNode, RefObject, createContext, useContext } from 'react';
import { ExplorerItem } from '@sd/client';

export type ExplorerViewSelection = number | Set<number>;

export interface ExplorerViewContext<T extends ExplorerViewSelection = ExplorerViewSelection> {
	items: ExplorerItem[] | null;
	viewRef: RefObject<HTMLDivElement>;
	scrollRef: RefObject<HTMLDivElement>;
	selected?: T;
	onSelectedChange?: React.Dispatch<React.SetStateAction<ExplorerViewSelectionChange<T>>>;
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
	gap?: number | { x?: number; y?: number };
}

export type ExplorerViewSelectionChange<T extends ExplorerViewSelection> = T extends Set<number>
	? Set<number>
	: number | undefined;

export const ViewContext = createContext<ExplorerViewContext | null>(null);

export const useExplorerViewContext = () => {
	const ctx = useContext(ViewContext);

	if (ctx === null) throw new Error('ViewContext.Provider not found!');

	return ctx;
};
