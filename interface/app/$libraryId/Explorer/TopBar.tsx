import clsx from 'clsx';
import { CaretLeft, CaretRight } from 'phosphor-react';
import { useRef } from 'react';
import { useLayoutEffect, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { Popover, Tooltip } from '@sd/ui';
import { RoutePaths, groupKeys, useToolBarRouteOptions } from '~/hooks/useToolBarOptions';
import SearchBar from './SearchBar';
import TopBarButton from './TopBarButton';
import TopBarMobile from './TopBarMobile';

export const TOP_BAR_HEIGHT = 46;

export default () => {
	const TOP_BAR_ICON_STYLE = 'm-0.5 w-5 h-5 text-ink-dull';
	const navigate = useNavigate();
	const topBarRef = useRef<HTMLDivElement>(null);
	const searchRef = useRef<HTMLInputElement>(null);
	const { pathname } = useLocation();
	const getPageName = pathname.split('/')[2] as RoutePaths;
	const { toolBarRouteOptions } = useToolBarRouteOptions();
	const [topBarWidth, setTopBarWidth] = useState(0);
	const countToolOptions = toolBarRouteOptions[getPageName].options.reduce(
		(totalOptionsCount, option) => {
			const allTools = ([].concat as any)(...Object.values(option));
			return totalOptionsCount + allTools.length;
		},
		0
	);

	useLayoutEffect(() => {
		const topBar = topBarRef.current;
		const searchBar = searchRef.current;
		const getTopBarWidth = () => {
			if (topBar && searchBar) {
				const { width } = topBar.getBoundingClientRect();
				setTopBarWidth(width);
			}
		};
		getTopBarWidth();
		window.addEventListener('resize', getTopBarWidth);
		return () => {
			window.removeEventListener('resize', getTopBarWidth);
		};
	}, [topBarRef]);

	const topBarCondition =
		(topBarWidth < 1000 && countToolOptions >= 8) || (topBarWidth < 600 && countToolOptions >= 6);

	return (
		<div
			data-tauri-drag-region
			ref={topBarRef}
			className={clsx(
				'duration-250 absolute top-0 z-20 flex grid h-[46px] w-full shrink-0 grid-cols-3 items-center justify-center overflow-hidden border-b border-sidebar-divider bg-app px-5 transition-[background-color] transition-[border-color] ease-out'
			)}
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

			<SearchBar formClassName="justify-center" ref={searchRef} />

			<div data-tauri-drag-region className="flex flex-row justify-end w-full">
				<div data-tauri-drag-region className={`gap-0 ${topBarCondition ? 'hidden' : 'flex'}`}>
					{toolBarRouteOptions[getPageName].options.map((group) => {
						return (Object.keys(group) as groupKeys[]).map((groupKey) => {
							return group[groupKey]?.map(
								(
									{ icon, onClick, popOverComponent, toolTipLabel, topBarActive, individual },
									index
								) => {
									const groupCount = Object.keys(group).length;
									const groupIndex = Object.keys(group).indexOf(groupKey);
									const roundingCondition = individual
										? 'both'
										: index === 0
										? 'left'
										: index === (group[groupKey]?.length as number) - 1
										? 'right'
										: 'none';
									return (
										<div
											data-tauri-drag-region
											key={toolTipLabel}
											className={`flex items-center ${individual && 'mx-1'}`}
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
														<div className="block w-[250px] ">{popOverComponent}</div>
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
											{index === (group[groupKey]?.length as number) - 1 &&
												groupCount !== groupIndex + 1 && (
													<div
														data-tauri-drag-region
														className="mx-4 h-[15px] w-0 border-l border-zinc-600"
													/>
												)}
										</div>
									);
								}
							);
						});
					})}
				</div>
				<TopBarMobile className={`${topBarCondition ? 'flex' : 'hidden'}`} />
			</div>
		</div>
	);
};
