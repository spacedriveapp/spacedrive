'use client';

import React from 'react';
import After from '~/assets/after.jpg';
import Before from '~/assets/before.jpg';
import { Slider } from '~/components/before-after';

interface Props {
	// props
}

const Page: React.FC<Props> = () => {
	return (
		<div className="flex w-full flex-col items-center px-4">
			<div className="mt-22 lg:mt-28" id="content" aria-hidden="true" />
			<div className="mt-24 lg:mt-8" aria-hidden="true" /> <h1>Teams</h1>
			<Slider beforeImage={Before} afterImage={After} />
		</div>
	);
};

export default Page;
