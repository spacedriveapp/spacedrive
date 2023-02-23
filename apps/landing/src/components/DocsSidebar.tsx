import clsx from 'clsx';
import { MagnifyingGlass } from 'phosphor-react';
import { Input } from '@sd/ui';
import { DocsNavigation } from '../pages/docs/api';
import config from '../pages/docs/docs';

interface Props {
	navigation: DocsNavigation;
	activePath?: string;
}

export default function DocsSidebar(props: Props) {
	const activeSection = props.activePath?.split('/')[0] || props.navigation[0].slug;

	const activeSectionData = props.navigation.find((section) => section.slug === activeSection);

	return (
		<nav className="mr-8 flex w-full flex-col sm:w-52">
			<div onClick={() => alert('Search coming soon...')} className="relative w-full">
				<MagnifyingGlass weight="bold" className="absolute top-3 left-3" />
				<Input className="pointer-events-none mb-5 w-full pl-9" placeholder="Search" />
				<span className="absolute  right-3 top-[9px] text-sm font-semibold text-gray-400">⌘K</span>
			</div>
			<div className="mb-6 flex flex-col">
				{props.navigation.map((section) => {
					const isActive = section.slug === activeSection;
					const Icon = config.sections.find((s) => s.slug === section.slug)?.icon;
					return (
						<a
							href={`/docs/${section.section[0].category[0].url}`}
							key={section.slug}
							className={clsx(
								`doc-sidebar-button flex items-center py-1.5 text-[14px] font-semibold`,
								section.slug,
								isActive && 'nav-active'
							)}
						>
							<div className={clsx(`mr-4 rounded-lg border-t border-gray-400/20 bg-gray-500 p-1`)}>
								<Icon weight="bold" className="h-4 w-4 text-white opacity-80" />
							</div>
							{section.title}
						</a>
					);
				})}
			</div>
			{activeSectionData?.section.map((category) => {
				return (
					<div className="mb-5" key={category.title}>
						<h2 className="font-semibold no-underline">{category.title}</h2>
						<ul className="mt-3">
							{category.category.map((page) => {
								const active = props.activePath === page.url;
								return (
									<li
										className={clsx(
											'flex border-l border-gray-600',
											active && 'border-primary border-l-2'
										)}
										key={page.title}
									>
										<a
											href={`/docs/${page.url}`}
											className={clsx(
												'text-gray-350 w-full rounded px-3 py-1 text-[14px] font-normal no-underline hover:text-gray-50',
												active && '!font-medium !text-white '
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
