import clsx from 'clsx';
import { useLayoutEffect, useState } from 'react';
import { Popover, Tooltip } from '@sd/ui';
import TopBarButton from './TopBarButton';
import TopBarMobile from './TopBarMobile';

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
interface TopBarChildrenProps {
	options?: ToolOption[][];
}

export const TOP_BAR_ICON_STYLE = 'm-0.5 w-[18px] h-[18px] text-ink-dull';

export default ({ options }: TopBarChildrenProps) => {
	const [windowSize, setWindowSize] = useState(0);
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
		<div data-tauri-drag-region className="flex flex-1 justify-end">
			<div data-tauri-drag-region className={`flex gap-0`}>
				{options?.map((group, groupIndex) => {
					return group.map(
						(
							{
								icon,
								onClick,
								popOverComponent,
								toolTipLabel,
								topBarActive,
								individual,
								showAtResolution
							},
							index
						) => {
							const groupCount = options.length;
							const roundingCondition = individual
								? 'both'
								: index === 0
								? 'left'
								: index === group.length - 1
								? 'right'
								: 'none';
							return (
								<div
									data-tauri-drag-region
									key={toolTipLabel}
									className={clsx(
										[showAtResolution],
										[individual && 'mx-1'],
										`hidden items-center`
									)}
								>
									<>
										{popOverComponent ? (
											<Popover
												className="focus:outline-none"
												trigger={
													<TopBarButton
														rounding={roundingCondition}
														active={topBarActive}
														onClick={onClick}
													>
														<Tooltip label={toolTipLabel}>
															{icon}
														</Tooltip>
													</TopBarButton>
												}
											>
												<div className="block min-w-[250px] max-w-[500px]">
													{popOverComponent}
												</div>
											</Popover>
										) : (
											<TopBarButton
												rounding={roundingCondition}
												active={topBarActive}
												onClick={onClick ?? undefined}
											>
												<Tooltip label={toolTipLabel}>{icon}</Tooltip>
											</TopBarButton>
										)}
									</>
									{index + 1 === group.length &&
										groupIndex + 1 !== groupCount && (
											<div
												data-tauri-drag-region
												className="mx-4 h-[15px] w-0 border-l border-zinc-600"
											/>
										)}
								</div>
							);
						}
					);
				})}
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
