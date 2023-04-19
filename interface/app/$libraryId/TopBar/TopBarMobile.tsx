import { DotsThreeCircle } from 'phosphor-react';
import { HTMLAttributes, useContext } from 'react';
import { Popover } from '@sd/ui';
import { TOP_BAR_ICON_STYLE } from './ToolBarProvider';
import { ToolBarContext } from './ToolBarProvider';
import TopBarButton from './TopBarButton';

interface Props extends HTMLAttributes<HTMLDivElement> {}

export default ({ className = '' }: Props) => {
	const { toolBar } = useContext(ToolBarContext);
	const toolsNotSmFlex = toolBar.options.map((group) =>
		group.filter((tool) => tool.showAtResolution !== 'sm:flex')
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
				<div className="flex flex-col overflow-hidden p-2">
					{toolsNotSmFlex.map((group, groupIndex) => {
						return group.map(
							(
								{ icon, onClick, popOverComponent, toolTipLabel, topBarActive },
								index
							) => {
								const groupCount = toolBar.options.length;
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
														<div className="flex w-full items-center justify-between">
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
