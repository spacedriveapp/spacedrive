import { RouteObject } from 'react-router';
import { LocationIdParams, LocationIdParamsSchema } from '~/app/$libraryId';

export type { LocationIdParams } from '~/app/$libraryId';

export default [
	{
		lazy: () => import('../OverviewLayout'),
		children: [
			{ path: 'contacts', lazy: () => import('./contacts') },
			// { path: 'keys', lazy: () => import('./keys') },
			{ path: 'security', lazy: () => import('./security') },
			{ path: 'sharing', lazy: () => import('./sharing') },
			{ path: 'sync', lazy: () => import('./sync') },
			{ path: 'general', lazy: () => import('./general') },
			{
				path: 'tags',
				lazy: () => import('./tags'),
				loader(): LocationIdParams {
					return { id: -1 };
				}
			},
			{
				path: 'tags/:id', //this is for edit in tags context menu
				lazy: () => import('./tags'),
				loader: ({ params }) => LocationIdParamsSchema.parse(params)
			},
			{ path: 'nodes', lazy: () => import('./nodes') },
			{ path: 'locations', lazy: () => import('./locations') }
		]
	},
	{
		path: 'locations/:id',
		lazy: () => import('./locations/$id'),
		loader: ({ params }) => LocationIdParamsSchema.parse(params)
	}
] satisfies RouteObject[];
