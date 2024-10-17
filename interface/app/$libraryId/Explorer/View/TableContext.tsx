import { ColumnSizingState } from '@tanstack/react-table';
import { createContext, useContext } from 'react';

interface TableContext {
	columnSizing: ColumnSizingState;
}

export const TableContext = createContext<TableContext | null>(null);

export const useTableContext = () => {
	const ctx = useContext(TableContext);

	if (ctx === null) throw new Error('useTableContext() was called outside of a TableContext!');

	return ctx;
};
