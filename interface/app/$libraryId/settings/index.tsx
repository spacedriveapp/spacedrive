import { RouteObject } from 'react-router-dom';

import clientRoutes from './client';
import libraryRoutes from './library';
import nodeRoutes from './node';
import resourcesRoutes from './resources';

export default [
	{
		path: 'client',
		lazy: () => import('./OverviewLayout'),
		children: clientRoutes
	},
	{
		path: 'node',
		lazy: () => import('./OverviewLayout'),
		children: nodeRoutes
	},
	{
		path: 'library',
		children: libraryRoutes
	},
	{
		path: 'resources',
		lazy: () => import('./OverviewLayout'),
		children: resourcesRoutes
	}
] satisfies RouteObject[];
