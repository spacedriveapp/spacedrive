import { RouteObject } from 'react-router';
import { lazyEl } from '~/util';

export default [
	{
		element: lazyEl(() => import('../OverviewLayout')),
		children: [
			{ path: 'contacts', element: lazyEl(() => import('./contacts')) },
			{ path: 'keys', element: lazyEl(() => import('./keys')) },
			{ path: 'security', element: lazyEl(() => import('./security')) },
			{ path: 'sharing', element: lazyEl(() => import('./sharing')) },
			{ path: 'sync', element: lazyEl(() => import('./sync')) },
			{ path: 'tags', element: lazyEl(() => import('./tags')) },
			{ path: 'general', element: lazyEl(() => import('./general')) },
			{ path: 'tags', element: lazyEl(() => import('./tags')) },
			{ path: 'nodes', element: lazyEl(() => import('./nodes')) },
			{ path: 'locations', element: lazyEl(() => import('./locations')) }
		]
	},
	{ path: 'locations/:id', element: lazyEl(() => import('./locations/$id')) }
] satisfies RouteObject[];
