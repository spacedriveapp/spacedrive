import { ColumnSizingState, VisibilityState } from '@tanstack/react-table';
import { createContext, useContext } from 'react';

import { ExplorerViewPadding } from '..';

interface TableContext {
	padding: Required<Omit<ExplorerViewPadding, 'x' | 'y'>>;
	columnVisibility: VisibilityState;
	columnSizing: ColumnSizingState;
}

export const TableContext = createContext<TableContext | null>(null);

export const useTableContext = () => {
	const ctx = useContext(TableContext);

	if (ctx === null) throw new Error('TableContext.Provider not found!');

	return ctx;
};
