import { RouteObject } from "react-router";
import { lazyEl } from "~/util";

export default [
	{ path: 'general', element: lazyEl(() => import('./general')) },
	{ path: 'appearance', element: lazyEl(() => import('./appearance')) },
	{ path: 'keybindings', element: lazyEl(() => import('./keybindings')) },
	{ path: 'extensions', element: lazyEl(() => import('./extensions')) },
	{ path: 'privacy', element: lazyEl(() => import('./privacy')) },
] satisfies RouteObject[]
