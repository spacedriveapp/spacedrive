import { Input } from '@sd/ui';
import clsx from 'clsx';
import { MagnifyingGlass } from 'phosphor-react';
import React from 'react';

import { DocsList, SidebarCategory } from '../pages/docs/api';

interface Props {
	data: DocsList;
	activePath?: string;
}

export default function DocsSidebar(props: Props) {
	return (
		<nav className="flex flex-col mr-8 w-52">
			<div className="relative">
				<MagnifyingGlass weight="bold" className="absolute top-3 left-3" />
				<Input className="mb-5 pl-9" placeholder="Search" />
				<span className="absolute font-bold text-gray-400 right-[22px] top-[7px]">⌘K</span>
			</div>
			{props.data.map((item) => {
				return (
					<div className="mb-5" key={item.name}>
						<h2 className="font-semibold no-underline">{item.name}</h2>
						<ul className="mt-3">
							{item.items.map((page) => {
								const docURL = `/docs/${page.url}`;

								const active = props.activePath === page.url;
								return (
									<li
										className={clsx(
											'flex border-l border-gray-600',
											active && 'border-l-2 border-primary'
										)}
										key={page.title}
									>
										<a
											href={docURL}
											className={clsx(
												'font-normal w-full rounded px-3 py-1 hover:text-gray-50 no-underline text-[14px] text-gray-350',
												active && '!text-white !font-medium '
											)}
										>
											{page.title}
										</a>
										{/* this fixes the links no joke */}
										{active && <div />}
									</li>
								);
							})}
						</ul>
					</div>
				);
			})}
		</nav>
	);
}
