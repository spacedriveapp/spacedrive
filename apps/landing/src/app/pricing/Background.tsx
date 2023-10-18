'use client';

import dynamic from 'next/dynamic';

const Space = dynamic(() => import('~/components/Space'), { ssr: false });

export function Background() {
	return (
		<div className="opacity-60">
			<Space />
		</div>
	);
}
