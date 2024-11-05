import { RouteObject } from 'react-router';

export default [
	{ path: 'general', lazy: () => import('./general') },
	{ path: 'account', lazy: () => import('./account') },
	{ path: 'appearance', lazy: () => import('./appearance') },
	{ path: 'keybindings', lazy: () => import('./keybindings') },
	{ path: 'privacy', lazy: () => import('./privacy') },
	{ path: 'backups', lazy: () => import('./backups') },
	{ path: 'network', lazy: () => import('./network/index') },
	{ path: 'network/debug', lazy: () => import('./network/debug') }
] satisfies RouteObject[];
