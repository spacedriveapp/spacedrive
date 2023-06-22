import { RouteObject } from 'react-router-dom';
import { z } from 'zod';
import settingsRoutes from './settings';

// Routes that should be contained within the standard Page layout
const pageRoutes: RouteObject = {
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
		{ path: 'sync', lazy: () => import('./sync') }
	]
};

export const LocationIdParamsSchema = z.object({ id: z.coerce.number() });
export type LocationIdParams = z.infer<typeof LocationIdParamsSchema>;

export const NodeIdParamsSchema = z.object({ id: z.string() });
export type NodeIdParams = z.infer<typeof NodeIdParamsSchema>;

// Routes that render the explorer and don't need padding and stuff
// provided by PageLayout
const explorerRoutes: RouteObject[] = [
	{
		path: 'location/:id',
		lazy: () => import('./location/$id'),
		loader: ({ params }) => LocationIdParamsSchema.parse(params)
	},
	{
		path: 'node/:id',
		lazy: () => import('./node/$id'),
		loader: ({ params }) => NodeIdParamsSchema.parse(params)
	},
	{
		path: 'tag/:id',
		lazy: () => import('./tag/$id'),
		loader: ({ params }) => LocationIdParamsSchema.parse(params)
	},
	{ path: 'search', lazy: () => import('./search') }
];

// Routes that should render with the top bar - pretty much everything except
// 404 and settings
const topBarRoutes: RouteObject = {
	lazy: () => import('./TopBar/Layout'),
	children: [pageRoutes, ...explorerRoutes]
};

export default [
	topBarRoutes,
	{
		path: 'settings',
		lazy: () => import('./settings/Layout'),
		children: settingsRoutes
	},
	{ path: '*', lazy: () => import('./404') }
] satisfies RouteObject[];
