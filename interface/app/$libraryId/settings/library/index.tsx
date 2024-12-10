import { RouteObject } from 'react-router';

export default [
	{
		lazy: () => import('../OverviewLayout'),
		children: [
			{ path: 'security', lazy: () => import('./security') },
			{ path: 'sharing', lazy: () => import('./sharing') },
			{ path: 'general', lazy: () => import('./general') },
			{ path: 'tags', lazy: () => import('./tags') },
			{ path: 'tags/:id', lazy: () => import('./tags') },
			{ path: 'locations', lazy: () => import('./locations') },
			{ path: 'volumes', lazy: () => import('./volumes') },
			{ path: 'devices', lazy: () => import('./devices') },
			{ path: 'sync', lazy: () => import('./sync') },
			{ path: 'clouds', lazy: () => import('./clouds') },
			{ path: 'users', lazy: () => import('./users') }
		]
	},
	{ path: 'locations/:id', lazy: () => import('./locations/$id') }
] satisfies RouteObject[];
