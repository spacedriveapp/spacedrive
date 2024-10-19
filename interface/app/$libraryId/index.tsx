import { redirect } from 'react-router';
import { type RouteObject } from 'react-router-dom';
import { guessOperatingSystem } from '~/hooks';
import { Platform } from '~/util/Platform';

import settingsRoutes from './settings';

// Routes that should be contained within the standard Page layout
const pageRoutes: RouteObject = {
	lazy: () => import('./PageLayout'),
	children: [{ path: 'overview', lazy: () => import('./overview') }]
};

// Routes that render the explorer and don't need padding and stuff
// provided by PageLayout
const explorerRoutes: RouteObject[] = [
	{ path: 'recents', lazy: () => import('./recents') },
	{ path: 'favorites', lazy: () => import('./favorites') },
	// { path: 'labels', lazy: () => import('./labels') },
	{ path: 'search', lazy: () => import('./search') },
	{ path: 'ephemeral/:id', lazy: () => import('./ephemeral') },
	{ path: 'location/:id', lazy: () => import('./location/$id') },
	{ path: 'node/:id', lazy: () => import('./node/$id') },
	{ path: 'peer/:id', lazy: () => import('./peer/$id') },
	{ path: 'tag/:id', lazy: () => import('./tag/$id') },
	{ path: 'network', lazy: () => import('./network') },
	{ path: 'saved-search/:id', lazy: () => import('./saved-search/$id') }
];

function loadTopBarRoutes() {
	const os = guessOperatingSystem();
	if (os === 'windows') {
		return [
			...explorerRoutes,
			pageRoutes,
			{ path: 'settings', lazy: () => import('./settings/Layout'), children: settingsRoutes }
		];
	} else return [...explorerRoutes, pageRoutes];
}

// Routes that should render with the top bar - pretty much everything except
// 404 and settings, which are rendered only for Windows with top bar
const topBarRoutes: RouteObject = {
	lazy: () => import('./TopBar/Layout'),
	children: loadTopBarRoutes()
};

export default (platform: Platform) =>
	[
		{
			index: true,
			loader: async () => {
				try {
					if (platform.userHomeDir) {
						const homeDir = await platform.userHomeDir();
						return redirect(`ephemeral/0?${new URLSearchParams({ path: homeDir })}`, {
							replace: true
						});
					}
				} catch (e) {
					console.error('Failed to redirect to user home', e);
				}

				return redirect(`network`, {
					replace: true
				});
			}
		},
		topBarRoutes,
		{
			path: 'settings',
			lazy: () => import('./settings/Layout'),
			children: settingsRoutes
		},
		{
			path: 'auth',
			lazy: () => import('./Layout/auth'),
			children: []
		},
		{ path: '*', lazy: () => import('./404') }
	] satisfies RouteObject[];
