import clsx from 'clsx';
import { Suspense, useEffect, useMemo, useRef } from 'react';
import { Navigate, Outlet, useNavigate } from 'react-router-dom';
import {
	ClientContextProvider,
	configureAnalyticsProperties,
	LibraryContextProvider,
	useBridgeQuery,
	useClientContext,
	usePlausibleEvent,
	usePlausiblePageViewMonitor,
	usePlausiblePingMonitor
} from '@sd/client';
import { useRootContext } from '~/app/RootContext';
import { LibraryIdParamsSchema } from '~/app/route-schemas';
import ErrorFallback, { BetterErrorBoundary } from '~/ErrorFallback';
import {
	useDeeplinkEventHandler,
	useKeybindEventHandler,
	useOperatingSystem,
	useRedirectToNewLocation,
	useShowControls,
	useWindowState,
	useZodRouteParams
} from '~/hooks';
import { usePlatform } from '~/util/Platform';

import { DragOverlay } from '../Explorer/DragOverlay';
import { QuickPreviewContextProvider } from '../Explorer/QuickPreview/Context';
import CMDK from './CMDK';
import { LayoutContext } from './Context';
import { DndContext } from './DndContext';
import Sidebar from './Sidebar';

const Layout = () => {
	useRedirectToNewLocation();

	const { libraries, library } = useClientContext();
	const os = useOperatingSystem();
	const showControls = useShowControls();
	const windowState = useWindowState();

	useKeybindEventHandler(library?.uuid);
	useDeeplinkEventHandler();

	const layoutRef = useRef<HTMLDivElement>(null);

	const ctxValue = useMemo(() => ({ ref: layoutRef }), [layoutRef]);

	usePlausible();
	useUpdater();

	if (library === null && libraries.data) {
		const firstLibrary = libraries.data[0];

		if (firstLibrary) return <Navigate to={`/${firstLibrary.uuid}`} replace />;
		else return <Navigate to="./" replace />;
	}

	return (
		<LayoutContext.Provider value={ctxValue}>
			<div
				ref={layoutRef}
				className={clsx(
					// App level styles
					'flex h-screen select-none overflow-hidden text-ink',
					os === 'macOS' && [
						'has-blur-effects',
						!windowState.isFullScreen &&
							'frame rounded-[10px] border border-transparent'
					]
				)}
				onContextMenu={(e) => {
					// TODO: allow this on some UI text at least / disable default browser context menu
					e.preventDefault();
				}}
			>
				<DndContext>
					<Sidebar />
					<div
						className={clsx(
							'relative flex w-full overflow-hidden',
							showControls.transparentBg ? 'bg-app/80' : 'bg-app'
						)}
					>
						<BetterErrorBoundary FallbackComponent={ErrorFallback}>
							{library ? (
								<QuickPreviewContextProvider>
									<LibraryContextProvider library={library}>
										<Suspense
											fallback={<div className="h-screen w-screen bg-app" />}
										>
											<Outlet />
											<CMDK />
											<DragOverlay />
										</Suspense>
									</LibraryContextProvider>
								</QuickPreviewContextProvider>
							) : (
								<h1 className="p-4 text-white">
									Please select or create a library in the sidebar.
								</h1>
							)}
						</BetterErrorBoundary>
					</div>
				</DndContext>
			</div>
		</LayoutContext.Provider>
	);
};

export const Component = () => {
	const { libraryId } = useZodRouteParams(LibraryIdParamsSchema);

	return (
		<ClientContextProvider currentLibraryId={libraryId ?? null}>
			<Layout />
		</ClientContextProvider>
	);
};

function useUpdater() {
	const alreadyChecked = useRef(false);

	const { updater } = usePlatform();
	const navigate = useNavigate();

	useEffect(() => {
		if (alreadyChecked.current || !updater) return;

		updater.runJustUpdatedCheck(() => navigate('settings/resources/changelog'));

		if (import.meta.env.PROD) updater.checkForUpdate();
		alreadyChecked.current = true;
	}, [updater, navigate]);
}

function usePlausible() {
	const { rawPath } = useRootContext();
	const { platform } = usePlatform();
	const { data: buildInfo } = useBridgeQuery(['buildInfo']) ?? {};

	usePlausiblePageViewMonitor({ currentPath: rawPath });
	usePlausiblePingMonitor({ currentPath: rawPath });

	const plausibleEvent = usePlausibleEvent();

	useEffect(() => {
		configureAnalyticsProperties({
			buildInfo,
			platformType: platform === 'tauri' ? 'desktop' : 'web'
		});
	}, [platform, buildInfo]);

	useEffect(() => {
		const interval = setInterval(
			() => {
				// ping every 10 minutes -- this just tells us that Spacedrive is running and helps us gauge the amount of active users we have.
				plausibleEvent({ event: { type: 'ping' } });
			},
			10 * 60 * 1_000
		);

		return () => clearInterval(interval);
	}, [plausibleEvent]);
}
