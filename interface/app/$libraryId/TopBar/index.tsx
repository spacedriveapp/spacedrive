import { platform } from 'os';
import clsx from 'clsx';
import type { Ref } from 'react';
import { useOperatingSystem } from '~/hooks';

import { useExplorerStore } from '../Explorer/store';
import { NavigationButtons } from './NavigationButtons';
import SearchBar from './SearchBar';

export const TOP_BAR_HEIGHT = 46;

interface Props {
	leftRef?: Ref<HTMLDivElement>;
	rightRef?: Ref<HTMLDivElement>;
	noSearch?: boolean;
}

const TopBar = (props: Props) => {
	const transparentBg = window.location.search.includes('transparentBg');

	const { isDragging } = useExplorerStore();
	const os = useOperatingSystem();

	return (
		<div
			data-tauri-drag-region={os === 'macOS'}
			style={{ height: TOP_BAR_HEIGHT }}
			className={clsx(
				'top-bar-blur absolute inset-x-0 z-50 flex items-center gap-3.5 overflow-hidden border-b !border-sidebar-divider px-3.5',
				'duration-250 transition-[background-color,border-color] ease-out',
				isDragging && 'pointer-events-none',
				transparentBg ? 'bg-app/0' : 'bg-app/90'
			)}
		>
			<div
				data-tauri-drag-region={os === 'macOS'}
				className="flex flex-1 items-center gap-3.5 overflow-hidden"
			>
				<NavigationButtons />
				<div ref={props.leftRef} className="overflow-hidden" />
			</div>

			{!props.noSearch && <SearchBar />}

			<div ref={props.rightRef} className={clsx(!props.noSearch && 'flex-1')} />
		</div>
	);
};

export default TopBar;
