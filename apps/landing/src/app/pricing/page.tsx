import Image from 'next/image';
import React from 'react';

// import { Background } from './Background';
import { Cards } from './Cards';

export const metadata = {
	title: 'Pricing - Spacedrive',
	description: 'Spacedrive pricing and packages'
};

export default function PricingPage() {
	return (
		<>
			{/* <Background /> */}
			<Image
				loading="eager"
				className="absolute-horizontal-center top-0 fade-in"
				width={1278}
				height={626}
				alt="l"
				src="/images/misc/header-gradient.webp"
			/>
			<div className="z-5 relative mt-48">
				<h1 className="fade-in-heading mb-3 bg-gradient-to-r from-white from-40% to-indigo-400 to-60% bg-clip-text px-2 text-center text-4xl font-bold leading-tight text-transparent">
					Pricing
				</h1>
				<p className="animation-delay-1 fade-in-heading text-md leading-2 z-30 mx-auto mb-8 mt-1 max-w-2xl px-2 text-center text-gray-450 lg:text-lg lg:leading-8">
					Spacedrive can be used for free as you like. Upgrading gives you access to early
					features, and this is placeholder text
				</p>
				<Cards />
			</div>
		</>
	);
}
