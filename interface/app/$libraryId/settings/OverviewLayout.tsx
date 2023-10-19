import clsx from 'clsx';
import { Outlet } from 'react-router';
import { useOperatingSystem } from '~/hooks';
import { useWindowState } from '~/hooks/useWindowState';

export const Component = () => {
	const os = useOperatingSystem();
	const windowState = useWindowState();

	return (
		<div
			className={clsx(
				'custom-scroll page-scroll relative flex h-full max-h-screen w-full grow-0',
				os === 'macOS' && windowState.isMaximized ? 'pt-1' : 'pt-8'
			)}
		>
			<div className="flex w-full max-w-4xl flex-col space-y-6 px-12 pb-5 pt-2">
				<Outlet />
				<div className="block h-4 shrink-0" />
			</div>
		</div>
	);
};
