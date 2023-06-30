import clsx from 'clsx';
import { Suspense } from 'react';
import { Navigate, Outlet } from 'react-router-dom';
import {
	ClientContextProvider,
	LibraryContextProvider,
	initPlausible,
	useClientContext,
	usePlausiblePageViewMonitor
} from '@sd/client';
import { useRootContext } from '~/app/RootContext';
import { LibraryIdParamsSchema } from '~/app/route-schemas';
import { useOperatingSystem, useZodRouteParams } from '~/hooks';
import { usePlatform } from '~/util/Platform';
import { QuickPreviewContextProvider } from '../Explorer/QuickPreview/Context';
import Sidebar from './Sidebar';
import Toasts from './Toasts';

const Layout = () => {
	const { libraries, library } = useClientContext();
	const os = useOperatingSystem();
	const transparentBg = window.location.search.includes('transparentBg');

	initPlausible({
		platformType: usePlatform().platform === 'tauri' ? 'desktop' : 'web'
	});

	const { rawPath } = useRootContext();

	usePlausiblePageViewMonitor({ currentPath: rawPath });

	if (library === null && libraries.data) {
		const firstLibrary = libraries.data[0];

		if (firstLibrary) return <Navigate to={`/${firstLibrary.uuid}/overview`} replace />;
		else return <Navigate to="/" replace />;
	}

	return (
		<div
			className={clsx(
				// App level styles
				'flex h-screen cursor-default select-none overflow-hidden text-ink',
				os === 'browser' && 'border-t border-app-line/50',
				// os === 'browser' && !transparentBg && 'bg-app',
				os === 'macOS' && 'has-blur-effects rounded-[10px]',
				os !== 'browser' && os !== 'windows' && 'border border-app-frame'
			)}
			onContextMenu={(e) => {
				// TODO: allow this on some UI text at least / disable default browser context menu
				e.preventDefault();
				return false;
			}}
		>
			<Sidebar />
			<div
				className={clsx(
					'relative flex w-full overflow-hidden',
					transparentBg ? 'bg-app/80' : 'bg-app'
				)}
			>
				{library ? (
					<QuickPreviewContextProvider>
						<LibraryContextProvider library={library}>
							<Suspense
								fallback={
									<div
										className={clsx(
											'h-screen w-screen',
											transparentBg ? 'bg-app/80' : 'bg-app'
										)}
									/>
								}
							>
								<Outlet />
							</Suspense>
						</LibraryContextProvider>
					</QuickPreviewContextProvider>
				) : (
					<h1 className="p-4 text-white">
						Please select or create a library in the sidebar.
					</h1>
				)}
			</div>
			<Toasts />
		</div>
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
