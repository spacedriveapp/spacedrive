import { RefObject, createContext, useContext } from 'react';
import { ExplorerItem } from '@sd/client';

interface Context {
	data: ExplorerItem[] | null;
	scrollRef: RefObject<HTMLDivElement>;
	isFetchingNextPage?: boolean;
	onLoadMore?(): void;
	hasNextPage?: boolean;
	selectedItems: Set<number>;
	onSelectedChange?(selectedItems: Set<number>): void;
	overscan?: number;
}

export const ViewContext = createContext<Context | null>(null);

export const useExplorerViewContext = () => {
	const ctx = useContext(ViewContext);

	if (ctx === null) throw new Error('ViewContext.Provider not found!');

	return ctx;
};
