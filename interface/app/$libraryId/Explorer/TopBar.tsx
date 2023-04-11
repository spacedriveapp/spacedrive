import clsx from 'clsx';
import { CaretLeft, CaretRight } from 'phosphor-react';
import { useRef } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { Popover, Tooltip } from '@sd/ui';
import { RoutePaths, groupKeys, useToolBarRouteOptions } from '~/hooks/useToolBarOptions';
import SearchBar from './SearchBar';
import TopBarButton from './TopBarButton';

export default () => {
	const TOP_BAR_ICON_STYLE = 'm-0.5 w-5 h-5 text-ink-dull';
	const navigate = useNavigate();

	const searchRef = useRef<HTMLInputElement>(null);
	const { pathname } = useLocation();
	const getPageName = pathname.split('/')[2] as RoutePaths;
	const { toolBarRouteOptions } = useToolBarRouteOptions();

	return (
		<div
			data-tauri-drag-region
			className={clsx(
				'duration-250 absolute top-0 z-20 flex grid h-[46px] w-full shrink-0 grid-cols-3 items-center justify-center overflow-hidden border-b border-sidebar-divider bg-app px-5 transition-[background-color] transition-[border-color] ease-out'
			)}
		>
			<div className="flex ">
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

			<SearchBar formClassName="justify-center" className={'flex'} ref={searchRef} />

			<div data-tauri-drag-region className="flex w-full flex-row justify-end">
				<div className="flex gap-2">
					{toolBarRouteOptions[getPageName].options.map((group) => {
						return (Object.keys(group) as groupKeys[]).map((groupKey) => {
							return group[groupKey]?.map(
								({ icon, onClick, popOverComponent, toolTipLabel, topBarActive }, index) => {
									const groupCount = Object.keys(group).length;
									const groupIndex = Object.keys(group).indexOf(groupKey);
									return (
										<div key={toolTipLabel} className="flex items-center">
											<Tooltip label={toolTipLabel}>
												{popOverComponent ? (
													<Popover
														className="focus:outline-none"
														trigger={
															<TopBarButton active={topBarActive} onClick={onClick}>
																{icon}
															</TopBarButton>
														}
													>
														<div className="block w-[250px] ">{popOverComponent}</div>
													</Popover>
												) : (
													<TopBarButton active={topBarActive} onClick={onClick ?? undefined}>
														{icon}
													</TopBarButton>
												)}
											</Tooltip>
											{index === (group[groupKey]?.length as number) - 1 &&
												groupCount !== groupIndex + 1 && (
													<div className="ml-2 h-[15px] w-0 border-l border-zinc-600" />
												)}
										</div>
									);
								}
							);
						});
					})}
				</div>
			</div>
		</div>
	);
};
