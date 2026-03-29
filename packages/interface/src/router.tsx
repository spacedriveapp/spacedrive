import { createBrowserRouter, Navigate, Outlet } from "react-router-dom";
import { Overview } from "./routes/overview";
import { ExplorerView } from "./routes/explorer";
import { ShellLayout } from "./ShellLayout";
import { JobsScreen } from "./components/JobManager";
import { DaemonManager } from "./routes/daemon";
import { TagView } from "./routes/tag";
import { FileKindsView } from "./routes/file-kinds";
import { RecentsView } from "./routes/explorer/views/RecentsView";
import { SourcesHome } from "./routes/sources";
import { SourceDetail } from "./routes/sources/SourceDetail";
import { AdaptersScreen } from "./routes/sources/Adapters";
import { SpacebotProvider } from "./Spacebot/SpacebotContext";
import { SpacebotLayout } from "./Spacebot/SpacebotLayout";
import { ChatRoute } from "./Spacebot/routes/ChatRoute";
import { ConversationRoute } from "./Spacebot/routes/ConversationRoute";
import { TasksRoute } from "./Spacebot/routes/TasksRoute";
import { MemoriesRoute } from "./Spacebot/routes/MemoriesRoute";
import { AutonomyRoute } from "./Spacebot/routes/AutonomyRoute";
import { ScheduleRoute } from "./Spacebot/routes/ScheduleRoute";

/**
 * Spacebot wrapper component that provides the Spacebot context
 */
function SpacebotRoutes() {
	return (
		<SpacebotProvider>
			<Outlet />
		</SpacebotProvider>
	);
}

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
				path: "sources",
				element: <SourcesHome />,
			},
			{
				path: "sources/adapters",
				element: <AdaptersScreen />,
			},
			{
				path: "sources/:sourceId",
				element: <SourceDetail />,
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
				path: "spacebot",
				element: <SpacebotRoutes />,
				children: [
					{
						index: true,
						element: <Navigate to="/spacebot/chat" replace />,
					},
					{
						element: <SpacebotLayout />,
						children: [
							{
								path: "chat",
								children: [
									{
										index: true,
										element: <ChatRoute />,
									},
									{
										path: "new",
										element: <ChatRoute />,
									},
									{
										path: "conversation/*",
										element: <ConversationRoute />,
									},
								],
							},
							{
								path: "tasks",
								element: <TasksRoute />,
							},
							{
								path: "memories",
								element: <MemoriesRoute />,
							},
							{
								path: "autonomy",
								element: <AutonomyRoute />,
							},
							{
								path: "schedule",
								element: <ScheduleRoute />,
							},
						],
					},
				],
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
