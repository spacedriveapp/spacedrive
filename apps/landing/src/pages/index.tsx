/* eslint-disable tailwindcss/enforces-negative-arbitrary-values */

/* eslint-disable tailwindcss/classnames-order */

/* eslint-disable jsx-a11y/alt-text */
import clsx from 'clsx';
import Head from 'next/head';
import Image from 'next/image';
import { Download } from 'phosphor-react';
import { useEffect, useState } from 'react';
import { tw } from '@sd/ui';
import AccessData from '~/components/AccessData';
import BentoBoxes from '~/components/BentoBoxes';
import CloudStorage from '~/components/CloudStorage';
import DownloadToday from '~/components/DownloadToday';
import HomeCTA from '~/components/HomeCTA';
import NewBanner from '~/components/NewBanner';
import PageWrapper from '~/components/PageWrapper';
import Space from '~/components/Space';

const ExplainerHeading = tw.h1`z-30 mb-3 px-2 text-center text-3xl font-black leading-tight text-white`;
const ExplainerText = tw.p`leading-2 z-30 mb-8 mt-1 max-w-4xl text-center text-gray-450"`;

const AppFrameOuter = tw.div`relative m-auto flex w-full max-w-7xl rounded-lg border border-black transition-opacity`;
const AppFrameInner = tw.div`z-30 flex w-full rounded-lg border-t border-app-line/50 bg-app/30 backdrop-blur`;

export default function HomePage() {
	const [opacity, setOpacity] = useState(1);

	useEffect(() => {
		const fadeStart = 300; // start fading out at 100px
		const fadeEnd = 1300; // end fading out at 300px

		const handleScroll = () => {
			const currentScrollY = window.scrollY;

			if (currentScrollY <= fadeStart) {
				setOpacity(1);
			} else if (currentScrollY <= fadeEnd) {
				const range = fadeEnd - fadeStart;
				const diff = currentScrollY - fadeStart;
				const ratio = diff / range;
				setOpacity(1 - ratio);
			} else {
				setOpacity(0);
			}
		};

		window.addEventListener('scroll', handleScroll);

		return () => {
			window.removeEventListener('scroll', handleScroll);
		};
	}, []);

	return (
		<>
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
			<div style={{ opacity }}>
				<Space />
			</div>

			<PageWrapper>
				<div
					className="absolute-horizontal-center h-[253px] w-[60%] overflow-hidden
				rounded-full bg-gradient-to-r from-violet-500 to-fuchsia-400 opacity-40 blur-[80px] md:blur-[150px]"
				/>
				<div className="flex w-full flex-col items-center px-4">
					<div className="mt-22 lg:mt-28" id="content" aria-hidden="true" />
					<div className="mt-24 lg:mt-8" />
					<NewBanner
						headline="Alpha has been released"
						href="/blog/spacedrive-funding-announcement"
						link="Read post"
					/>

					<h1 className="fade-in-heading z-30 mb-3 bg-gradient-to-r from-white to-indigo-400 bg-clip-text px-2 text-center text-4xl font-bold leading-tight text-transparent md:text-5xl lg:text-7xl">
						One Explorer. All Your Files.
					</h1>
					<p className="animation-delay-1 fade-in-heading text-md leading-2 z-30 mb-8 mt-1 max-w-4xl text-center text-gray-450 lg:text-lg lg:leading-8">
						Unify files from all your devices and clouds into a single, easy-to-use
						explorer.
						<br />
						<span className="hidden sm:block">
							Designed for creators, hoarders and the painfully disorganized.
						</span>
					</p>
					<HomeCTA icon={<Download />} text="Download on Mac" />
					<p
						className={clsx(
							'animation-delay-3 z-30 mt-3 px-6 text-center text-sm text-gray-400 fade-in'
						)}
					>
						Alpha v0.1.4 <span className="mx-2 opacity-50">|</span> macOS 12+
					</p>

					<div>
						<div
							className="absolute-horizontal-center top-[400px] h-[10%] w-[70%] overflow-hidden rounded-full bg-gradient-to-r
						from-transparent to-indigo-400 blur-[50px] md:h-[500px] md:blur-[150px] lg:top-[550px]
						lg:h-[526px] lg:w-[728px]"
						/>
						<div
							className="absolute-horizontal-center top-[500px] h-[12%] w-[60%] overflow-hidden rounded-full bg-gradient-to-r from-violet-900
						to-fuchsia-400 blur-[50px] md:top-[900px] md:h-[150px] md:blur-[150px]
						lg:h-[250px] lg:w-[600px] xl:h-[400px] xl:w-[900px]"
						/>
						<video
							className="absolute-horizontal-center pointer-events-none w-[1000px]"
							src="/images/ball.webm"
							autoPlay
							muted
							playsInline
							loop
						/>
						<div
							className="xl2: z-30 mt-[24%] flex h-[255px] w-full px-6
						 xs:mt-[170px] sm:mt-20 sm:h-[428px] md:mt-[250px] md:h-[428px] lg:mt-[310px] lg:h-[628px]"
						>
							<AppFrameOuter>
								<AppFrameInner>
									<Image
										width={1278}
										height={626}
										quality={100}
										alt="l"
										className="rounded-lg"
										src="/images/app.webp"
									/>
								</AppFrameInner>
							</AppFrameOuter>
						</div>
					</div>
					<BentoBoxes />
					<CloudStorage />
					<AccessData />
					<DownloadToday />
				</div>
			</PageWrapper>
		</>
	);
}
