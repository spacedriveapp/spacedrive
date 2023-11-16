import { createRouter, InitialEntry, MemoryHistory } from '@remix-run/router';
import { UNSAFE_mapRouteProperties } from 'react-router';
import { RouteObject } from 'react-router-dom';

export function createMemoryRouterWithHistory(props: {
	routes: RouteObject[];
	history: MemoryHistory;
	basename?: string;
	initialEntries?: InitialEntry[];
	initialIndex?: number;
}) {
	return createRouter({
		routes: props.routes,
		history: props.history,
		basename: props.basename,
		future: {
			v7_prependBasename: true
		},
		mapRouteProperties: UNSAFE_mapRouteProperties
	}).initialize();
}
