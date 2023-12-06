import { RouteObject } from 'react-router';

export const debugRoutes: RouteObject = {
	children: [{ path: 'cache', lazy: () => import('./cache') }]
};
