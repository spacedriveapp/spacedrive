import clsx from 'clsx';
import { PropsWithChildren, useEffect } from 'react';
import { MacTrafficLights } from '~/components';
import { useOperatingSystem, useShowControls } from '~/hooks';
import { useWindowState } from '~/hooks/useWindowState';

import Footer from './Footer';
import LibrariesDropdown from './LibrariesDropdown';

export default (props: PropsWithChildren) => {
	const os = useOperatingSystem();
	const showControls = useShowControls();
	const windowState = useWindowState();

	//prevent sidebar scrolling with keyboard
	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			const arrows = ['ArrowUp', 'ArrowDown'];
			if (arrows.includes(e.key)) {
				e.preventDefault();
			}
		};
		document.addEventListener('keydown', handleKeyDown);
		return () => document.removeEventListener('keydown', handleKeyDown);
	}, []);

	return (
		<div
			className={clsx(
				'relative flex min-h-full w-44 shrink-0 grow-0 flex-col gap-2.5 border-r border-sidebar-divider bg-sidebar px-2.5 pb-2 transition-[padding-top] ease-linear motion-reduce:transition-none',
				os === 'macOS' && windowState.isFullScreen
					? '-mt-2 pt-[8.75px] duration-100'
					: 'pt-2.5 duration-75',
				os === 'macOS' || showControls.transparentBg
					? 'bg-opacity-[0.65]'
					: 'bg-opacity-[1]'
			)}
		>
			{showControls.isEnabled && <MacTrafficLights className="z-50 mb-1" />}

			{os === 'macOS' && (
				<div
					data-tauri-drag-region
					className={clsx(
						'w-full shrink-0 transition-[height] ease-linear motion-reduce:transition-none',
						windowState.isFullScreen ? 'h-0 duration-100' : 'h-4 duration-75'
					)}
				/>
			)}
			<LibrariesDropdown />
			<div className="no-scrollbar mask-fade-out flex grow flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10">
				{props.children}
				<div className="grow" />
			</div>
			<Footer />
		</div>
	);
};
