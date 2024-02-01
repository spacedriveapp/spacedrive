import { createContext, useContext, useLayoutEffect } from 'react';
import { useRoutingContext } from '~/RoutingContext';

export function useRouteTitle(title: string) {
	const routingCtx = useRoutingContext();
	const ctx = useContext(RouteTitleContext);

	// layout effect avoids 'New Tab' showing up briefly
	useLayoutEffect(() => {
		document.title = title;
		if (ctx) ctx.setTitle(routingCtx.tabId, title);
	}, [routingCtx.tabId, title, ctx]);

	return title;
}

export const RouteTitleContext = createContext<{
	setTitle: (id: string, title: string) => void;
} | null>(null);
