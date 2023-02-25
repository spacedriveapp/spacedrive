import { RouteObject } from 'react-router-dom';
import { lazyEl } from '~/util';
import settingsRoutes from './settings';

export default [
	{
		element: lazyEl(() => import("./PageLayout")),
		children: [
			{
				path: 'overview',
				element: lazyEl(() => import('./overview'))
			},
			{ path: 'people', element: lazyEl(() => import('./people'))},
			{ path: 'media', element: lazyEl(() => import('./media')) },
			{ path: 'spaces', element: lazyEl(() => import('./spaces')) },
			{ path: 'debug', element: lazyEl(() => import('./debug')) },
			{ path: 'spacedrop', element: lazyEl(() => import('./spacedrop')) },
		]
	},
	{ path: 'location/:id', element: lazyEl(() => import('./location/:id')) },
	{ path: 'tag/:id', element: lazyEl(() => import('./tag/:id')) },
	{
		path: 'settings',
		element: lazyEl(() => import('./settings/Layout')),
		children: settingsRoutes
	},
	{ path: '*', element: lazyEl(() => import('./404')) }
] satisfies RouteObject[];
