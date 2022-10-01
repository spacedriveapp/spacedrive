import { useCurrentLibrary } from '@sd/client';
import clsx from 'clsx';
import { Suspense } from 'react';
import { Outlet } from 'react-router-dom';

import { Sidebar } from './components/layout/Sidebar';
import { useOperatingSystem } from './hooks/useOperatingSystem';

export function AppLayout() {
	const { libraries } = useCurrentLibrary();
	const os = useOperatingSystem();

	// This will ensure nothing is rendered while the `useCurrentLibrary` hook navigates to the onboarding page. This prevents requests with an invalid library id being sent to the backend
	if (libraries?.length === 0) {
		return null;
	}

	return (
		<div
			onContextMenu={(e) => {
				// TODO: allow this on some UI text at least / disable default browser context menu
				e.preventDefault();
				return false;
			}}
			className={clsx(
				'flex flex-row h-screen overflow-hidden text-gray-900 select-none dark:text-white cursor-default',
				os === 'macOS' && 'rounded-xl',
				os !== 'browser' && os !== 'windows' && 'border border-gray-200 dark:border-gray-500'
			)}
		>
			<Sidebar />
			<div className="relative flex w-full h-screen max-h-screen bg-white dark:bg-gray-650">
				<Suspense fallback={<p>Loading...</p>}>
					<Outlet />
				</Suspense>
			</div>
		</div>
	);
}
