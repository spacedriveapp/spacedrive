import clsx from 'clsx';
import { Ref } from 'react';
import { useExplorerStore } from '../Explorer/store';
import { NavigationButtons } from './NavigationButtons';
import SearchBar from './SearchBar';

export const TOP_BAR_HEIGHT = 46;

interface Props {
	leftRef?: Ref<HTMLDivElement>;
	rightRef?: Ref<HTMLDivElement>;
}

const TopBar = (props: Props) => {
	const { isDragging } = useExplorerStore();

	return (
		<div
			data-tauri-drag-region
			className={clsx(
				'duration-250 top-bar-blur absolute left-0 top-0 z-50 flex',
				'h-[46px] w-full flex-row items-center justify-center overflow-hidden',
				'border-b border-sidebar-divider bg-app/90 px-3.5',
				'transition-[background-color,border-color] ease-out',
				isDragging && 'pointer-events-none'
			)}
		>
			<div data-tauri-drag-region className="flex flex-1 flex-row items-center">
				<NavigationButtons />
				<div ref={props.leftRef} />
			</div>
			<SearchBar />
			<div className="flex-1" ref={props.rightRef} />
		</div>
	);
};

export default TopBar;
