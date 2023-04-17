import { DotsThreeCircle } from 'phosphor-react';
import { HTMLAttributes } from 'react';
import { useLocation } from 'react-router-dom';
import { Popover } from '@sd/ui';
import TopBarButton from './TopBarButton';
import {
	RoutePaths,
	TOP_BAR_ICON_STYLE,
	ToolOption,
	useToolBarRouteOptions
} from './useToolBarOptions';

interface Props extends HTMLAttributes<HTMLDivElement> {}

export default ({ className = '' }: Props) => {
	const { pathname } = useLocation();
	const getPageName = pathname.split('/')[2] as RoutePaths;
	const { toolBarRouteOptions } = useToolBarRouteOptions();
	const toolsNotSmFlex = toolBarRouteOptions[getPageName].options.map((group) =>
		(group as ToolOption[]).filter((tool) => tool.showAtResolution !== 'sm:flex')
	);

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
					{toolsNotSmFlex.map((group, groupIndex) => {
						return (group as ToolOption[]).map(
							(
								{ icon, onClick, popOverComponent, toolTipLabel, topBarActive },
								index
							) => {
								const groupCount = toolBarRouteOptions[getPageName].options.length;
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
												<div className="block w-[250px] ">
													{popOverComponent}
												</div>
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
										{index === group.length - 1 &&
											groupIndex + 1 !== groupCount && (
												<div className="my-2 w-[100%] border-b border-app-line" />
											)}
									</div>
								);
							}
						);
					})}
				</div>
			</Popover>
		</div>
	);
};
