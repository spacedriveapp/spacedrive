'use client';

import clsx from 'clsx';
import Link from 'next/link';
import { ComponentProps } from 'react';

import { useDocsParams } from '../utils';

export function SectionLink({
	className,
	slug,
	...props
}: ComponentProps<typeof Link> & { slug: string }) {
	const params = useDocsParams();
	const isActive = slug === params.slug?.[0];

	return (
		<Link
			{...props}
			className={clsx(
				'doc-sidebar-button flex items-center py-1.5 text-[14px] font-semibold',
				isActive && 'nav-active',
				params.slug === undefined && 'first:nav-active',
				slug
			)}
		/>
	);
}
