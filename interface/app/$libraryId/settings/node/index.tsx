import { RouteObject } from 'react-router';

export default [
	{ path: 'p2p', lazy: () => import('./p2p') },
	{ path: 'libraries', lazy: () => import('./libraries') }
] satisfies RouteObject[];
