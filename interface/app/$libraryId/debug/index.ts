import { RouteObject } from 'react-router';

export const debugRoutes = [
	{ path: 'cache', lazy: () => import('./cache') },
	{ path: 'cloud', lazy: () => import('./cloud') },
	{ path: 'sync', lazy: () => import('./sync') },
	{ path: 'actors', lazy: () => import('./actors') },
	{
		path: 'p2p',
		lazy: () => import('./p2p'),
		children: [
			{
				path: 'overview',
				lazy: () => import('./p2p').then((m) => ({ Component: m.Overview }))
			},
			{
				path: 'remote',
				lazy: () => import('./p2p').then((m) => ({ Component: m.RemotePeers }))
			},
			{
				path: 'instances',
				lazy: () => import('./p2p').then((m) => ({ Component: m.Instances }))
			}
		]
	}
] satisfies RouteObject[];
