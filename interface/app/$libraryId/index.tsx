import { redirect } from '@remix-run/router';
import { Navigate, type RouteObject } from 'react-router-dom';
import { useHomeDir } from '~/hooks/useHomeDir';
import { Platform } from '~/util/Platform';

import settingsRoutes from './settings';

// Routes that should be contained within the standard Page layout
const pageRoutes: RouteObject = {
	lazy: () => import('./PageLayout'),
	children: [
		{ path: 'people', lazy: () => import('./people') },
		{ path: 'media', lazy: () => import('./media') },
		{ path: 'spaces', lazy: () => import('./spaces') },
		{ path: 'debug', lazy: () => import('./debug') },
		{ path: 'sync', lazy: () => import('./sync') },
		{ path: 'p2p', lazy: () => import('./p2p') },
		{ path: 'cloud', lazy: () => import('./cloud') }
	]
};

// Routes that render the explorer and don't need padding and stuff
// provided by PageLayout
const explorerRoutes: RouteObject[] = [
	{ path: 'ephemeral/:id', lazy: () => import('./ephemeral') },
	{ path: 'location/:id', lazy: () => import('./location/$id') },
	{ path: 'node/:id', lazy: () => import('./node/$id') },
	{ path: 'tag/:id', lazy: () => import('./tag/$id') },
	{ path: 'network', lazy: () => import('./network') },
	{
		path: 'saved-search/:id',
		lazy: () => import('./saved-search/$id')
	}
];

// Routes that should render with the top bar - pretty much everything except
// 404 and settings
const topBarRoutes: RouteObject = {
	lazy: () => import('./TopBar/Layout'),
	children: [...explorerRoutes, pageRoutes]
};

export default (platform: Platform) =>
	[
		{
			index: true,
			Component: () => {
				const homeDir = useHomeDir();

				if (homeDir.data)
					return (
						<Navigate
							to={`ephemeral/0?${new URLSearchParams({ path: homeDir.data })}`}
						/>
					);

				return <Navigate to="network" />;
			},
			loader: async () => {
				if (!platform.userHomeDir) return null;
				const homeDir = await platform.userHomeDir();
				return redirect(`ephemeral/0?${new URLSearchParams({ path: homeDir })}`);
			}
		},
		topBarRoutes,
		{
			path: 'settings',
			lazy: () => import('./settings/Layout'),
			children: settingsRoutes
		},
		{ path: '*', lazy: () => import('./404') }
	] satisfies RouteObject[];
