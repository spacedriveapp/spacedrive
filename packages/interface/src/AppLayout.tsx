import clsx from 'clsx';
import { Outlet } from 'react-router-dom';

import { Sidebar } from './components/layout/Sidebar';
import { useOperatingSystem } from './hooks/useOperatingSystem';

export function AppLayout() {
	const os = useOperatingSystem();

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
				<Outlet />
			</div>
		</div>
	);
}
