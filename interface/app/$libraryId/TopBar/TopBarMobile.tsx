import { DotsThreeCircle } from 'phosphor-react';
import React, { HTMLAttributes, forwardRef } from 'react';
import { Popover } from '@sd/ui';
import { TOP_BAR_ICON_STYLE, ToolOption } from '.';
import TopBarButton, { TopBarButtonProps } from './TopBarButton';

const GroupTool = forwardRef<
	HTMLButtonElement,
	Omit<TopBarButtonProps, 'children'> & { tool: ToolOption }
>(({ tool, ...props }, ref) => {
	return (
		<TopBarButton
			ref={ref}
			className="!mr-0 w-full gap-1"
			active={tool.topBarActive}
			onClick={tool.onClick}
			checkIcon
			{...props}
		>
			{tool.icon}
			{tool.toolTipLabel}
		</TopBarButton>
	);
});

interface Props extends HTMLAttributes<HTMLDivElement> {
	toolOptions?: ToolOption[][];
}

export default ({ toolOptions, className }: Props) => {
	const toolsNotSmFlex = toolOptions?.map((group) =>
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
				<div className="flex flex-col p-2">
					{toolsNotSmFlex?.map((group, i) => (
						<React.Fragment key={i}>
							<div className="flex flex-col gap-1">
								{group.map((tool) => (
									<React.Fragment key={tool.toolTipLabel}>
										{tool.popOverComponent ? (
											<Popover trigger={<GroupTool tool={tool} />}>
												<div className="min-w-[250px]">
													{tool.popOverComponent}
												</div>
											</Popover>
										) : (
											<GroupTool tool={tool} />
										)}
									</React.Fragment>
								))}
							</div>

							{i !== 0 && i !== toolsNotSmFlex.length - 1 && (
								<div className="my-2 border-b border-app-line" />
							)}
						</React.Fragment>
					))}
				</div>
			</Popover>
		</div>
	);
};
