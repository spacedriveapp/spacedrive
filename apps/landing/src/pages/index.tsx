/* eslint-disable tailwindcss/enforces-negative-arbitrary-values */

/* eslint-disable tailwindcss/classnames-order */

/* eslint-disable jsx-a11y/alt-text */
import { Apple } from '@sd/assets/svgs/brands';
import clsx from 'clsx';
import { motion } from 'framer-motion';
import dynamic from 'next/dynamic';
import Head from 'next/head';
import Image from 'next/image';
import { AndroidLogo, Download, Globe, LinuxLogo, WindowsLogo } from 'phosphor-react';
import { IconProps } from 'phosphor-react';
import { FunctionComponent, forwardRef, memo, useEffect, useState } from 'react';
import { Tooltip, tw } from '@sd/ui';
import BentoBoxes from '~/components/BentoBoxes';
import CloudStorage from '~/components/CloudStorage';
import DownloadToday from '~/components/DownloadToday';
import NewBanner from '~/components/NewBanner';
import PageWrapper from '~/components/PageWrapper';
import Space from '~/components/Space';

const Link = dynamic(() => import('next/link'), {
	ssr: false
});
const HomeCTA = dynamic(() => import('~/components/HomeCTA'), {
	ssr: false
});

const ExplainerHeading = tw.h1`z-30 mb-3 px-2 text-center text-3xl font-black leading-tight text-white`;
const ExplainerText = tw.p`leading-2 z-30 mb-8 mt-1 max-w-4xl text-center text-gray-450"`;

const AppFrameOuter = tw.div`relative m-auto flex w-full max-w-7xl rounded-lg transition-opacity`;
const AppFrameInner = tw.div`z-30 flex w-full rounded-lg border-t border-app-line/50 backdrop-blur`;

const platforms = [
	{ name: 'Android', icon: AndroidLogo, url: 'https://www.google.com' },
	{ name: 'iOS', icon: Apple, url: 'https://www.github.com' },
	{ name: 'Windows', icon: WindowsLogo, url: 'https://www.github.com' },
	{ name: 'Linux', icon: LinuxLogo, url: 'https://www.github.com' },
	{ name: 'Web', icon: Globe, url: 'https://www.github.com' }
];

