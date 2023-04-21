import { RouteObject } from 'react-router-dom';
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
// Routes that should render with the top bar - pretty much everything except
// 404 and settings
const topBarRoutes: RouteObject = {
	lazy: () => import('./TopBar/Layout'),
	children: [
		pageRoutes,
		{ path: 'location/:id', lazy: () => import('./location/$id') },
		{ path: 'tag/:id', lazy: () => import('./tag/$id') }
	]
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
