import clsx from 'clsx';
import { PropsWithChildren } from 'react';
import { useOperatingSystem } from '../../hooks/useOperatingSystem';

export const SettingsContainer = ({ children }: PropsWithChildren) => {
	const os = useOperatingSystem();

	return (
		<>
			{os !== 'browser' ? (
				<div data-tauri-drag-region className="w-full h-5" />
			) : (
				<div className="h-5" />
			)}
			<div className="flex flex-grow-0 w-full h-full max-h-screen custom-scroll page-scroll">
				<div className={clsx('flex flex-col flex-grow w-full max-w-4xl space-y-6 pt-2 px-12 pb-5')}>
					{children}
					<div className="block h-20" />
				</div>
			</div>
		</>
	);
};
