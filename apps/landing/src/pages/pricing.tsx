import Head from 'next/head';
import React from 'react';
import PageWrapper from '~/components/PageWrapper';
import Space from '~/components/Space';
import Image from 'next/image';
import { Button, Switch } from '@sd/ui';
import { Check } from 'phosphor-react';
import clsx from 'clsx';
import { useState } from 'react';

export default function PricingPage() {
	const [toggle, setToggle] = useState<boolean>(false)
	return (
		<>
			<Head>
			<title>Pricing - Spacedrive</title>
				<meta
					name="description"
					content="Spacedrive pricing and packages"
				/>
			</Head>
			<div className='opacity-60'>
				<Space />
			</div>
			<PageWrapper>
			<Image
					loading="eager"
					className="top-0 absolute-horizontal-center fade-in"
					width={1278}
					height={626}
					alt="l"
					src="/images/headergradient.webp"
				/>
				<div className='relative mt-48 z-5'>
				<h1 className="px-2 mb-3 text-4xl font-bold leading-tight text-center text-transparent fade-in-heading bg-gradient-to-r from-white from-40%
				to-indigo-400 to-60% bg-clip-text">
						Pricing
					</h1>
					<p className='z-30 max-w-2xl px-2 mx-auto mt-1 mb-8 text-center animation-delay-1 fade-in-heading text-md leading-2 text-gray-450 lg:text-lg lg:leading-8'>
						Spacedrive can be used for free as you like. Upgrading gives you access
							to early features, and this is placeholder text</p>
							<div className='flex items-center justify-center w-full gap-3 mx-auto fade-in-heading animation-delay-2'>
								<p className='text-sm font-medium text-white'>Monthly</p>
								<Switch onCheckedChange={setToggle} checked={toggle} size="lg"/>
								<p className='text-sm font-medium text-white'>Yearly</p>
							</div>
							<div className='flex-col md:flex-row w-full max-w-[1000px] fade-in-heading animation-delay-2
							 items-center px-2 justify-center mx-auto flex gap-10 mb-[200px] mt-[75px]'>
							<PackageCard
							features={[
								"lorem ipsum text",
								"lorem ipsum text",
								"lorem ipsum text",
								"lorem ipsum text",
								"lorem ipsum text",
								"lorem ipsum text"
							]}
							toggle={toggle}
							name="Free"
							price={{
								monthly: "0.00",
								yearly: "0.00",
							}}
							/>
							<PackageCard
							features={[
								"lorem ipsum text",
								"lorem ipsum text",
								"lorem ipsum text",
								"lorem ipsum text",
								"lorem ipsum text",
								"lorem ipsum text"
							]}
							toggle={toggle}
							name="Pro"
							price={{
								monthly: "14.99",
								yearly: "99.99"
							}}
							/>
							<PackageCard
							features={[
								"lorem ipsum text",
								"lorem ipsum text",
								"lorem ipsum text",
								"lorem ipsum text",
								"lorem ipsum text",
								"lorem ipsum text"
							]}
							toggle={toggle}
							name="Enterprise"
							/>
							</div>
				</div>
				</PageWrapper>
		</>
	);
};

interface Props {
	features: string[];
	name: 'Free' | 'Pro' | 'Enterprise';
	price?:
	{
		monthly: string,
		yearly: string
	}
	toggle: boolean;
}

const PackageCard = ({features, name, price, toggle}: Props) => {
	const duration = toggle ? 'year' : 'month'
	return (
		<div className={clsx('bg-[#0E0D1B] w-full max-w-[300px] h-auto',
		'rounded-md relative', name === 'Pro' ? 'pro-card-border-gradient pro-card-shadow' : 'border border-[#222041]')}>
			{name === 'Pro' && (
				<div className='pro-card-border-gradient rounded-[6px] popular-shadow bg-[#0E0D1B]
				 absolute-horizontal-center top-[-12px] px-5 py-1'>
					<p className='text-[10px] text-white uppercase font-medium'>Popular</p>
					</div>
			)}
				<div className='relative h-fit z-2'>
				<div className='border-b border-b-[#222041] h-[138px] w-[99.4%] mx-auto'>
					<div className='flex flex-col items-center justify-center py-6'>
					<p className='uppercase text-[#A7ADD2] text-md mb-4'>{name}</p>
					{
					price ? <>
						<p className='text-2xl font-bold text-white leading-[1]'>${toggle ? price.yearly : price.monthly}</p>
					<p className='text-[#A7ADD2] text-md'>per {duration}</p>
					</>
					:
					<p className='text-2xl font-bold text-white leading-[1]'>Contact Sales</p>
					}
					</div>
				</div>
				<div className="h-full px-3 pb-8 text-center pt-14">
				<div className='flex flex-col items-start justify-center h-[200px] gap-3 mb-20 w-fit mx-auto'>
					{name === 'Pro' && <p className='text-sm text-white'>Everything in <b>Free</b>, plus...</p>}
					{name === 'Enterprise' && <p className='text-sm text-white'>Everything in <b>Pro</b>, plus...</p>}
				{features.map((feature, index) => (
					<div key={index} className='flex items-center justify-center gap-2.5'>
					<div className='w-5 h-5 bg-[#2A2741] border border-[#353252] flex items-center justify-center rounded-full'>
						<Check weight='bold' size={12} color="white"/>
					</div>
					<p className='text-sm text-white'>{feature}</p>
					</div>
				))}
				</div>
				<Button className={clsx('h-[35px] px-3', name === 'Pro' && 'bg-gradient-to-r from-violet-500 to-blur-500 border-0')} variant="accent">{
					name === 'Free' && 'Try for free'
					|| name === 'Pro' && 'Subscribe'
					|| name === 'Enterprise' && 'Contact us'
				}</Button>
				</div>
				</div>
		</div>
	)
}
