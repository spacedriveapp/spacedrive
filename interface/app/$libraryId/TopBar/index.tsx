import clsx from 'clsx';
import { CaretLeft, CaretRight } from 'phosphor-react';
import { useLayoutEffect, useRef } from 'react';
import { useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { Popover, Tooltip } from '@sd/ui';
import SearchBar from '../Explorer/SearchBar';
import TopBarButton from './TopBarButton';
import TopBarMobile from './TopBarMobile';
import { RoutePaths, ToolOption, useToolBarRouteOptions } from './useToolBarOptions';

export const TOP_BAR_HEIGHT = 46;

export default function TopBar() {
	const TOP_BAR_ICON_STYLE = 'm-0.5 w-5 h-5 text-ink-dull';
	const navigate = useNavigate();
	const { pathname } = useLocation();
	const getPageName = pathname.split('/')[2] as RoutePaths;
	const { toolBarRouteOptions } = useToolBarRouteOptions();
	const [windowSize, setWindowSize] = useState(0);
	const countToolOptions = toolBarRouteOptions[getPageName].options
		.map((group) => {
			if (Array.isArray(group)) {
				return group.length;
			}
			return 0;
		})
		.reduce((acc, curr) => acc + curr, 0);

	useLayoutEffect(() => {
		const handleResize = () => {
			setWindowSize(window.innerWidth);
		};
		window.addEventListener('resize', handleResize);
		handleResize();
		return () => window.removeEventListener('resize', handleResize);
	}, []);

	return (
		<div
			data-tauri-drag-region
			className="duration-250 top-bar-blur absolute top-0 z-50 grid h-[46px] w-full shrink-0 grid-cols-3 items-center justify-center overflow-hidden border-b border-sidebar-divider bg-app/90 px-5 transition-[background-color,border-color] ease-out"
		>
			<div data-tauri-drag-region className="flex ">
				<Tooltip label="Navigate back">
					<TopBarButton onClick={() => navigate(-1)}>
						<CaretLeft weight="bold" className={TOP_BAR_ICON_STYLE} />
					</TopBarButton>
				</Tooltip>
				<Tooltip label="Navigate forward">
					<TopBarButton onClick={() => navigate(1)}>
						<CaretRight weight="bold" className={TOP_BAR_ICON_STYLE} />
					</TopBarButton>
				</Tooltip>
			</div>

			<SearchBar formClassName="justify-center mr-12 lg:mr-0" />

			<div data-tauri-drag-region className="flex w-full flex-row justify-end">
				<div data-tauri-drag-region className={`flex gap-0`}>
					{toolBarRouteOptions[getPageName].options.map((group, groupIndex) => {
						return (group as ToolOption[]).map(
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
								const groupCount = toolBarRouteOptions[getPageName].options.length;
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
													<div className="block w-[250px] ">
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
					className={`${windowSize <= 1279 && countToolOptions > 4 ? 'flex' : 'hidden'}`}
				/>
			</div>
		</div>
	);
}
