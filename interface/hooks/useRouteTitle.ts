import { createContext, useContext, useLayoutEffect } from 'react';

export function useRouteTitle(title: string) {
	const ctx = useContext(RouteTitleContext);

	// layout effect avoids 'New Tab' showing up briefly
	useLayoutEffect(() => {
		document.title = title;
		if (ctx) ctx.setTitle(title);
	}, [title, ctx]);

	return title;
}

export const RouteTitleContext = createContext<{
	setTitle: (title: string) => void;
} | null>(null);
