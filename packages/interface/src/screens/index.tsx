import { Navigate, Route, RouteProps } from 'react-router-dom';
import { lazyEl } from '~/util';
import settingsScreens from './settings';

const routes: RouteProps[] = [
	{
		index: true,
		element: <Navigate to="overview" relative="route" />
	},
	{
		path: 'overview',
		element: lazyEl(() => import('./Overview'))
	},
	{ path: 'people', element: lazyEl(() => import('./People')) },
	{ path: 'media', element: lazyEl(() => import('./Media')) },
	{ path: 'spaces', element: lazyEl(() => import('./Spaces')) },
	{ path: 'debug', element: lazyEl(() => import('./Debug')) },
	{ path: 'spacedrop', element: lazyEl(() => import('./Spacedrop')) },
	{ path: 'location/:id', element: lazyEl(() => import('./LocationExplorer')) },
	{ path: 'tag/:id', element: lazyEl(() => import('./TagExplorer')) },
	{
		path: 'settings',
		element: lazyEl(() => import('./settings/_Layout')),
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
