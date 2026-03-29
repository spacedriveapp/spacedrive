import { SpacedriveProvider, type SpacedriveClient } from "./contexts/SpacedriveContext";
import { ServerProvider } from "./contexts/ServerContext";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { RouterProvider } from "react-router-dom";
import { Dialogs, Toaster, TooltipProvider } from "@spaceui/primitives";
import { ShellLayout } from "./ShellLayout";
import { explorerRoutes } from "./router";
import { useDaemonStatus } from "./hooks/useDaemonStatus";
import { DaemonDisconnectedOverlay } from "./components/overlays/DaemonDisconnectedOverlay";
import { DaemonStartupOverlay } from "./components/overlays/DaemonStartupOverlay";
import { DndProvider } from "./components/DndProvider";
import {
	TabManagerProvider,
	TabKeyboardHandler,
	useTabManager,
} from "./components/TabManager";
import { usePlatform } from "./contexts/PlatformContext";
import { useTheme } from "./hooks/useTheme";

interface ShellProps {
	client: SpacedriveClient;
}

function ThemeApplier() {
	useTheme();
	return null;
}

function ShellWithTabs() {
	const { router } = useTabManager();

	return (
		<DndProvider>
			<ThemeApplier />
			<RouterProvider router={router} />
		</DndProvider>
	);
}

/**
 * Tauri-specific wrapper that prevents Shell from rendering until daemon is connected.
 * This avoids the connection storm where hundreds of queries try to execute before daemon is ready.
 */
function ShellWithDaemonCheck() {
	const daemonStatus = useDaemonStatus();
	const { isConnected, isStarting } = daemonStatus;

	return (
		<>
			{isConnected ? (
				// Daemon connected - render full app
				<>
					<TabManagerProvider routes={explorerRoutes}>
						<TabKeyboardHandler />
						<ShellWithTabs />
					</TabManagerProvider>
					<Dialogs />
					<Toaster />
					<ReactQueryDevtools
						initialIsOpen={false}
						buttonPosition="bottom-right"
					/>
				</>
			) : (
				// Daemon not connected - show appropriate overlay
				<>
					<DaemonStartupOverlay show={isStarting} />
					{!isStarting && (
						<DaemonDisconnectedOverlay
							daemonStatus={daemonStatus}
						/>
					)}
				</>
			)}
		</>
	);
}

export function Shell({ client }: ShellProps) {
	const platform = usePlatform();
	const isTauri = platform.platform === "tauri";

	return (
		<SpacedriveProvider client={client}>
			<ServerProvider>
				<TooltipProvider>
					{isTauri ? (
						// Tauri: Wait for daemon connection before rendering content
						<ShellWithDaemonCheck />
					) : (
						// Web: Render immediately (daemon connection handled differently)
						<>
							<TabManagerProvider routes={explorerRoutes}>
								<TabKeyboardHandler />
								<ShellWithTabs />
							</TabManagerProvider>
							<Dialogs />
							<Toaster />
							<ReactQueryDevtools
								initialIsOpen={false}
								buttonPosition="bottom-right"
							/>
						</>
					)}
				</TooltipProvider>
			</ServerProvider>
		</SpacedriveProvider>
	);
}