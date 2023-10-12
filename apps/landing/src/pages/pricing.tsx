import { Check } from '@phosphor-icons/react';
import clsx from 'clsx';
import Head from 'next/head';
import Image from 'next/image';
import React, { useState } from 'react';
import { Button, Switch } from '@sd/ui';
import PageWrapper from '~/components/PageWrapper';
import { Space } from '~/components/Space';

export default function PricingPage() {
	const [toggle, setToggle] = useState<boolean>(false);
	return (
		<>
			<Head>
				<title>Pricing - Spacedrive</title>
				<meta name="description" content="Spacedrive pricing and packages" />
			</Head>
			<div className="opacity-60">
				<Space />
			</div>
			<PageWrapper>
				<Image
					loading="eager"
					className="absolute-horizontal-center top-0 fade-in"
					width={1278}
					height={626}
					alt="l"
					src="/images/headergradient.webp"
				/>
				<div className="z-5 relative mt-48">
					<h1
						className="fade-in-heading mb-3 bg-gradient-to-r from-white from-40% to-indigo-400 to-60% bg-clip-text px-2 text-center text-4xl
				font-bold leading-tight text-transparent"
					>
						Pricing
					</h1>
					<p className="animation-delay-1 fade-in-heading text-md leading-2 z-30 mx-auto mb-8 mt-1 max-w-2xl px-2 text-center text-gray-450 lg:text-lg lg:leading-8">
						Spacedrive can be used for free as you like. Upgrading gives you access to
						early features, and this is placeholder text
					</p>
					<div className="fade-in-heading animation-delay-2 mx-auto flex w-full items-center justify-center gap-3">
						<p className="text-sm font-medium text-white">Monthly</p>
						<Switch onCheckedChange={setToggle} checked={toggle} size="lg" />
						<p className="text-sm font-medium text-white">Yearly</p>
					</div>
					<div
						className="fade-in-heading animation-delay-2 mx-auto mb-[200px] mt-[75px] flex
							 w-full max-w-[1000px] flex-col items-center justify-center gap-10 px-2 md:flex-row"
					>
						<PackageCard
							features={[
								'lorem ipsum text',
								'lorem ipsum text',
								'lorem ipsum text',
								'lorem ipsum text',
								'lorem ipsum text',
								'lorem ipsum text'
							]}
							subTitle="Free for everyone"
							toggle={toggle}
							name="Free"
						/>
						<PackageCard
							features={[
								'lorem ipsum text',
								'lorem ipsum text',
								'lorem ipsum text',
								'lorem ipsum text',
								'lorem ipsum text',
								'lorem ipsum text'
							]}
							toggle={toggle}
							name="Pro"
							price={{
								monthly: '14.99',
								yearly: '99.99'
							}}
						/>
						<PackageCard
							features={[
								'lorem ipsum text',
								'lorem ipsum text',
								'lorem ipsum text',
								'lorem ipsum text',
								'lorem ipsum text',
								'lorem ipsum text'
							]}
							subTitle="Contact sales"
							toggle={toggle}
							name="Enterprise"
						/>
					</div>
				</div>
			</PageWrapper>
		</>
	);
}

interface Props {
	features: string[];
	name: 'Free' | 'Pro' | 'Enterprise';
	subTitle?: string;
	price?: {
		monthly: string;
		yearly: string;
	};
	toggle: boolean;
}

const PackageCard = ({ features, name, price, toggle, subTitle }: Props) => {
	const duration = toggle ? 'year' : 'month';
	return (
		<div
			className={clsx(
				'h-auto w-full max-w-[300px] bg-[#0E0D1B]',
				'relative rounded-md',
				name === 'Pro'
					? 'pro-card-border-gradient pro-card-shadow'
					: 'border border-[#222041]'
			)}
		>
			{name === 'Pro' && (
				<div
					className="pro-card-border-gradient popular-shadow absolute-horizontal-center top-[-12px]
				 rounded-[6px] bg-[#0E0D1B] px-5 py-1"
				>
					<p className="text-[10px] font-medium uppercase text-white">Popular</p>
				</div>
			)}
			<div className="z-2 relative h-fit">
				<div className="mx-auto h-[138px] w-[99.4%] border-b border-b-[#222041]">
					<div className="flex flex-col items-center justify-center py-6">
						<p className="text-md mb-4 uppercase text-[#A7ADD2]">{name}</p>
						{price && (
							<>
								<p className="text-2xl font-bold leading-[1] text-white">
									${toggle ? price.yearly : price.monthly}
								</p>
								<p className="text-md text-[#A7ADD2]">per {duration}</p>
							</>
						)}
						{subTitle && (
							<p className="text-2xl font-bold leading-[1] text-white">{subTitle}</p>
						)}
					</div>
				</div>
				<div className="h-full px-3 pb-8 pt-14 text-center">
					<div className="mx-auto mb-20 flex h-[200px] w-fit flex-col items-start justify-center gap-3">
						{name === 'Pro' && (
							<p className="text-sm text-white">
								Everything in <b>Free</b>, plus...
							</p>
						)}
						{name === 'Enterprise' && (
							<p className="text-sm text-white">
								Everything in <b>Pro</b>, plus...
							</p>
						)}
						{features.map((feature, index) => (
							<div key={index} className="flex items-center justify-center gap-2.5">
								<div className="flex h-5 w-5 items-center justify-center rounded-full border border-[#353252] bg-[#2A2741]">
									<Check weight="bold" size={12} color="white" />
								</div>
								<p className="text-sm text-white">{feature}</p>
							</div>
						))}
					</div>
					<Button
						className={clsx(
							'h-[35px] px-3',
							name === 'Pro' &&
								'to-blur-500 border-0 bg-gradient-to-r from-violet-500'
						)}
						variant="accent"
					>
						{(name === 'Free' && 'Try for free') ||
							(name === 'Pro' && 'Subscribe') ||
							(name === 'Enterprise' && 'Contact us')}
					</Button>
				</div>
			</div>
		</div>
	);
};
