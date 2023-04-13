import { Check, DotsThreeCircle } from 'phosphor-react';
import { HTMLAttributes } from 'react';
import { useState } from 'react';
import { useLocation } from 'react-router-dom';
import { Popover } from '@sd/ui';
import { TOP_BAR_ICON_STYLE } from '~/hooks/useToolBarOptions';
import { RoutePaths, groupKeys, useToolBarRouteOptions } from '~/hooks/useToolBarOptions';
import TopBarButton from './TopBarButton';

interface Props extends HTMLAttributes<HTMLDivElement> {}

export default ({ className = '' }: Props) => {
	const { pathname } = useLocation();
	const getPageName = pathname.split('/')[2] as RoutePaths;
	const { toolBarRouteOptions } = useToolBarRouteOptions();

	return (
		<div className={className}>
			<Popover
				trigger={
					<TopBarButton>
						<DotsThreeCircle className={TOP_BAR_ICON_STYLE} />
					</TopBarButton>
				}
			>
				<div className="flex flex-col p-2 overflow-hidden">
					{toolBarRouteOptions[getPageName].options.map((group) => {
						return (Object.keys(group) as groupKeys[]).map((groupKey) => {
							return group[groupKey]?.map(
								({ icon, onClick, popOverComponent, toolTipLabel, topBarActive }, index) => {
									const groupCount = Object.keys(group).length;
									const groupIndex = Object.keys(group).indexOf(groupKey);
									return (
										<div key={toolTipLabel}>
											{popOverComponent ? (
												<Popover
													className="focus:outline-none"
													trigger={
														<TopBarButton
															className="mb-1 flex !w-full gap-1"
															active={topBarActive}
															onClick={onClick}
															checkIcon={true}
														>
															<div className="flex items-center justify-between w-full">
																<div className="flex items-center gap-1">
																	{icon}
																	{toolTipLabel}
																</div>
															</div>
														</TopBarButton>
													}
												>
													<div className="block w-[250px] ">{popOverComponent}</div>
												</Popover>
											) : (
												<TopBarButton
													className="mb-1 flex !w-full gap-1"
													active={topBarActive}
													onClick={onClick ?? undefined}
													checkIcon={true}
												>
													{icon}
													{toolTipLabel}
												</TopBarButton>
											)}
											{index === (group[groupKey]?.length as number) - 1 &&
												groupCount !== groupIndex + 1 && (
													<div className="my-2 w-[100%] border-b border-app-line" />
												)}
										</div>
									);
								}
							);
						});
					})}
				</div>
			</Popover>
		</div>
	);
};
