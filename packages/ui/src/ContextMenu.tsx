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
				'flex flex-col select-none cursor-default bg-gray-600 text-gray-100 text-left text-sm font-semibold rounded p-1.5 gap-1.5 border-2 border-gray-500',
				className
			)}
			{...rest}
		>
			{sections.map((sec, i) => (
				<>
					{i !== 0 && <hr className="border-0 border-b border-b-gray-500 mx-2" />}

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
										className={clsx(
											'flex flex-row gap-1.5 items-center cursor-default rounded-sm flex-1 px-1.5 py-1 focus:bg-gray-500 hover:bg-gray-500',
											{
												'text-red-400': item.danger
											}
										)}
										onClick={item.onClick}
									>
										{<ItemIcon size={18} />}
										<span className="leading-snug">{item.label}</span>
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