export default function HomePage() {
	const [opacity, setOpacity] = useState(0.6);
	const [deviceOs, setDeviceOs] = useState<null | {
		isWindows: boolean;
		isMacOs: boolean;
		isMobile: boolean;
	}>(null);
	useEffect(() => {
		(async () => {
			const os = await import('react-device-detect').then(
				({ isWindows, isMacOs, isMobile }) => {
					return { isWindows, isMacOs, isMobile };
				}
			);
			setDeviceOs({
				isWindows: os.isWindows,
				isMacOs: os.isMacOs,
				isMobile: os.isMobile
			});
		})();
		const fadeStart = 300; // start fading out at 100px
		const fadeEnd = 1300; // end fading out at 300px

		const handleScroll = () => {
			const currentScrollY = window.scrollY;

			if (currentScrollY <= fadeStart) {
				setOpacity(0.6);
			} else if (currentScrollY <= fadeEnd) {
				const range = fadeEnd - fadeStart;
				const diff = currentScrollY - fadeStart;
				const ratio = diff / range;
				setOpacity(0.6 - ratio);
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
				{/* <div
					className="absolute-horizontal-center h-[140px] w-[60%] overflow-hidden
				rounded-full bg-gradient-to-r from-indigo-500 to-fuchsia-500 opacity-60 blur-[80px] md:blur-[150px]"
				/> */}
				<Image
					loading="eager"
					className="absolute-horizontal-center fade-in"
					width={1278}
					height={626}
					alt="l"
					src="/images/headergradient.webp"
				/>
				<div className="flex flex-col items-center w-full px-4">
					<div className="mt-22 lg:mt-28" id="content" aria-hidden="true" />
					<div className="mt-24 lg:mt-8" />
					<NewBanner
						headline="Alpha has been released"
						href="/blog/spacedrive-funding-announcement"
						link="Read post"
						className="mt-[50px] lg:mt-0"
					/>

					<h1 className="z-30 px-2 mb-3 text-4xl font-bold leading-tight text-center text-transparent fade-in-heading bg-gradient-to-r from-white to-indigo-400 bg-clip-text md:text-5xl lg:text-7xl">
						One Explorer. All Your Files.
					</h1>
					<p className="z-30 max-w-4xl mt-1 mb-8 text-center animation-delay-1 fade-in-heading text-md leading-2 text-gray-450 lg:text-lg lg:leading-8">
						Unify files from all your devices and clouds into a single, easy-to-use
						explorer.
						<br />
						<span className="hidden sm:block">
							Designed for creators, hoarders and the painfully disorganized.
						</span>
					</p>
					<Link
						target="_blank"
						href={
							deviceOs?.isWindows
								? 'https://www.google.com'
								: 'https://www.github.com'
						}
					>
						<HomeCTA
							icon={<Download />}
							className="relative z-5"
							text={deviceOs?.isWindows ? 'Download on Windows' : 'Download on Mac'}
						/>
					</Link>
					<p
						className={clsx(
							'animation-delay-3 z-30 mt-3 px-6 text-center text-sm text-gray-400 fade-in'
						)}
					>
						Alpha v0.1.4 <span className="mx-2 opacity-50">|</span> macOS 12+
					</p>
					<div className="relative z-10 flex gap-3 mt-5">
						{platforms.map((platform, i) => (
							<motion.div
								initial={{ opacity: 0, y: 20 }}
								animate={{ opacity: 1, y: 0 }}
								transition={{ delay: i * 0.2, ease: 'easeInOut' }}
								key={platform.name}
							>
								<Platform
									icon={platform.icon}
									url={platform.url}
									label={platform.name}
								/>
							</motion.div>
						))}
					</div>
					<div>
						<div
							className="xl2:relative z-30 flex h-[255px] w-full px-6
						 sm:h-[428px] md:mt-[75px] md:h-[428px] lg:h-[628px]"
						>
							<Image
								loading="eager"
								className="absolute-horizontal-center animation-delay-2 top-[380px] w-[400px] fade-in xs:top-[360px] md:top-[130px] md:w-auto"
								width={1200}
								height={626}
								alt="l"
								src="/images/appgradient.webp"
							/>
							<AppFrameOuter className="relative overflow-hidden fade-in-heading animation-delay-2">
								<LineAnimation />
								<AppFrameInner>
									<Image
										loading="eager"
										width={1278}
										height={626}
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
					<DownloadToday isWindows={deviceOs?.isWindows} />
				</div>
			</PageWrapper>
		</>
	);
}

interface Props {
	icon: FunctionComponent<IconProps>;
	url: string;
	label: string;
}

const Platform = forwardRef<HTMLAnchorElement, Props>(({ icon: Icon, url, label }, ref) => {
	return (
		<Tooltip label={label}>
			<Link ref={ref} href={url} target="_blank">
				<Icon size={20} className="opacity-80" weight="fill" />
			</Link>
		</Tooltip>
	);
});

Platform.displayName = 'Platform';

const LineAnimation = memo(() => {
	const [numberOfLines, setNumberOfLines] = useState(1);
	const [isMounted, setIsMounted] = useState(false);
	useEffect(() => {
		setIsMounted(true);
		const randomlySetNumberOfLines = () => {
			return setInterval(() => {
				setNumberOfLines(Math.floor(Math.random() * 3));
			}, 5000);
		};
		randomlySetNumberOfLines();
		return () => clearInterval(randomlySetNumberOfLines());
	}, []);
	return (
		<>
			{[...Array(numberOfLines)].map((_, i) => (
				<div
					key={i}
					style={{
						animation: isMounted
							? `left-line-animation-fade ${Math.floor(
									Math.random() + Math.floor(Math.random() * 2) + 2
							  )}s ease-in-out infinite`
							: '',
						animationDelay: `${Math.floor(Math.random() * 3)}s`,
						width: `${isMounted && Math.floor(Math.random() * 50) + 50}px`,
						height: '1px',
						top: 0,
						position: 'absolute',
						zIndex: 50
					}}
					className="opacity-0 left-line bg-gradient-to-r from-transparent to-white/50"
				/>
			))}
			{[...Array(numberOfLines)].map((_, i) => (
				<div
					key={i}
					style={{
						animation: isMounted
							? `top-line-animation-fade ${Math.floor(
									Math.random() + Math.floor(Math.random() * 2) + 2
							  )}s ease-in-out infinite`
							: '',
						animationDelay: `${Math.floor(Math.random() * 3)}s`,
						height: `${isMounted && Math.floor(Math.random() * 2) + 30}px`,
						width: '1px',
						right: 0,
						position: 'absolute',
						zIndex: 50
					}}
					className="opacity-0 bg-gradient-to-b from-transparent to-white/50"
				/>
			))}
		</>
	);
});

LineAnimation.displayName = 'LineAnimation';
