'use client';

import clsx from 'clsx';
import Link from 'next/link';
import { iconConfig } from '~/utils/contentlayer';
import { toTitleCase } from '~/utils/util';

import { navigationMeta } from './data';
import { SearchBar } from './Search';

interface Props {
	slug?: string[];
}

export function Sidebar({ slug }: Props) {
	const slugString = slug?.join('/') ?? '/';
	const currentSection =
		navigationMeta.find((section) => section.slug === slug?.[0]) ?? navigationMeta[0];

	return (
		<nav className="mr-8 flex w-full flex-col sm:w-52">
			<SearchBar />
			<div className="mb-6 flex flex-col">
				{navigationMeta.map((section) => {
					const isActive = section.slug === currentSection.slug;
					const Icon = iconConfig[section.slug];
					return (
						<Link
							// Use the first page in the section as the link
							href={section.categories[0]?.docs[0]?.url}
							key={section.slug}
							className={clsx(
								`doc-sidebar-button flex items-center py-1.5 text-[14px] font-semibold`,
								section.slug,
								isActive && 'nav-active'
							)}
						>
							<div
								className={clsx(
									`mr-4 rounded-lg border-t border-gray-400/20 bg-gray-500 p-1`
								)}
							>
								<Icon weight="bold" className="h-4 w-4 text-white opacity-80" />
							</div>
							{toTitleCase(section.slug)}
						</Link>
					);
				})}
			</div>
			{currentSection?.categories.map((category) => (
				<div className="mb-5" key={category.title}>
					<h2 className="font-semibold no-underline">{category.title}</h2>
					<ul className="mt-3">
						{category.docs.map((doc) => {
							const active = slugString === doc.slug;
							return (
								<li
									className={clsx(
										'flex border-l border-gray-600',
										active && 'border-l-2 border-primary'
									)}
									key={doc.title}
								>
									<Link
										href={doc.url}
										className={clsx(
											'w-full rounded px-3 py-1 text-[14px] font-normal text-gray-350 no-underline hover:text-gray-50',
											active && '!font-medium !text-white '
										)}
									>
										{doc.title}
									</Link>
									{/* this fixes the links no joke */}
									{active && <div />}
								</li>
							);
						})}
					</ul>
				</div>
			))}
		</nav>
	);
}
