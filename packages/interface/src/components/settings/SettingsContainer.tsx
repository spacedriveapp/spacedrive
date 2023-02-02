import clsx from 'clsx';
import { PropsWithChildren } from 'react';
import { useOperatingSystem } from '../../hooks/useOperatingSystem';

export const SettingsContainer = ({ children }: PropsWithChildren) => {
	const os = useOperatingSystem();

	return (
		<>
			{os !== 'browser' ? (
				<div data-tauri-drag-region className="h-5 w-full" />
			) : (
				<div className="h-5" />
			)}
			<div className="custom-scroll page-scroll flex h-full max-h-screen w-full flex-grow-0">
				<div className={clsx('flex w-full max-w-4xl flex-grow flex-col space-y-6 px-12 pt-2 pb-5')}>
					{children}
					<div className="block h-20" />
				</div>
			</div>
		</>
	);
};
