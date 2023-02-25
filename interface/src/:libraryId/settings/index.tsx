import { RouteObject } from 'react-router-dom';

import clientRoutes from "./client"
import nodeRoutes from "./node"
import libraryRoutes from "./library"
import infoRoutes from "./info"

export default[
	{
		path: "client", children: clientRoutes
	},
	{
		path: "node",
		children: nodeRoutes
	},
	{
		path: 'library',
		children: libraryRoutes
	},
	{
		path: "info",
		children: infoRoutes
	},
] satisfies RouteObject[];
