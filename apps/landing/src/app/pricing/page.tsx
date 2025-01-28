'use client';

import { ArrowCircleDown, ArrowRight } from '@phosphor-icons/react';
import clsx from 'clsx';
import { AnimatePresence, motion } from 'framer-motion';
import Image from 'next/image';
import React, { useState } from 'react';
import { Switch } from '@sd/ui';
import { CtaButton } from '~/components/cta-button';

const BENTO_BASE_CLASS = `bento-border-left relative flex flex-col rounded-[10px] bg-[radial-gradient(66.79%_83.82%_at_0%_3.69%,#1B1D25_0%,#15161C_100%)] overflow-hidden group`;

interface PricingTier {
	name: string;
	price: {
		monthly: string;
		yearly: string;
	};
	description: string;
	features: string[];
	highlighted?: boolean;
	href: string;
	cta: {
		text: string;
		icon: React.ReactNode;
	};
}

const pricingTiers: PricingTier[] = [
	{
		name: 'Free',
		price: {
			monthly: '$0',
			yearly: '$0'
		},
		description: 'Your personal data command center',
		features: [
			'All client apps',
			'Full core feature set',
			'Your devices are your cloud',
			'Connect custom S3 storage',
			'Local P2P sync & transfer',
			'Unlimited personal use'
		],
		href: '/download',
		cta: {
			text: 'Download',
			icon: <ArrowCircleDown weight="bold" size={22} />
		}
	},
	{
		name: 'Sync',
		price: {
			monthly: '$2.99',
			yearly: '$29.99'
		},
		description: 'Seamless multi-device experience',
		features: [
			'Everything in Free',
			'Always-on encrypted cloud sync',
			'10GB cloud backup for library & previews',
			'Global P2P relay network',
			'Email support'
		],
		highlighted: true,
		href: '/cloud/subscribe/sync',
		cta: {
			text: 'Get Started',
			icon: <ArrowRight weight="bold" size={22} />
		}
	},
	{
		name: 'Pro',
		price: {
			monthly: '$9.99',
			yearly: '$99.99'
		},
		description: 'Power user cloud features',
		features: [
			'Everything in Sync',
			'1TB managed cloud storage',
			'100GB monthly transfer quota',
			'Custom sd.app share URLs',
			'Unlimited sharing capabilities',
			'Priority support'
		],
		href: '/cloud/subscribe/pro',
		cta: {
			text: 'Get Started',
			icon: <ArrowRight className="size-5 opacity-60" />
		}
	},
	{
		name: 'Enterprise',
		price: {
			monthly: 'Contact us',
			yearly: 'Contact us'
		},
		description: 'Scale to meet your needs',
		features: [
			'Per-seat licensing model',
			'Team workspace & management',
			'Cross-cloud data transfer',
			'Self-hosted infrastructure',
			'Custom integration support',
			'Dedicated success manager'
		],
		href: '/enterprise',
		cta: {
			text: 'Contact Sales',
			icon: <ArrowRight className="size-5 opacity-60" />
		}
	}
];

export default function PricingPage() {
	const [yearlyBilling, setYearlyBilling] = useState(true);

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
					solutions. All plans include client-side encryption and optional self-hosting
					capabilities.
				</p>

				<div className="mb-8 flex items-center gap-3">
					<span className="text-sm font-medium text-gray-400">Monthly</span>
					<Switch
						checked={yearlyBilling}
						onCheckedChange={setYearlyBilling}
						className="data-[state=checked]:bg-accent"
					/>
					<span className="text-sm font-medium text-gray-400">
						Yearly <span className="text-accent">(Save 20%)</span>
					</span>
				</div>

				<div className="grid w-full max-w-7xl gap-6 px-4 md:grid-cols-2 lg:grid-cols-4">
					{pricingTiers.map((tier) => {
						const monthlyPrice =
							tier.name === 'Free'
								? '0'
								: yearlyBilling
									? (parseFloat(tier.price.yearly.replace('$', '')) / 12).toFixed(
											2
										)
									: tier.price.monthly.replace('$', '');

						return (
							<div
								key={tier.name}
								className={clsx(BENTO_BASE_CLASS, {
									'border-2 border-accent': tier.highlighted
								})}
							>
								<div className="flex h-full flex-col gap-6 p-6">
									<div>
										<h3 className="text-lg font-bold text-white">
											{tier.name}
										</h3>
										<div className="mt-2 flex flex-col">
											{tier.price.monthly !== 'Contact us' ? (
												<>
													<div className="flex items-end gap-1">
														<span className="text-2xl font-bold text-white">
															${monthlyPrice}
														</span>
														<span className="mb-0.5 text-sm text-gray-400">
															/mo
														</span>
													</div>
													{yearlyBilling && tier.name !== 'Free' && (
														<AnimatePresence>
															<motion.span
																initial={{ opacity: 0, y: -10 }}
																animate={{ opacity: 1, y: 0 }}
																exit={{ opacity: 0, y: -10 }}
																transition={{
																	duration: 0.2,
																	ease: 'easeOut'
																}}
																className="text-xs text-gray-400"
															>
																billed {tier.price.yearly} yearly
															</motion.span>
														</AnimatePresence>
													)}
												</>
											) : (
												<span className="text-2xl font-bold text-white">
													Contact us
												</span>
											)}
										</div>
										<p className="mt-2 text-sm text-gray-400">
											{tier.description}
										</p>
									</div>

									<ul className="flex flex-1 flex-col gap-3">
										{tier.features.map((feature) => (
											<li
												key={feature}
												className="flex items-center gap-2 text-sm text-white"
											>
												<div className="size-1 rounded-full bg-accent" />
												{feature}
											</li>
										))}
									</ul>

									<CtaButton
										href={tier.href}
										icon={tier.cta.icon}
										highlighted={tier.highlighted}
									>
										{tier.cta.text}
									</CtaButton>
								</div>
							</div>
						);
					})}
				</div>
			</div>
		</div>
	);
}
