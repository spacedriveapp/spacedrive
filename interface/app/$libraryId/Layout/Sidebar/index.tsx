import clsx from 'clsx';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import Contents from './Contents';
import Footer from './Footer';
import LibrariesDropdown from './LibrariesDropdown';
import WindowControls from './WindowControls';
import { macOnly } from './helpers';

export default () => {
	const os = useOperatingSystem();
	return (
		<div
			className={clsx(
				'relative flex min-h-full w-44 shrink-0 grow-0 flex-col space-y-2.5 border-r border-sidebar-divider bg-sidebar px-2.5 pb-2',
				macOnly(os, 'bg-opacity-[0.65]')
			)}
		>
			<WindowControls />
			<LibrariesDropdown />
			<Contents />
			<Footer />
		</div>
	);
};
