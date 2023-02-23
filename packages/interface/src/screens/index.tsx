import { RouteObject } from 'react-router-dom';
import { lazyEl } from '~/util';
import settingsScreens from './settings';

const screens: RouteObject[] = [
	{
		path: 'overview',
		element: lazyEl(() => import('./Overview'))
	},
	{ path: 'people', element: lazyEl(() => import('./People')) },
	{ path: 'media', element: lazyEl(() => import('./Media')) },
	{ path: 'spaces', element: lazyEl(() => import('./Spaces')) },
	{ path: 'debug', element: lazyEl(() => import('./Debug')) },
	{ path: 'location/:id', element: lazyEl(() => import('./LocationExplorer')) },
	{ path: 'tag/:id', element: lazyEl(() => import('./TagExplorer')) },
	{
		path: 'settings',
		element: lazyEl(() => import('./settings/Layout')),
		children: settingsScreens
	},
	{ path: '*', element: lazyEl(() => import('./NotFound')) }
];

export default screens;
