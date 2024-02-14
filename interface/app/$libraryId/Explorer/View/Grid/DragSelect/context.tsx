import { createContext, useContext } from 'react';
import Selecto from 'react-selecto';

import { Drag } from '.';
import { useSelectedTargets } from './useSelectedTargets';

interface DragSelectContext extends ReturnType<typeof useSelectedTargets> {
	selecto: React.RefObject<Selecto>;
	drag: React.MutableRefObject<Drag | null>;
}

export const DragSelectContext = createContext<DragSelectContext | null>(null);

export const useDragSelectContext = () => {
	const ctx = useContext(DragSelectContext);

	if (ctx === null) throw new Error('DragSelectContext.Provider not found!');

	return ctx;
};
