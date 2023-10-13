import clsx from 'clsx';
import { useLayoutEffect, useState } from 'react';
import { ModifierKeys, Popover, Tooltip, usePopover } from '@sd/ui';
import { ExplorerLayout } from '~/../packages/client/src';
import { useKeybind, useKeyMatcher, useOperatingSystem } from '~/hooks';

import { useExplorerContext } from '../Explorer/Context';
import TopBarButton from './TopBarButton';
import TopBarMobile from './TopBarMobile';

export interface ToolOption {
	icon: JSX.Element;
	onClick?: () => void;
	individual?: boolean;
	toolTipLabel: string;
	toolTipClassName?: string;
	topBarActive?: boolean;
	popOverComponent?: JSX.Element;
	showAtResolution: ShowAtResolution;
	keybinds?: Array<String | ModifierKeys>;
}

export type ShowAtResolution = 'sm:flex' | 'md:flex' | 'lg:flex' | 'xl:flex' | '2xl:flex';
interface TopBarChildrenProps {
	options?: ToolOption[][];
}

export const TOP_BAR_ICON_STYLE = 'm-0.5 w-[18px] h-[18px] text-ink-dull';

export default ({ options }: TopBarChildrenProps) => {
	const [windowSize, setWindowSize] = useState(0);
	const explorer = useExplorerContext();
	const os = useOperatingSystem();
	const toolsNotSmFlex = options
		?.flatMap((group) => group)
		.filter((t) => t.showAtResolution !== 'sm:flex');
	const metaCtrlKey = useKeyMatcher('Meta').key;

	const layoutKeybinds = [
		{ key: '1', mode: 'grid' },
		{ key: '2', mode: 'list' },
		{ key: '3', mode: 'media' }
	];

	layoutKeybinds.forEach(({ key, mode }) => {
		useKeybind([metaCtrlKey, key], (e) => {
			e.stopPropagation();
			explorer.settingsStore.layoutMode = mode as ExplorerLayout;
		});
	});

	useLayoutEffect(() => {
		const handleResize = () => {
			setWindowSize(window.innerWidth);
		};
		window.addEventListener('resize', handleResize);
		handleResize();
		return () => window.removeEventListener('resize', handleResize);
	}, []);

	return (
		<div data-tauri-drag-region={os === 'macOS'} className="flex justify-end flex-1">
			<div data-tauri-drag-region={os === 'macOS'} className={`flex gap-0`}>
				{options?.map((group, groupIndex) =>
					group.map((option, index) => (
						<ToolGroup
							option={option}
							index={index}
							group={group}
							groupIndex={groupIndex}
							options={options}
							key={`${groupIndex}_${index}`}
						/>
					))
				)}
			</div>
			<TopBarMobile
				toolOptions={options}
				className={
					windowSize <= 1279 && (toolsNotSmFlex?.length as number) > 0 ? 'flex' : 'hidden'
				}
			/>
		</div>
	);
};

function ToolGroup({
	option,
	index,
	groupIndex,
	options,
	group
}: {
	option: ToolOption;
	options: ToolOption[][];
	group: ToolOption[];
	index: number;
	groupIndex: number;
}) {
	const {
		icon,
		onClick,
		popOverComponent,
		toolTipLabel,
		topBarActive,
		individual,
		showAtResolution,
		keybinds,
		toolTipClassName
	} = option;

	const groupCount = options.length;
	const roundingCondition = individual
		? 'both'
		: index === 0
		? 'left'
		: index === group.length - 1
		? 'right'
		: 'none';

	const popover = usePopover();
	const os = useOperatingSystem();

	return (
		<div
			data-tauri-drag-region={os === 'macOS'}
			key={toolTipLabel}
			className={clsx([showAtResolution], [individual && 'mx-1'], `hidden items-center`)}
		>
			<>
				{popOverComponent ? (
					<Popover
						popover={popover}
						className="focus:outline-none"
						trigger={
							<TopBarButton
								rounding={roundingCondition}
								active={topBarActive}
								onClick={onClick}
							>
								<Tooltip
									keybinds={keybinds}
									tooltipClassName={toolTipClassName}
									label={toolTipLabel}
								>
									{icon}
								</Tooltip>
							</TopBarButton>
						}
					>
						<div className="block min-w-[250px] max-w-[500px]">{popOverComponent}</div>
					</Popover>
				) : (
					<TopBarButton
						rounding={roundingCondition}
						active={topBarActive}
						onClick={onClick ?? undefined}
					>
						<Tooltip
							keybinds={keybinds}
							tooltipClassName={toolTipClassName}
							label={toolTipLabel}
						>
							{icon}
						</Tooltip>
					</TopBarButton>
				)}
			</>
			{index + 1 === group.length && groupIndex + 1 !== groupCount && (
				<div
					data-tauri-drag-region={os === 'macOS'}
					className="mx-4 h-[15px] w-0 border-l border-zinc-600"
				/>
			)}
		</div>
	);
}
