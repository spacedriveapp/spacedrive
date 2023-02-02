import clsx from 'clsx';
import { Suspense } from 'react';
import { Outlet } from 'react-router-dom';
import { useCurrentLibrary } from '@sd/client';
import { Sidebar } from '~/components/layout/Sidebar';
import { Toasts } from '~/components/primitive/Toasts';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';

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
				<Suspense fallback={<div className="bg-app h-screen w-screen" />}>
					<Outlet />
				</Suspense>
			</div>
			<Toasts />
		</div>
	);
}
