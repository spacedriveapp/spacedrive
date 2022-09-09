import { Input } from '@sd/ui';
import clsx from 'clsx';

import { SidebarCategory } from '../pages/docs/api';

interface Props {
	data: SidebarCategory[];
	activePath: string;
}

export default function DocsSidebar(props: Props) {
	return (
		<nav className="flex flex-col mr-8 w-52">
			<Input className="mb-5" placeholder="Search" />
			{props.data.map((item) => {
				const categoryURL = `/docs/${item.name}`;
				return (
					<div className="mb-5" key={item.name}>
						<a href={categoryURL} className="font-semibold no-underline">
							{item.name}
						</a>
						<ul className="mt-3">
							{item.items.map((page) => {
								const docURL = `/docs/${page.path}`;

								const active = props.activePath === page.path;
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
