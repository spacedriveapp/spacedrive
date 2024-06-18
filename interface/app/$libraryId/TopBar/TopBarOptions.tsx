import { Cards, Minus, Square, X } from '@phosphor-icons/react';
import { getCurrent, Window } from '@tauri-apps/api/window';
import clsx from 'clsx';
import { useCallback, useEffect, useLayoutEffect, useState } from 'react';
import { ModifierKeys, Popover, Tooltip, usePopover } from '@sd/ui';
import { useIsDark, useOperatingSystem } from '~/hooks';

import TopBarButton from './TopBarButton';
import TopBarMobile from './TopBarMobile';

const appWindow = new Window('main');
export interface ToolOption {
	icon: JSX.Element | ((props: { triggerOpen: () => void }) => JSX.Element);
	onClick?: () => void;
	individual?: boolean;
	toolTipLabel: string;
	toolTipClassName?: string;
	topBarActive?: boolean;
	popOverComponent?: JSX.Element | ((props: { triggerClose: () => void }) => JSX.Element);
	showAtResolution: ShowAtResolution;
	keybinds?: Array<String | ModifierKeys>;
}

export type ShowAtResolution = 'sm:flex' | 'md:flex' | 'lg:flex' | 'xl:flex' | '2xl:flex';
interface TopBarChildrenProps {
	options?: ToolOption[][];
}

export const TOP_BAR_ICON_CLASSLIST = 'm-0.5 w-[18px] h-[18px] text-ink-dull';

export default ({ options }: TopBarChildrenProps) => {
	const [windowSize, setWindowSize] = useState(0);
	const os = useOperatingSystem();
	const toolsNotSmFlex = options
		?.flatMap((group) => group)
		.filter((t) => t.showAtResolution !== 'sm:flex');

	useLayoutEffect(() => {
		const handleResize = () => {
			setWindowSize(window.innerWidth);
		};
		window.addEventListener('resize', handleResize);
		handleResize();
		return () => window.removeEventListener('resize', handleResize);
	}, []);

	return (
		<div data-tauri-drag-region={os === 'macOS'} className="flex flex-1 justify-end">
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
				{os === 'windows' && <WindowsControls windowSize={windowSize} />}
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
	const isDark = useIsDark();

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
									tooltipClassName={clsx('capitalize', toolTipClassName)}
									label={toolTipLabel}
								>
									{typeof icon === 'function'
										? icon({
												triggerOpen: () => popover.setOpen(true)
											})
										: icon}
								</Tooltip>
							</TopBarButton>
						}
					>
						<div className="block min-w-[250px] max-w-[500px]">
							{typeof popOverComponent === 'function'
								? popOverComponent({ triggerClose: () => popover.setOpen(false) })
								: popOverComponent}
						</div>
					</Popover>
				) : (
					<TopBarButton
						rounding={roundingCondition}
						active={topBarActive}
						onClick={onClick ?? undefined}
					>
						<Tooltip
							keybinds={keybinds}
							tooltipClassName={clsx('capitalize', toolTipClassName)}
							label={toolTipLabel}
						>
							{typeof icon === 'function' ? icon({ triggerOpen: () => {} }) : icon}
						</Tooltip>
					</TopBarButton>
				)}
			</>
			{index + 1 === group.length && groupIndex + 1 !== groupCount && (
				<div
					data-tauri-drag-region={os === 'macOS'}
					className={clsx(
						'mx-4 h-[15px] w-0 border-l',
						isDark ? 'border-zinc-600' : 'border-zinc-300'
					)}
				/>
			)}
		</div>
	);
}

export function WindowsControls({ windowSize }: { windowSize: number }) {
	const [maximized, setMaximized] = useState(false);
	const getWindowState = useCallback(async () => {
		const isMaximized = await getCurrent().isMaximized();
		setMaximized(isMaximized);
	}, []);

	useEffect(() => {
		getWindowState().catch(console.error);
	}, [getWindowState, windowSize]);
	return (
		<div className="mx-1 hidden items-center xl:flex">
			<TopBarButton
				className="mx-2"
				rounding="both"
				active={false}
				onClick={() => appWindow.minimize()}
			>
				<Minus weight="regular" className={clsx(TOP_BAR_ICON_CLASSLIST)} />
			</TopBarButton>
			<TopBarButton
				rounding="both"
				className="mx-2"
				active={false}
				onClick={() => {
					appWindow.toggleMaximize();
				}}
			>
				{maximized ? (
					<Cards weight="regular" className={clsx(TOP_BAR_ICON_CLASSLIST)} />
				) : (
					<Square weight="regular" className={clsx(TOP_BAR_ICON_CLASSLIST)} />
				)}
			</TopBarButton>
			<TopBarButton
				rounding="both"
				className="mx-2 hover:bg-red-500 *:hover:text-white"
				active={false}
				onClick={() => appWindow.close()}
			>
				<X weight="regular" className={clsx(TOP_BAR_ICON_CLASSLIST, 'hover:text-white')} />
			</TopBarButton>
		</div>
	);
}
