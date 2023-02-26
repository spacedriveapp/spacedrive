import clsx from 'clsx';
import { Suspense } from 'react';
import { Navigate, Outlet, useParams } from 'react-router-dom';
import { ClientContextProvider, LibraryContextProvider, useClientContext } from '@sd/client';
import { Toasts } from '~/components/Toasts';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import Sidebar from './Sidebar';

const Layout = () => {
	const { libraries, library } = useClientContext();

	const os = useOperatingSystem();

	if (library === null && libraries.data) {
		const firstLibrary = libraries.data[0];

		if (firstLibrary) return <Navigate to={`${firstLibrary.uuid}/overview`} />;
		else return <Navigate to="/" />;
	}

	return (
		<div
			className={clsx(
				// App level styles
				'text-ink flex h-screen cursor-default select-none overflow-hidden',
				os === 'browser' && 'bg-app border-app-line/50 border-t',
				os === 'macOS' && 'has-blur-effects rounded-[10px]',
				os !== 'browser' && os !== 'windows' && 'border-app-frame border'
			)}
			onContextMenu={(e) => {
				// TODO: allow this on some UI text at least / disable default browser context menu
				e.preventDefault();
				return false;
			}}
		>
			<Sidebar />
			<div className="relative flex w-full">
				{library ? (
					<LibraryContextProvider library={library}>
						<Suspense fallback={<div className="bg-app h-screen w-screen" />}>
							<Outlet />
						</Suspense>
					</LibraryContextProvider>
				) : (
					<h1 className="p-4 text-white">Please select or create a library in the sidebar.</h1>
				)}
			</div>
			<Toasts />
		</div>
	);
};

export default () => {
	const params = useParams<{ libraryId: string }>();

	return (
		<ClientContextProvider currentLibraryId={params.libraryId ?? null}>
			<Layout />
		</ClientContextProvider>
	);
};
