import { RouteObject } from 'react-router-dom';
import clientRoutes from './client';
import libraryRoutes from './library';
import nodeRoutes from './node';
import OverviewContainer from './OverviewContainer';
import resourcesRoutes from './resources';

export default [
	{
		path: 'client',
		element: <OverviewContainer/>,
		children: clientRoutes
	},
	{
		path: 'node',
		element: <OverviewContainer/>,
		children: nodeRoutes
	},
	{
		path: 'library',
		children: libraryRoutes
	},
	{
		path: 'resources',
		element: <OverviewContainer/>,
		children: resourcesRoutes
	}
] satisfies RouteObject[];
