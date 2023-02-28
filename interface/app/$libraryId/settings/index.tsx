import { RouteObject } from 'react-router-dom';
import { lazyEl } from '~/util';
import clientRoutes from './client';
import libraryRoutes from './library';
import nodeRoutes from './node';
import resourcesRoutes from './resources';

export default [
	{
		path: 'client',
		element: lazyEl(() => import('./OverviewLayout')),
		children: clientRoutes
	},
	{
		path: 'node',
		element: lazyEl(() => import('./OverviewLayout')),
		children: nodeRoutes
	},
	{
		path: 'library',
		children: libraryRoutes
	},
	{
		path: 'resources',
		element: lazyEl(() => import('./OverviewLayout')),
		children: resourcesRoutes
	}
] satisfies RouteObject[];
