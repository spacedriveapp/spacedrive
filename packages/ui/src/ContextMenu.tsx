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
	sections?: {
		heading?: string;
		items: ContextMenuItem[];
	}[];
	className?: string;
}

export const ContextMenu: React.FC<ContextMenuProps> = (props) => {
	const { sections = [], className, ...rest } = props;

	return (
		<div
			role="menu"
			className={clsx(
				'shadow-box shadow-gray-300 dark:shadow-gray-700 flex flex-col select-none cursor-default bg-gray-50 text-gray-800 border-gray-200 dark:bg-gray-600 dark:text-gray-100 dark:border-gray-500 text-left text-sm font-semibold rounded gap-1.5 border-2 py-1.5',
				className
			)}
			{...rest}
		>
			{sections.map((sec, i) => (
				<>
					{i !== 0 && (
						<hr className="border-0 border-b border-b-gray-300 dark:border-b-gray-500 mx-2" />
					)}

					<section key={i} className="flex items-stretch flex-col gap-0.5">
						{sec.heading && (
							<span className="text-xs ml-2 mt-1 uppercase text-gray-400">{sec.heading}</span>
						)}

						<ul>
							{sec.items.map(({ icon: ItemIcon = Question, ...item }) => (
								<li key={item.label} className="flex">
									<button
										style={{
											font: 'inherit',
											textAlign: 'inherit'
										}}
										className={clsx('group cursor-default flex-1 px-1.5 py-0 group-first:pt-1.5', {
											'text-red-600 dark:text-red-400': item.danger
										})}
										onClick={item.onClick}
									>
										<div className="px-1.5 py-[0.4em] group-focus-visible:bg-gray-150 group-hover:bg-gray-150 dark:group-focus-visible:bg-gray-500 dark:group-hover:bg-gray-500 flex flex-row gap-1.5 items-center rounded-sm">
											{<ItemIcon size={18} />}
											<span className="leading-snug">{item.label}</span>
										</div>
									</button>
								</li>
							))}
						</ul>
					</section>
				</>
			))}
		</div>
	);
};
