import { Folder } from '@sd/ui';
import { forwardRef } from 'react';
import { NavigationButtons } from './NavigationButtons';
import SearchBar from './SearchBar';
import { useExplorerStore } from '~/hooks';

export interface ToolOption {
	icon: JSX.Element;
	onClick?: () => void;
	individual?: boolean;
	toolTipLabel: string;
	topBarActive?: boolean;
	popOverComponent?: JSX.Element;
	showAtResolution: ShowAtResolution;
}

export type ShowAtResolution = 'sm:flex' | 'md:flex' | 'lg:flex' | 'xl:flex' | '2xl:flex';

export const TOP_BAR_ICON_STYLE = 'm-0.5 w-5 h-5 text-ink-dull';
export const TOP_BAR_HEIGHT = 46;

const TopBar = forwardRef<HTMLDivElement>((_, ref) => {
	const explorerStore = useExplorerStore();
	return (
		<div
			data-tauri-drag-region
			className="
				duration-250 top-bar-blur absolute left-0 top-0 z-50 flex
				h-[46px] w-full flex-row items-center justify-center overflow-hidden
				border-b border-sidebar-divider bg-app/90 px-3.5
				transition-[background-color,border-color] ease-out
			"
		>
			<div data-tauri-drag-region className='flex flex-1 flex-row items-center'>
				<NavigationButtons />
				{explorerStore.topBarActiveDirectory && <div className=' m-3 flex  items-center'>
					<Folder className='mr-2 inline-block' />
					<span className='mt-[1px] text-sm font-medium'>{explorerStore.topBarActiveDirectory.name}</span>
				</div>}
			</div>
			<SearchBar />
			<div className="flex-1" ref={ref} />
		</div>
	);
});

export default TopBar;
