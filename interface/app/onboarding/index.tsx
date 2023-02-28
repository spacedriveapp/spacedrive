import { Navigate, RouteObject } from 'react-router';
import CreatingLibrary from './creating-library';
import MasterPassword from './master-password';
import NewLibrary from './new-library';
import Privacy from './privacy';
import Start from './start';

export default [
	{
		index: true,
		element: <Navigate to="start" />
	},
	{
		element: <Start />,
		path: 'start'
	},
	{
		element: <NewLibrary />,
		path: 'new-library'
	},
	{
		element: <MasterPassword />,
		path: 'master-password'
	},
	{
		element: <Privacy />,
		path: 'privacy'
	},
	{
		element: <CreatingLibrary />,
		path: 'creating-library'
	}
] satisfies RouteObject[];
