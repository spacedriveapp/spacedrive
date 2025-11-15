import { createBrowserRouter } from "react-router-dom";
import { Overview } from "./routes/overview";
import { ExplorerView } from "./components/explorer";
import { ExplorerLayout } from "./Explorer";
import { JobsScreen } from "./components/JobManager";
import { DaemonManager } from "./routes/DaemonManager";

/**
 * Router for the main Explorer interface
 */
export function createExplorerRouter() {
	return createBrowserRouter([
		{
			path: "/",
			element: <ExplorerLayout />,
			children: [
				{
					index: true,
					element: <Overview />,
				},
				{
					path: "location/:locationId",
					element: <ExplorerView />,
				},
				{
					path: "location/:locationId/*",
					element: <ExplorerView />,
				},
				{
					path: "favorites",
					element: <div className="flex items-center justify-center h-full text-ink">Favorites (coming soon)</div>,
				},
				{
					path: "recents",
					element: <div className="flex items-center justify-center h-full text-ink">Recents (coming soon)</div>,
				},
				{
					path: "tag/:tagId",
					element: <div className="flex items-center justify-center h-full text-ink">Tag view (coming soon)</div>,
				},
				{
					path: "search",
					element: <div className="flex items-center justify-center h-full text-ink">Search (coming soon)</div>,
				},
				{
					path: "jobs",
					element: <JobsScreen />,
				},
				{
					path: "daemon",
					element: <DaemonManager />,
				},
			],
		},
	]);
}
