import clsx from 'clsx';
import { MacTrafficLights } from '~/components';
import { useOperatingSystem, useSearchStore } from '~/hooks';
import Contents from './Contents';
import Footer from './Footer';
import LibrariesDropdown from './LibrariesDropdown';
import { macOnly } from './helpers';

export default () => {
	const os = useOperatingSystem();
	const showControls = useSearchStore();
	const transparentBg = window.location.search.includes('transparentBg');

	return (
		<div
			className={clsx(
				'relative flex min-h-full w-44 shrink-0 grow-0 flex-col gap-2.5 border-r border-sidebar-divider bg-sidebar px-2.5 pb-2 pt-2.5',
				os === 'macOS' || transparentBg ? 'bg-opacity-[0.65]' : 'bg-opacity-[1]'
			)}
		>
			{showControls && <MacTrafficLights className="z-50 mb-1" />}
			{os === 'macOS' && <div data-tauri-drag-region className="h-5 w-full" />}
			<LibrariesDropdown />
			<Contents />
			<Footer />
		</div>
	);
};
