import { CogIcon } from '@heroicons/react/24/outline';
import { Input } from '@sd/ui';
import clsx from 'clsx';
import { MagnifyingGlass } from 'phosphor-react';
import React from 'react';

import { DocCategory, DocsNavigation } from '../pages/docs/api';
import config from '../pages/docs/docs';

interface Props {
	navigation: DocsNavigation;
	activePath?: string;
}

export default function DocsSidebar(props: Props) {
	const activeSection = props.activePath?.split('/')[0] || props.navigation[0].slug;

	const activeSectionData = props.navigation.find((section) => section.slug === activeSection);

	return (
		<nav className="flex flex-col mr-8 w-52">
			<div className="relative">
				<MagnifyingGlass weight="bold" className="absolute top-3 left-3" />
				<Input className="mb-5 pl-9" placeholder="Search" />
				<span className="absolute font-bold text-gray-400 right-[22px] top-[7px]">⌘K</span>
			</div>
			<div className="flex flex-col mb-6">
				{props.navigation.map((section) => {
					const isActive = section.slug === activeSection;
					const Icon = config.sections.find((s) => s.slug === section.slug)?.icon;
					return (
						<a
							href={`/docs/${section.section[0].category[0].url}`}
							key={section.slug}
							className={clsx(
								`flex font-semibold text-[14px] items-center py-1.5 doc-sidebar-button`,
								section.slug,
								isActive && 'nav-active'
							)}
						>
							<div className={clsx(`p-1 mr-4 bg-gray-500 border-t rounded-lg border-gray-400/20`)}>
								<Icon className="w-4 h-4 text-white opacity-80" />
							</div>
							{section.title}
						</a>
					);
				})}
			</div>
			{activeSectionData?.section.map((category) => {
				return (
					<div className="mb-5" key={category.name}>
						<h2 className="font-semibold no-underline">{category.name}</h2>
						<ul className="mt-3">
							{category.category.map((page) => {
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
											href={`/docs/${page.url}`}
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
