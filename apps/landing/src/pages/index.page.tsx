import { ReactComponent as Info } from '@sd/assets/svgs/info.svg';
import clsx from 'clsx';
import { useEffect, useState } from 'react';
import { Helmet } from 'react-helmet';
import AppEmbed from '../components/AppEmbed';
import { Bubbles } from '../components/Bubbles';
// import { Bubbles } from '../components/Bubbles';
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
	const info = (
		<div className="px-4 py-10 sm:px-10">
			{props.heading && <h1 className="text-2xl font-black sm:text-4xl">{props.heading}</h1>}
			{props.description && (
				<p className="text-md text-gray-450 mt-5 sm:text-xl">{props.description}</p>
			)}
		</div>
	);
	return (
		<div className={clsx('my-10 grid grid-cols-1 lg:my-44 lg:grid-cols-2', props.className)}>
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
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);

	return (
		<div className="flex w-full flex-col items-center px-4">
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
						'bg-opacity/20 my-2 -mt-8 flex flex-row items-center rounded-md border-2 border-green-900 bg-green-800 px-2'
					}
				>
					<Info className="mr-1 w-5 fill-green-500" />
					<p className={'text-sm text-green-500'}>You have been unsubscribed from the waitlist</p>
				</div>
			)}

			<h1 className="fade-in-heading z-30 mb-3 px-2 text-center text-4xl font-black leading-tight text-white md:text-6xl">
				A file explorer from the future.
			</h1>
			<p className="animation-delay-1 fade-in-heading text-md leading-2 text-gray-450 z-30 mt-1 mb-8 max-w-4xl text-center lg:text-lg lg:leading-8">
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
							className="text-primary-600 hover:text-primary-500 transition"
							href="/docs/product/getting-started/introduction"
						>
							Find out more →
						</a>
					</>
				}
			/>
			<Bubbles />
		</div>
	);
}

export { Page };
