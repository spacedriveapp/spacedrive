import { Popover, Tooltip } from '@sd/ui';
import clsx from 'clsx';
import { useContext, useLayoutEffect, useState } from 'react';
import { createPortal } from 'react-dom';
import { ToolOption } from '.';
import { TopBarContext } from './Layout';
import TopBarButton from './TopBarButton';
import TopBarMobile from './TopBarMobile';

interface TopBarChildrenProps {
	toolOptions?: ToolOption[][];
}

export default ({ toolOptions }: TopBarChildrenProps) => {
	const ctx = useContext(TopBarContext);
	const target = ctx.topBarChildrenRef?.current;
	const [windowSize, setWindowSize] = useState(0);
	const toolsNotSmFlex = toolOptions
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

	if (!target) {
		return null;
	}

	return createPortal(
		<div data-tauri-drag-region className="flex w-full flex-row justify-end">
			<div data-tauri-drag-region className={`flex gap-0`}>
				{toolOptions?.map((group, groupIndex) => {
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
							const groupCount = toolOptions.length;
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
									<Tooltip label={toolTipLabel}>
										{popOverComponent ? (
											<Popover
												className="focus:outline-none"
												trigger={
													<TopBarButton
														rounding={roundingCondition}
														active={topBarActive}
														onClick={onClick}
													>
														{icon}
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
												{icon}
											</TopBarButton>
										)}
									</Tooltip>
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
				toolOptions={toolOptions}
				className={`${windowSize <= 1279 && (toolsNotSmFlex?.length as number) > 0 ? 'flex' : 'hidden'
					}`}
			/>
		</div>,
		target
	);
};
