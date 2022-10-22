import { useCurrentLibrary } from '@sd/client';
import clsx from 'clsx';
import { Suspense } from 'react';
import { Outlet } from 'react-router-dom';

import { Sidebar } from './components/layout/Sidebar';
import { Toasts } from './components/primitive/Toasts';
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
			className={clsx(
				// App level styles
				'flex h-screen overflow-hidden text-ink select-none cursor-default',
				os === 'macOS' && 'rounded-[10px] has-blur-effects',
				os !== 'browser' && os !== 'windows' && 'border border-app-frame'
			)}
			onContextMenu={(e) => {
				// TODO: allow this on some UI text at least / disable default browser context menu
				e.preventDefault();
				return false;
			}}
		>
			<Sidebar />
			<div className="relative flex w-full">
				<Suspense>
					<Outlet />
				</Suspense>
			</div>
			<Toasts />
		</div>
	);
}
