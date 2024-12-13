'use client';

import { CaretRight } from '@phosphor-icons/react';
import { useParams } from 'next/navigation';
import { Fragment } from 'react';
import { toTitleCase } from '~/utils/misc';

export function Breadcrumbs() {
	const { slug } = useParams<{ slug?: string[] }>();
	if (!slug) return null;

	return (
		<div className="flex flex-row items-center gap-1">
			{slug.map((item, index) => (
				<Fragment key={index}>
					{index > 0 && <CaretRight className="size-4" />}
					<span className="px-1 text-sm">{toTitleCase(item)}</span>
				</Fragment>
			))}
		</div>
	);
}
