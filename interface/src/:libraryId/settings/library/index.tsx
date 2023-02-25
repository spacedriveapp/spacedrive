import { RouteObject } from "react-router";
import { lazyEl } from "~/util";
import locationsRoutes from "./locations"

export default [
	{ path: 'contacts', element: lazyEl(() => import('./contacts')) },
	{ path: 'keys', element: lazyEl(() => import('./keys')) },
	{ path: 'security', element: lazyEl(() => import('./security')) },
	{ path: 'sharing', element: lazyEl(() => import('./sharing')) },
	{ path: 'sync', element: lazyEl(() => import('./sync')) },
	{ path: 'tags', element: lazyEl(() => import('./tags')) },
	{ path: 'general', element: lazyEl(() => import('./general')) },
	{ path: 'tags', element: lazyEl(() => import('./tags')) },
	{ path: 'nodes', element: lazyEl(() => import('./nodes')) },
	{
		path: 'locations',
		element: lazyEl(() => import('../SettingsSubPage')),
		children: locationsRoutes
	}
] satisfies RouteObject[]
