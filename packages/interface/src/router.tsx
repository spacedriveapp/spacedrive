import { createBrowserRouter } from "react-router-dom";
import { JobsScreen } from "./components/JobManager";
import { DaemonManager } from "./routes/daemon";
import { ExplorerView } from "./routes/explorer";
import { FileKindsView } from "./routes/file-kinds";
import { Overview } from "./routes/overview";
import { TagView } from "./routes/tag";
import { ShellLayout } from "./ShellLayout";

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
          <div className="flex h-full items-center justify-center text-ink">
            Favorites (coming soon)
          </div>
        ),
      },
      {
        path: "recents",
        element: (
          <div className="flex h-full items-center justify-center text-ink">
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
          <div className="flex h-full items-center justify-center text-ink">
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
