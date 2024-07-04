import { RouteObject } from 'react-router';

export const debugRoutes = [
	{ path: 'cloud', lazy: () => import('./cloud') },
	{ path: 'actors', lazy: () => import('./actors') }
] satisfies RouteObject[];
