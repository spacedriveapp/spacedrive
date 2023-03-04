import { RouteObject } from "react-router";
import { lazyEl } from "~/util";

export default [
	{ path: 'about', element: lazyEl(() => import('./about')) },
	{ path: 'changelog', element: lazyEl(() => import('./changelog')) },
	{ path: 'dependencies', element: lazyEl(() => import('./dependencies')) },
	{ path: 'support', element: lazyEl(() => import('./support')) },
] satisfies RouteObject[]
