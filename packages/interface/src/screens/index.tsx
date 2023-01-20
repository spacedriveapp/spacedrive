import { Navigate, Route, RouteProps } from 'react-router-dom';
import { lazyEl } from '~/util';

import settingsScreens from './settings';

const routes: RouteProps[] = [
	{
		index: true,
		element: <Navigate to="overview" relative="path" />
	},
	{
		path: 'overview',
		element: lazyEl(() => import('./Overview'))
	},
	{ path: 'content', element: lazyEl(() => import('./Content')) },
	{ path: 'photos', element: lazyEl(() => import('./Photos')) },
	{ path: 'debug', element: lazyEl(() => import('./Debug')) },
	{ path: 'location/:id', element: lazyEl(() => import('./LocationExplorer')) },
	{ path: 'tag/:id', element: lazyEl(() => import('./TagExplorer')) },
	{
		path: 'settings',
		element: lazyEl(() => import('./settings/Layout')),
		children: settingsScreens
	}
];

export default (
	<>
		{routes.map((route) => (
			<Route key={route.path} {...route} />
		))}
	</>
);
