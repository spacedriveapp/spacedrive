import { ReactComponent as Info } from '@sd/assets/svgs/info.svg';
import clsx from 'clsx';
import Head from 'next/head';
import Link from 'next/link';
import { useRouter } from 'next/router';
import { useEffect, useState } from 'react';
import AppImage from '~/components/AppImage';
import HomeCTA from '~/components/HomeCTA';
import NewBanner from '~/components/NewBanner';
import PageWrapper from '~/components/PageWrapper';
import { detectWebGLContext, getWindow } from '~/utils/util';

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
				<p className="text-md mt-5 text-gray-450 sm:text-xl">{props.description}</p>
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

export default function HomePage() {
	const [unsubscribedFromWaitlist, setUnsubscribedFromWaitlist] = useState(false);
	const [background, setBackground] = useState<JSX.Element | null>(null);

	const router = useRouter();

	useEffect(() => {
		if (!getWindow()) return;
		const cuid = router.query.wunsub;
		if (!cuid) return;
		(async () => {
			console.log('Unsubscribing from waitlist', process.env.NODE_ENV);
			const prod = process.env.NODE_ENV === 'production';

			const req = await fetch(`/api/waitlist?i=${cuid}`, {
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
	}, [router.query.wunsub]);

	useEffect(() => {
		if (!(getWindow() && background == null)) return;
		(async () => {
			if (detectWebGLContext()) {
				const Space = (await import('~/components/Space')).Space;
				setBackground(<Space />);
			} else {
				console.warn('Fallback to Bubbles background due WebGL not being available');
				const Bubbles = (await import('~/components/Bubbles')).Bubbles;
				setBackground(<Bubbles />);
			}
		})();
	}, [background]);

	return (
		<PageWrapper>
			<div className="flex w-full flex-col items-center px-4">
				<Head>
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
				</Head>
				<div className="mt-22 lg:mt-28" id="content" aria-hidden="true" />
				<div className="mt-24 lg:mt-8" />
				<NewBanner
					headline="Spacedrive raises $2M led by OSS Capital"
					href="/blog/spacedrive-funding-announcement"
					link="Read post"
				/>

				{unsubscribedFromWaitlist && (
					<div
						className={
							'my-2 -mt-8 flex flex-row items-center rounded-md border-2 border-green-900 bg-green-800/20 px-2'
						}
					>
						<Info className="mr-1 w-5 fill-green-500" />
						<p className={'text-sm text-green-500'}>
							You have been unsubscribed from the waitlist
						</p>
					</div>
				)}

				<h1 className="fade-in-heading z-30 mb-3 px-2 text-center text-4xl font-black leading-tight text-white md:text-7xl">
					A file explorer from the future.
				</h1>
				<p className="animation-delay-1 fade-in-heading text-md leading-2 z-30 mb-8 mt-1 max-w-4xl text-center text-gray-450 lg:text-lg lg:leading-8">
					Combine your drives and clouds into one database that you can organize and
					explore from any device.
					<br />
					<span className="hidden sm:block">
						Designed for creators, hoarders and the painfully disorganized.
					</span>
				</p>
				<HomeCTA />
				<AppImage />
				<Section
					orientation="right"
					heading="Never leave a file behind."
					className="z-30 mt-0 sm:mt-8"
					description={
						<>
							Spacedrive accounts for every file you own, uniquely fingerprinting and
							extracting metadata so you can sort, tag, backup and share files without
							limitations of any one cloud provider.
							<br />
							<br />
							<Link
								className="text-primary-600 transition hover:text-primary-500"
								href="/docs/product/getting-started/introduction"
							>
								Find out more →
							</Link>
						</>
					}
				/>
				{background}
			</div>
		</PageWrapper>
	);
}
