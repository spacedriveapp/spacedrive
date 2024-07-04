import { RouteObject } from 'react-router';

export default [
	{ path: 'about', lazy: () => import('./about') },
	{ path: 'changelog', lazy: () => import('./changelog') }
	// { path: 'dependencies', lazy: () => import('./dependencies') },
] satisfies RouteObject[];
