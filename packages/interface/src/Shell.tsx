import {Dialogs, Toaster, TooltipProvider} from '@spacedrive/primitives';
import {ReactQueryDevtools} from '@tanstack/react-query-devtools';
import {useEffect, useRef, useState} from 'react';
import {RouterProvider} from 'react-router-dom';
import {DndProvider} from './components/DndProvider';
import {DaemonDisconnectedOverlay} from './components/overlays/DaemonDisconnectedOverlay';
import {DaemonStartupOverlay} from './components/overlays/DaemonStartupOverlay';
import {
	TabKeyboardHandler,
	TabManagerProvider,
	useTabManager
} from './components/TabManager';
import {usePlatform} from './contexts/PlatformContext';
import {ServerProvider} from './contexts/ServerContext';
import {
	SpacedriveProvider,
	useLibraryMutation,
	useSpacedriveClient,
	type SpacedriveClient
} from './contexts/SpacedriveContext';
import {useDaemonStatus} from './hooks/useDaemonStatus';
import {useLibraries} from './hooks/useLibraries';
import {useTheme} from './hooks/useTheme';
import {explorerRoutes} from './router';

interface ShellProps {
	client: SpacedriveClient;
}

function ThemeApplier() {
	useTheme();
	return null;
}

/** Starts discovery only after the desktop shell has rendered and connected. */
function DesktopStartupCoordinator() {
	const client = useSpacedriveClient();
	const platform = usePlatform();
	const {data: libraries} = useLibraries();
	const {mutateAsync: startDiscovery} =
		useLibraryMutation('indexing.startup');
	const [libraryId, setLibraryId] = useState(client.getCurrentLibraryId());
	const attempted = useRef(new Set<string>());

	useEffect(() => {
		const handleLibraryChange = (nextLibraryId: string) =>
			setLibraryId(nextLibraryId);
		client.on('library-changed', handleLibraryChange);
		return () => client.off('library-changed', handleLibraryChange);
	}, [client]);

	useEffect(() => {
		let cancelled = false;

		const begin = async () => {
			let selectedLibraryId = libraryId ?? client.getCurrentLibraryId();
			if (!selectedLibraryId) {
				selectedLibraryId =
					(await platform.getCurrentLibraryId?.()) ??
					libraries?.[0]?.id ??
					null;
			}
			if (
				!selectedLibraryId ||
				cancelled ||
				attempted.current.has(selectedLibraryId)
			) {
				return;
			}

			client.setCurrentLibrary(selectedLibraryId, false);
			setLibraryId(selectedLibraryId);
			attempted.current.add(selectedLibraryId);

			try {
				await startDiscovery({force: false});
			} catch (error) {
				attempted.current.delete(selectedLibraryId);
				console.error('Failed to start filesystem discovery:', error);
			}
		};

		void begin();
		return () => {
			cancelled = true;
		};
	}, [client, libraries, libraryId, platform, startDiscovery]);

	return null;
}

function ShellWithTabs() {
	const {router} = useTabManager();

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
	const {isConnected, isStarting} = daemonStatus;

	return (
		<>
			{isConnected ? (
				// Daemon connected - render full app
				<>
					<DesktopStartupCoordinator />
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

export function Shell({client}: ShellProps) {
	const platform = usePlatform();
	const isTauri = platform.platform === 'tauri';

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
