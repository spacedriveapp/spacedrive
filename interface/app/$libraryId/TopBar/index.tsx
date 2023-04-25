import { CaretLeft, CaretRight } from 'phosphor-react';
import { forwardRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { Tooltip } from '@sd/ui';
import { useSearchStore } from '~/hooks/useSearchStore';
import SearchBar from './SearchBar';
import TopBarButton from './TopBarButton';

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
	const navigate = useNavigate();
	const { isFocused } = useSearchStore();

	return (
		<div
			data-tauri-drag-region
			className="
				duration-250 top-bar-blur absolute top-0 left-0 z-50 flex
				h-[46px] w-full flex-row items-center justify-center overflow-hidden
				border-b border-sidebar-divider bg-app/90 px-5
				transition-[background-color,border-color] ease-out
			"
		>
			<div data-tauri-drag-region className="flex flex-1">
				<Tooltip label="Navigate back">
					<TopBarButton onClick={() => navigate(-1)} disabled={isFocused}>
						<CaretLeft weight="bold" className={TOP_BAR_ICON_STYLE} />
					</TopBarButton>
				</Tooltip>
				<Tooltip label="Navigate forward">
					<TopBarButton onClick={() => navigate(1)} disabled={isFocused}>
						<CaretRight weight="bold" className={TOP_BAR_ICON_STYLE} />
					</TopBarButton>
				</Tooltip>
			</div>

			<SearchBar />

			<div className="flex-1" ref={ref} />
		</div>
	);
});

export default TopBar;
