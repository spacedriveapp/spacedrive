import { RouteObject } from 'react-router-dom';
import settingsRoutes from './settings';

export default [
	{
		lazy: () => import('./PageLayout'),
		children: [
			{
				path: 'overview',
				lazy: () => import('./overview')
			},
			{ path: 'people', lazy: () => import('./people') },
			{ path: 'media', lazy: () => import('./media') },
			{ path: 'spaces', lazy: () => import('./spaces') },
			{ path: 'debug', lazy: () => import('./debug') },
			{ path: 'spacedrop', lazy: () => import('./spacedrop') },
			{ path: 'sync', lazy: () => import("./sync") }
		]
	},
	{ path: 'location/:id', lazy: () => import('./location/$id') },
	{ path: 'tag/:id', lazy: () => import('./tag/$id') },
	{
		path: 'settings',
		lazy: () => import('./settings/Layout'),
		children: settingsRoutes
	},
	{ path: '*', lazy: () => import('./404') }
] satisfies RouteObject[];
