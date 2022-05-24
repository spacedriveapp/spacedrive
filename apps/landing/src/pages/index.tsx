import { Apple, Github, Linux, Windows } from '@icons-pack/react-simple-icons';
import { Button, Input } from '@sd/ui';
import clsx from 'clsx';
import React, { useEffect } from 'react';
import { useState } from 'react';

import AppEmbed from '../components/AppEmbed';
import { Bubbles } from '../components/Bubbles';
import { Footer } from '../components/Footer';
import HomeCTA from '../components/HomeCTA';
import NavBar from '../components/NavBar';
import NewBanner from '../components/NewBanner';

interface SectionProps {
	orientation: 'left' | 'right';
	heading?: string;
	description?: string | React.ReactNode;
	children?: React.ReactNode;
	className?: string;
}

function Section(props: SectionProps = { orientation: 'left' }) {
	let info = (
		<div className="p-10">
			{props.heading && <h1 className="text-4xl font-black">{props.heading}</h1>}
			{props.description && <p className="mt-5 text-xl text-gray-450">{props.description}</p>}
		</div>
	);
	return (
		<div className={clsx('grid grid-cols-1 my-10 lg:grid-cols-2 lg:my-44', props.className)}>
			{props.orientation === 'right' ? (
				<>
					{info}
					{props.children}
				</>
			) : (
				<>
					{props.children}
					{info}
				</>
			)}
		</div>
	);
}

function Page() {
	return (
		<>
			<div className="mt-24 lg:mt-28" />
			<NewBanner />
			<h1
				id="content"
				className="z-30 px-2 mb-3 text-4xl font-black leading-tight text-center fade-in-heading md:text-6xl"
			>
				A file explorer from the future.
			</h1>
			<p className="z-30 max-w-4xl mt-1 mb-8 text-center animation-delay-1 fade-in-heading text-md lg:text-lg leading-2 lg:leading-8 text-gray-450">
				Combine your drives and clouds into one database that you can organize and explore from any
				device.
				<br />
				<span className="hidden sm:block">
					Designed for creators, hoarders and the painfully disorganized.
				</span>
			</p>
			<HomeCTA />
			<AppEmbed />
			<Section
				orientation="right"
				heading="Never leave a file behind."
				className="z-30"
				description={
					<>
						Spacedrive accounts for every file you own, uniquely fingerprinting and extracting
						metadata so you can sort, tag, backup and share files without limitations of any one
						cloud provider.
						<br />
						<br />
						<a
							className="transition text-primary-600 hover:text-primary-500"
							href="https://github.com/spacedriveapp"
							target="_blank"
						>
							Find out more →
						</a>
					</>
				}
			/>
			<Bubbles />
		</>
	);
}

export default Page;
