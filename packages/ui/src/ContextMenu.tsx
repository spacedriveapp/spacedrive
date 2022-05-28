import clsx from 'clsx';
import type { Icon } from 'phosphor-react';
import { Question } from 'phosphor-react';
import React from 'react';

export interface ContextMenuItem {
	label: string;
	icon?: Icon;
	danger?: boolean;
	onClick: () => void;
}

export interface ContextMenuProps {
	items?: (ContextMenuItem | string)[][];
	className?: string;
}

export const ContextMenu: React.FC<ContextMenuProps> = (props) => {
	const { items = [], className, ...rest } = props;

	return (
		<div
			role="menu"
			className={clsx(
				'shadow-2xl min-w-[15rem] shadow-gray-300 dark:shadow-gray-750 flex flex-col select-none cursor-default bg-gray-50 text-gray-800 border-gray-200 dark:bg-gray-650 dark:text-gray-100 dark:border-gray-550 text-left text-sm rounded gap-1.5 border py-1.5',
				className
			)}
			{...rest}
		>
			{items.map((sec, i) => (
				<>
					{i !== 0 && (
						<hr className="border-0 border-b border-b-gray-300 dark:border-b-gray-550 mx-2" />
					)}

					<section key={i} className="flex items-stretch flex-col gap-0.5">
						<ul>
							{sec.map((item) => {
								if (typeof item === 'string')
									return <span className="text-xs ml-2 mt-1 uppercase text-gray-400">{item}</span>;

								const { icon: ItemIcon = Question } = item;

								return (
									<li key={item.label} className="flex">
										<button
											style={{
												font: 'inherit',
												textAlign: 'inherit'
											}}
											className={clsx(
												'group cursor-default flex-1 px-1.5 py-0 group-first:pt-1.5',
												{
													'text-red-600 dark:text-red-400': item.danger
												}
											)}
											onClick={item.onClick}
										>
											<div className="px-1.5 py-[0.4em] group-focus-visible:bg-gray-150 group-hover:bg-gray-150 dark:group-focus-visible:bg-gray-550 dark:group-hover:bg-gray-550 flex flex-row gap-2.5 items-center rounded-sm">
												{<ItemIcon size={18} />}
												<span className="leading-snug text-[14px] font-normal">{item.label}</span>
											</div>
										</button>
									</li>
								);
							})}
						</ul>
					</section>
				</>
			))}
		</div>
	);
};
