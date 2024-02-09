import { RouteObject } from 'react-router';

export const debugRoutes = [
	{ path: 'cache', lazy: () => import('./cache') },
	{ path: 'cloud', lazy: () => import('./cloud') },
	{ path: 'sync', lazy: () => import('./sync') },
	{ path: 'actors', lazy: () => import('./actors') },
	{ path: 'p2p', lazy: () => import('./p2p') }
] satisfies RouteObject[];
