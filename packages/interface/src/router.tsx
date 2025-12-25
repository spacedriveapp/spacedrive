import { createBrowserRouter } from "react-router-dom";
import { Overview } from "./routes/overview";
import { ExplorerView } from "./components/Explorer";
import { ExplorerLayout } from "./Explorer";
import { JobsScreen } from "./components/JobManager";
import { DaemonManager } from "./routes/DaemonManager";
import { TagView } from "./routes/tag";
import { FileKindsView } from "./routes/file-kinds";

/**
 * Router routes configuration (without router instance)
 */
export const explorerRoutes = [
	{
		path: "/",
		element: <ExplorerLayout />,
		children: [
			{
				index: true,
				element: <Overview />,
			},
			{
				path: "explorer",
				element: <ExplorerView />,
			},
			{
				path: "favorites",
				element: (
					<div className="flex items-center justify-center h-full text-ink">
						Favorites (coming soon)
					</div>
				),
			},
			{
				path: "recents",
				element: (
					<div className="flex items-center justify-center h-full text-ink">
						Recents (coming soon)
					</div>
				),
			},
			{
				path: "file-kinds",
				element: <FileKindsView />,
			},
			{
				path: "tag/:tagId",
				element: <TagView />,
			},
			{
				path: "search",
				element: (
					<div className="flex items-center justify-center h-full text-ink">
						Search (coming soon)
					</div>
				),
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
];

/**
 * Router for the main Explorer interface
 */
export function createExplorerRouter(): ReturnType<typeof createBrowserRouter> {
	return createBrowserRouter(explorerRoutes);
}
