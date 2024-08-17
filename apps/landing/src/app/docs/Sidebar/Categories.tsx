'use client';

import clsx from 'clsx';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { ReactNode } from 'react';

export function Categories(props: { sections: { slug: string; categories: ReactNode }[] }) {
	const path = usePathname();
	const sectionSlug = path.split('/')[2]!;

	const section = props.sections.find((s) => s.slug === sectionSlug) ?? props.sections[0];

	return section.categories;
}

export function Doc(props: { slug: string; title?: string; url: string }) {
	const path = usePathname();

	const active = path === props.url;

	return (
		<li
			className={clsx('flex border-l border-gray-600', active && 'border-l-2 border-primary')}
			key={props.title}
		>
			<Link
				href={props.url}
				className={clsx(
					'w-full rounded px-3 py-1 text-[14px] font-normal text-gray-350 no-underline hover:text-gray-50',
					active && '!font-medium !text-white'
				)}
			>
				{props.title}
			</Link>
		</li>
	);
}
