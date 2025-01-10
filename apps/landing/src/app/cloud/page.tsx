'use client';

import { ArrowRight } from '@phosphor-icons/react';
import Image from 'next/image';
import React from 'react';
import { CtaPrimaryButton } from '~/components/cta-primary-button';
import { CtaSecondaryButton } from '~/components/cta-secondary-button';

const BENTO_BASE_CLASS = `bento-border-left relative flex flex-col rounded-[10px] bg-[radial-gradient(66.79%_83.82%_at_0%_3.69%,#1B1D25_0%,#15161C_100%)] overflow-hidden group`;

interface PricingTier {
	name: string;
	price: string;
	description: string;
	features: string[];
	highlighted?: boolean;
	href: string;
}

const pricingTiers: PricingTier[] = [
	{
		name: 'Free',
		price: '$0',
		description: 'Perfect for personal use',
		features: [
			'All client apps',
			'All core features',
			'Self-hosting cloud services',
			'Local P2P sync & transfer',
			'Personal use'
		],
		href: '/download'
	},
	{
		name: 'Sync',
		price: '$2.99',
		description: 'A seamless Spacedrive experience',
		features: [
			'Everything in Free',
			'Always-on cloud sync',
			'Library and preview media backup up to 10GB',
			'Relay service for P2P internet file transfer',
			'Basic support'
		],
		highlighted: true,
		href: '/cloud/subscribe/sync'
	},
	{
		name: 'Pro',
		price: '$9.99',
		description: 'Advanced Spacedrive experience',
		features: [
			'Everything in Sync',
			'Always-on cloud sync',
			'Custom sd.app share links',
			'1TB cloud storage',
			'100GB cloud transfer',
			'Premium support'
		],
		href: '/cloud/subscribe/pro'
	},
	{
		name: 'Enterprise',
		price: 'Contact us',
		description: 'For teams and organizations',
		features: [
			'Per-seat licensing',
			'Team management',
			'Inter-cloud transfer',
			'Self-hosted option',
			'Custom integrations',
			'Dedicated support'
		],
		href: '/enterprise'
	}
];

const Page: React.FC = () => {
	return (
		<div className="container mx-auto mt-[50px] px-4 pt-24">
			<div className="flex flex-col items-center">
				<Image
					quality={100}
					src="/images/cloud.png"
					width={380}
					height={380}
					alt="Spacedrive vault"
				/>
				<h1 className="z-30 mb-3 text-center text-2xl font-bold leading-[1.3] tracking-tight md:text-5xl lg:text-6xl">
					<span className="inline bg-gradient-to-b from-[#EFF1FB_15%] to-[#B8CEE0_85%] bg-clip-text text-transparent">
						Go the extra mile with Spacedrive Cloud
					</span>
				</h1>
				<p className="text-md z-30 mb-16 mt-1 max-w-5xl text-balance text-center text-gray-450 lg:text-lg lg:leading-8">
					Choose the perfect plan for your needs, from personal projects to enterprise
					solutions. All plans include client-side encryption, and come with free
					self-hosting.
				</p>

				<div className="grid w-full max-w-7xl gap-6 px-4 md:grid-cols-2 lg:grid-cols-4">
					{pricingTiers.map((tier) => (
						<div
							key={tier.name}
							className={`${BENTO_BASE_CLASS} ${
								tier.highlighted ? 'border-2 border-primary' : ''
							}`}
						>
							<div className="flex h-full flex-col p-6">
								<div className="mb-4">
									<h3 className="text-xl font-semibold text-white">
										{tier.name}
									</h3>
									<div className="mt-2">
										<span className="text-2xl font-bold text-white">
											{tier.price}
										</span>
										{tier.price !== 'Contact us' && (
											<span className="text-gray-400">/month</span>
										)}
									</div>
									<p className="mt-2 text-sm text-gray-400">{tier.description}</p>
								</div>
								<ul className="mb-6 flex-grow space-y-3">
									{tier.features.map((feature) => (
										<li key={feature} className="flex items-start">
											<svg
												className="mr-2 h-5 w-5 flex-shrink-0 text-primary"
												fill="none"
												viewBox="0 0 24 24"
												stroke="currentColor"
											>
												<path
													strokeLinecap="round"
													strokeLinejoin="round"
													strokeWidth={2}
													d="M5 13l4 4L19 7"
												/>
											</svg>
											<span className="text-sm text-gray-300">{feature}</span>
										</li>
									))}
								</ul>
								{tier.highlighted ? (
									<CtaPrimaryButton
										href={tier.href}
										icon={<ArrowRight weight="bold" />}
									>
										Get Started
									</CtaPrimaryButton>
								) : (
									<CtaSecondaryButton
										href={tier.href}
										icon={<ArrowRight className="size-5 opacity-60" />}
									>
										{tier.name === 'Enterprise'
											? 'Contact Sales'
											: 'Get Started'}
									</CtaSecondaryButton>
								)}
							</div>
						</div>
					))}
				</div>
			</div>
		</div>
	);
};

export default Page;
