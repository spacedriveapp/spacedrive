import { createContext, useContext, useState } from 'react';
import { SearchFilterArgs } from '@sd/client';

export const TopBarContext = createContext<ReturnType<typeof useContextValue> | null>(null);

export function useContextValue() {
	const [left, setLeft] = useState<HTMLDivElement | null>(null);
	const [center, setCenter] = useState<HTMLDivElement | null>(null);
	const [right, setRight] = useState<HTMLDivElement | null>(null);
	const [children, setChildren] = useState<HTMLDivElement | null>(null);
	const [fixedArgs, setFixedArgs] = useState<SearchFilterArgs[] | null>(null);
	const [topBarHeight, setTopBarHeight] = useState(0);

	return {
		left,
		setLeft,
		center,
		setCenter,
		right,
		setRight,
		children,
		setChildren,
		fixedArgs,
		setFixedArgs,
		topBarHeight,
		setTopBarHeight
	};
}

export function useTopBarContext() {
	const ctx = useContext(TopBarContext);

	if (!ctx) throw new Error('TopBarContext not found!');

	return ctx;
}
