import { createContext, useContext } from 'react';
import Selecto from 'react-selecto';

interface GridContext {
	selecto?: React.RefObject<Selecto>;
	selectoUnselected: React.MutableRefObject<Set<string>>;
}

export const GridContext = createContext<GridContext | null>(null);

export const useGridContext = () => {
	const ctx = useContext(GridContext);

	if (ctx === null) throw new Error('GridContext.Provider not found!');

	return ctx;
};
