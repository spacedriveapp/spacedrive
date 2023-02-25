import { RouteObject } from "react-router";
import { lazyEl } from "~/util";

export default [
	{ path: 'p2p', element: lazyEl(() => import('./p2p')) },
	{ path: 'libraries', element: lazyEl(() => import('./libraries')) },
	{ path: 'experimental', element: lazyEl(() => import('./experimental')) },
] satisfies RouteObject[]
