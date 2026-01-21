import { createBrowserRouter } from "react-router-dom";
import { Overview } from "./routes/overview";
import { ExplorerView } from "./routes/explorer";
import { ShellLayout } from "./ShellLayout";
import { JobsScreen } from "./components/JobManager";
import { DaemonManager } from "./routes/daemon";
import { TagView } from "./routes/tag";
import { FileKindsView } from "./routes/file-kinds";
import { RecentsView } from "./routes/explorer/views/RecentsView";

/**
 * Router routes configuration (without router instance)
 */
export const explorerRoutes = [
	{
		path: "/",
		element: <ShellLayout />,
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
				element: <RecentsView />,
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