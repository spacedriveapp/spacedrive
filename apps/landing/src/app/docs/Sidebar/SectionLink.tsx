'use client';

import clsx from 'clsx';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { ComponentProps } from 'react';

export function SectionLink({
	className,
	slug,
	...props
}: ComponentProps<typeof Link> & { slug: string }) {
	const path = usePathname();
	const section = path.split('/')[2];
	const isActive = slug === section;

	return (
		<Link
			{...props}
			className={clsx(
				'doc-sidebar-button flex items-center py-1.5 text-[14px] font-semibold',
				isActive && 'nav-active',
				section === undefined && 'first:nav-active',
				slug
			)}
		/>
	);
}
