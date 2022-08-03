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
	onClick: () => void;

	children?: ContextMenuSection[];
}

export type ContextMenuSection = (ContextMenuItem | string)[];

export interface ContextMenuProps {
	items?: ContextMenuSection[];
	className?: string;
}

export const ContextMenu: React.FC<ContextMenuProps> = (props) => {
	const { items: sections = [], className, ...rest } = props;

	return (
		<ContextMenuPrimitive.Content
			className={clsx(
				'shadow-2xl min-w-[15rem] shadow-gray-300 dark:shadow-gray-750 flex flex-col select-none cursor-default bg-gray-50 text-gray-800 border-gray-200 dark:bg-gray-650 dark:text-gray-100 dark:border-gray-550 text-left text-sm rounded gap-1.5 border py-1.5',
				className
			)}
			{...rest}
		>
			{sections.map((sec, i) => (
				<React.Fragment key={i}>
					{i !== 0 && (
						<ContextMenuPrimitive.Separator className="border-0 border-b border-b-gray-300 dark:border-b-gray-550 mx-2" />
					)}

					<ContextMenuPrimitive.Group className="flex items-stretch flex-col gap-0.5">
						{sec.map((item) => {
							if (typeof item === 'string')
								return (
									<ContextMenuPrimitive.Label
										key={item}
										className="text-xs ml-2 mt-1 uppercase text-gray-400"
									>
										{item}
									</ContextMenuPrimitive.Label>
								);

							const { icon: ItemIcon = Question } = item;

							let ItemComponent:
								| typeof ContextMenuPrimitive.Item
								| typeof ContextMenuPrimitive.Trigger = ContextMenuPrimitive.Item;

							if ((item.children?.length ?? 0) > 0)
								ItemComponent = ((props) => (
									<ContextMenuPrimitive.Root>
										<ContextMenuPrimitive.Trigger {...props}>
											{props.children}
										</ContextMenuPrimitive.Trigger>

										<ContextMenu items={item.children} className="relative -left-1 -top-2" />
									</ContextMenuPrimitive.Root>
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
									<div className="px-1.5 py-[0.4em] group-focus:bg-gray-150 group-hover:bg-gray-150 dark:group-focus:bg-gray-550 dark:group-hover:bg-gray-550 flex flex-row gap-2.5 items-center rounded-sm">
										{<ItemIcon size={18} />}

										<ContextMenuPrimitive.Label className="leading-snug flex-grow text-[14px] font-normal">
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
		</ContextMenuPrimitive.Content>
	);
};

export { Trigger, Root };
