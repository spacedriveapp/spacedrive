import { forwardRef } from 'react';
import SearchBar from './SearchBar';

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
	return (
		<div
			data-tauri-drag-region
			className="
				duration-250 top-bar-blur absolute left-0 top-0 z-50 flex
				h-[46px] w-full flex-row items-center justify-center overflow-hidden
				border-b border-sidebar-divider bg-app/90 px-5
				transition-[background-color,border-color] ease-out
			"
		>
			<div className="flex-1" />
			<SearchBar />
			<div className="flex-1" ref={ref} />
		</div>
	);
});

export default TopBar;
