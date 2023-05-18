import { RefObject, createContext, useContext } from 'react';
import { ExplorerItem } from '@sd/client';

interface Context {
	data: ExplorerItem[];
	scrollRef: RefObject<HTMLDivElement>;
	isFetchingNextPage?: boolean;
	onLoadMore?(): void;
	hasNextPage?: boolean;
}

export const ViewContext = createContext<Context | null>(null);

export const useExplorerViewContext = () => {
	const ctx = useContext(ViewContext);

	if (ctx === null) throw new Error('ViewContext.Provider not found!');

	return ctx;
};
