import { DotsThreeCircle } from '@phosphor-icons/react';
import React, { forwardRef, HTMLAttributes } from 'react';
import { Popover, usePopover } from '@sd/ui';

import TopBarButton, { TopBarButtonProps } from './TopBarButton';
import { ToolOption, TOP_BAR_ICON_CLASSLIST } from './TopBarOptions';

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
			{typeof tool.icon === 'function' ? tool.icon({ triggerOpen: () => {} }) : tool.icon}
			{tool.toolTipLabel}
		</TopBarButton>
	);
});

interface Props extends HTMLAttributes<HTMLDivElement> {
	toolOptions?: ToolOption[][];
}

export default ({ toolOptions, className }: Props) => {
	const popover = usePopover();
	const toolsNotSmFlex = toolOptions?.map((group) =>
		group.filter((tool) => tool.showAtResolution !== 'sm:flex')
	);

	return (
		<div className={className}>
			<Popover
				popover={popover}
				trigger={
					<TopBarButton>
						<DotsThreeCircle className={TOP_BAR_ICON_CLASSLIST} />
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
											<ToolPopover
												tool={tool}
												triggerClose={() => popover.setOpen(false)}
											/>
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

function ToolPopover({ tool, triggerClose }: { tool: ToolOption; triggerClose: () => void }) {
	return (
		<Popover popover={usePopover()} trigger={<GroupTool tool={tool} />}>
			<div className="min-w-[250px]">
				{typeof tool.popOverComponent === 'function'
					? tool.popOverComponent({ triggerClose })
					: tool.popOverComponent}
			</div>
		</Popover>
	);
}
