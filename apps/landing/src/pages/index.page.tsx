import clsx from 'clsx';
import React, { Suspense, useEffect, useState } from 'react';
import { Helmet } from 'react-helmet';

import { ReactComponent as Info } from '@sd/interface/assets/svg/info.svg';

import AppEmbed, { AppEmbedPlaceholder } from '../components/AppEmbed';
import { Bubbles } from '../components/Bubbles';
import HomeCTA from '../components/HomeCTA';
import NewBanner from '../components/NewBanner';
import { usePageContext } from '../renderer/usePageContext';
import { getWindow } from '../utils';

interface SectionProps {
	orientation: 'left' | 'right';
	heading?: string;
	description?: string | React.ReactNode;
	children?: React.ReactNode;
	className?: string;
}

function Section(props: SectionProps = { orientation: 'left' }) {
	let info = (
		<div className="px-4 py-10 sm:px-10">
			{props.heading && <h1 className="text-2xl font-black sm:text-4xl">{props.heading}</h1>}
			{props.description && (
				<p className="mt-5 text-md sm:text-xl text-gray-450">{props.description}</p>
			)}
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
	const { urlParsed } = usePageContext();
	const [unsubscribedFromWaitlist, setUnsubscribedFromWaitlist] = useState(false);

	useEffect(() => {
		if (!getWindow()) return;

		const cuid = urlParsed.search?.['wunsub'];
		if (!cuid) return;

		(async () => {
			const prod = import.meta.env.PROD;
			const url = prod ? 'https://waitlist-api.spacedrive.com' : 'http://localhost:3000';

			const req = await fetch(`${url}/api/waitlist?i=${cuid}`, {
				method: 'DELETE'
			});

			if (req.status === 200) {
				setUnsubscribedFromWaitlist(true);
				window.history.replaceState(
					{},
					'',
					prod ? 'https://spacedrive.com' : 'http://localhost:8003'
				);

				setTimeout(() => {
					setUnsubscribedFromWaitlist(false);
				}, 5000);
			} else if (req.status >= 400 && req.status < 500) {
				alert('An error occurred while unsubscribing from waitlist');
			}
		})();
	}, []);

	return (
		<>
			<Helmet>
				<title>Spacedrive — A file manager from the future.</title>
				<meta
					name="description"
					content="Combine your drives and clouds into one database that you can organize and explore from any device. Designed for creators, hoarders and the painfully disorganized."
				/>
				<meta
					property="og:image"
					content="https://raw.githubusercontent.com/spacedriveapp/.github/main/profile/spacedrive_icon.png"
				/>
				<meta
					name="keywords"
					content="files,file manager,spacedrive,file explorer,vdfs,distributed filesystem,cas,content addressable storage,virtual filesystem,photos app, video organizer,video encoder,tags,tag based filesystem"
				/>
				<meta name="author" content="Spacedrive Technology Inc." />
			</Helmet>
			<div className="mt-22 lg:mt-28" id="content" aria-hidden="true" />
			<div className="mt-24 lg:mt-5" />
			<NewBanner
				headline="Spacedrive raises $2M led by OSS Capital"
				href="/blog/spacedrive-funding-announcement"
				link="Read post"
			/>
			{unsubscribedFromWaitlist && (
				<div
					className={
						'-mt-8 flex flex-row items-center bg-opacity-20 border-2 my-2 px-2 rounded-md bg-green-800 border-green-900'
					}
				>
					<Info className="w-5 mr-1 fill-green-500" />
					<p className={'text-sm text-green-500'}>You have been unsubscribed from the waitlist</p>
				</div>
			)}

			<h1 className="z-30 px-2 mb-3 text-4xl font-black leading-tight text-center text-white fade-in-heading md:text-6xl">
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
				className="z-30 mt-0 sm:mt-8"
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

export { Page };
