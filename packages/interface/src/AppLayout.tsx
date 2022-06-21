import clsx from 'clsx';
import React, { useContext } from 'react';
import { Outlet } from 'react-router-dom';

import { AppPropsContext } from './AppPropsContext';
import { Sidebar } from './components/file/Sidebar';

export function AppLayout() {
	const appProps = useContext(AppPropsContext);

	const isWindowRounded = appProps?.platform === 'macOS';
	const hasWindowBorder = appProps?.platform !== 'browser' && appProps?.platform !== 'windows';

	return (
		<div
			onContextMenu={(e) => {
				// TODO: allow this on some UI text at least / disable default browser context menu
				e.preventDefault();
				return false;
			}}
			className={clsx(
				'flex flex-row h-screen overflow-hidden text-gray-900 select-none dark:text-white',
				isWindowRounded && 'rounded-xl',
				hasWindowBorder && 'border border-gray-200 dark:border-gray-500'
			)}
		>
			<Sidebar />
			<div className="relative flex w-full h-screen max-h-screen bg-white dark:bg-gray-650">
				<Outlet />
			</div>
		</div>
	);
}
