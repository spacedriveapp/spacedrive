import clsx from 'clsx';
import type { Ref } from 'react';
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
	const { isDragging } = useExplorerStore();

	return (
		<div
			data-tauri-drag-region
			style={{ height: TOP_BAR_HEIGHT }}
			className={clsx(
				'top-bar-blur absolute inset-x-0 z-50 flex items-center gap-3.5 overflow-hidden border-b !border-sidebar-divider bg-app/90 px-3.5',
				'duration-250 transition-[background-color,border-color] ease-out',
				isDragging && 'pointer-events-none'
			)}
		>
			<div
				data-tauri-drag-region
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
