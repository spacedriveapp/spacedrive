import { type Ref } from 'react';
import { NavigationButtons } from './NavigationButtons';
import SearchBar from './SearchBar';

export const TOP_BAR_HEIGHT = 46;

interface Props {
	leftRef?: Ref<HTMLDivElement>;
	rightRef?: Ref<HTMLDivElement>;
	noSearch?: boolean;
}

const TopBar = (props: Props) => (
	<div
		data-tauri-drag-region
		style={{ height: TOP_BAR_HEIGHT }}
		className="
			duration-250 top-bar-blur absolute left-0 top-0 z-50 flex w-full
			flex-row items-center justify-between overflow-hidden border-b
			border-sidebar-divider bg-app/90 px-3.5
			transition-[background-color,border-color] ease-out
		"
	>
		<div data-tauri-drag-region className="flex min-w-0 flex-row items-center">
			<NavigationButtons />
			<div ref={props.leftRef} className="contents" />
		</div>
		{props.noSearch || <SearchBar />}
		<div ref={props.rightRef} className="contents" />
	</div>
);

export default TopBar;
