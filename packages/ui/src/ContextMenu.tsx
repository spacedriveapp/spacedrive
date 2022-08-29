import * as ContextMenuPrimitive from '@radix-ui/react-context-menu';
import { Root, Trigger } from '@radix-ui/react-context-menu';
import clsx from 'clsx';
import { CaretRight, Icon } from 'phosphor-react';
import { Question } from 'phosphor-react';
import React from 'react';

export interface ContextMenuItem {
	label: string;
	icon?: Icon;
	danger?: boolean;
	onClick?: () => void;

	children?: ContextMenuSection[];
}

export type ContextMenuSection = (ContextMenuItem | string)[];

export interface ContextMenuProps {
	items?: ContextMenuSection[];
	className?: string;
	isChild?: boolean;
}

export const ContextMenu: React.FC<ContextMenuProps> = (props) => {
	const { items: sections = [], className, isChild, ...rest } = props;

	const ContentPrimitive = isChild ? ContextMenuPrimitive.SubContent : ContextMenuPrimitive.Content;

	return (
		<ContentPrimitive
			sideOffset={7}
			className={clsx(
				'shadow-xl min-w-[12rem] shadow-gray-300 dark:shadow-gray-950 flex flex-col select-none cursor-default bg-gray-50 text-gray-800 border-gray-200 dark:bg-gray-950 dark:text-gray-100 dark:border-gray-550 text-left text-sm rounded-lg gap-1.5 py-1.5',
				className
			)}
			{...rest}
		>
			{sections.map((sec, i) => (
				<React.Fragment key={i}>
					{i !== 0 && (
						<ContextMenuPrimitive.Separator className="mx-2 border-0 border-b border-b-gray-300 dark:border-b-gray-550" />
					)}

					<ContextMenuPrimitive.Group className="flex items-stretch flex-col gap-0.5">
						{sec.map((item) => {
							if (typeof item === 'string')
								return (
									<ContextMenuPrimitive.Label
										key={item}
										className="mt-1 ml-2 text-xs text-gray-400 uppercase"
									>
										{item}
									</ContextMenuPrimitive.Label>
								);

							const { icon: ItemIcon } = item;

							let ItemComponent:
								| typeof ContextMenuPrimitive.Item
								| typeof ContextMenuPrimitive.Trigger = ContextMenuPrimitive.Item;

							if ((item.children?.length ?? 0) > 0)
								ItemComponent = ((props) => (
									<ContextMenuPrimitive.ContextMenuSub>
										<ContextMenuPrimitive.SubTrigger className="ml-1.5 rounded outline-none leading-snug flex-grow text-[14px] font-normal">
											{props.children}
										</ContextMenuPrimitive.SubTrigger>

										<ContextMenu
											isChild
											items={item.children}
											className="relative -left-1 -top-2"
										/>
									</ContextMenuPrimitive.ContextMenuSub>
								)) as typeof ContextMenuPrimitive.Trigger;

							return (
								<ItemComponent
									style={{
										font: 'inherit',
										textAlign: 'inherit'
									}}
									className={clsx(
										'focus:outline-none group cursor-default flex-1 px-1.5 py-0 group-first:pt-1.5',
										{
											'text-red-600 dark:text-red-400': item.danger
										}
									)}
									onClick={item.onClick}
									key={item.label}
								>
									<div className="px-1 py-[0.3em]  group-focus:bg-primary group-hover:bg-primary flex flex-row  items-center rounded">
										{ItemIcon && <ItemIcon size={18} />}

										<ContextMenuPrimitive.Label className="ml-1.5 leading-snug flex-grow text-[14px] font-normal">
											{item.label}
										</ContextMenuPrimitive.Label>

										{(item.children?.length ?? 0) > 0 && (
											<CaretRight weight="fill" size={12} alt="" />
										)}
									</div>
								</ItemComponent>
							);
						})}
					</ContextMenuPrimitive.Group>
				</React.Fragment>
			))}
		</ContentPrimitive>
	);
};

export { Trigger, Root };
