/* eslint-disable tailwindcss/enforces-negative-arbitrary-values */

/* eslint-disable tailwindcss/classnames-order */

/* eslint-disable jsx-a11y/alt-text */
import { Apple, Github } from '@sd/assets/svgs/brands';
import clsx from 'clsx';
import { motion } from 'framer-motion';
import { useInView } from 'framer-motion';
import dynamic from 'next/dynamic';
import Head from 'next/head';
import Image from 'next/image';
import { AndroidLogo, AppleLogo, Download, Globe, LinuxLogo, WindowsLogo } from 'phosphor-react';
import { IconProps } from 'phosphor-react';
import { FunctionComponent, forwardRef, memo, useEffect, useRef, useState } from 'react';
import { Tooltip, tw } from '@sd/ui';
import BentoBoxes from '~/components/BentoBoxes';
import CloudStorage from '~/components/CloudStorage';
import DownloadToday from '~/components/DownloadToday';
import NewBanner from '~/components/NewBanner';
import PageWrapper from '~/components/PageWrapper';
import Space from '~/components/Space';
import WormHole from '~/components/WormHole';
import CyclingImage from '../components/CyclingImage';

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
	{ name: 'iOS and macOS', icon: Apple, url: 'https://www.github.com' },
	{ name: 'Windows', icon: WindowsLogo, url: 'https://www.github.com' },
	{ name: 'Linux', icon: LinuxLogo, url: 'https://www.github.com' },
	{ name: 'Android', icon: AndroidLogo, url: 'https://www.google.com' },
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
				<div className="flex w-full flex-col items-center px-4">
					<div className="mt-22 lg:mt-28" id="content" aria-hidden="true" />
					<div className="mt-24 lg:mt-8" />
					<NewBanner
						headline="Alpha release is finally here!"
						href="/blog/october-alpha-release"
						link="Read post"
						className="mt-[50px] lg:mt-0"
					/>

					<h1 className="fade-in-heading z-30 mb-3 bg-clip-text px-2 text-center text-4xl font-bold leading-tight text-transparent text-white md:text-5xl lg:text-7xl">
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
					<div className="flex flex-row gap-3">
						<Link
							target="_blank"
							href={
								deviceOs?.isWindows
									? 'https://www.google.com'
									: 'https://www.github.com'
							}
						>
							<HomeCTA
								icon={deviceOs?.isWindows ? <WindowsLogo /> : <Apple />}
								className="z-5 relative"
								text={
									deviceOs?.isWindows
										? 'Download for Windows'
										: 'Download for Mac'
								}
							/>
						</Link>
						<Link
							target="_blank"
							href={
								deviceOs?.isWindows
									? 'https://www.google.com'
									: 'https://www.github.com'
							}
						>
							<HomeCTA
								icon={<Github />}
								className="z-5 relative"
								text="Star on GitHub"
							/>
						</Link>
					</div>
					<p
						className={clsx(
							'animation-delay-3 z-30 mt-3 px-6 text-center text-sm text-gray-400 fade-in'
						)}
					>
						Alpha v0.1.4 <span className="mx-2 opacity-50">|</span> macOS 12+
					</p>
					<div className="relative z-10 mt-5 flex gap-3">
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
						 sm:h-[428px] md:mt-[75px] md:h-[428px] lg:h-auto"
						>
							<Image
								loading="eager"
								className="absolute-horizontal-center animation-delay-2 fade-in xs:top-[360px] md:top-[130px]"
								width={1200}
								height={626}
								alt="l"
								src="/images/appgradient.webp"
							/>
							<AppFrameOuter className=" relative overflow-hidden transition-transform duration-700 ease-in-out hover:-translate-y-4 hover:scale-[1.02]">
								<AppFrameInner>
									<CyclingImage
										loading="eager"
										width={1278}
										height={626}
										alt="spacedrive app"
										className=" rounded-lg "
										images={['/images/app.webp']}
									/>
									<Image
										loading="eager"
										className="absolute opacity-100 transition-opacity duration-1000 ease-in-out hover:opacity-0 md:w-auto"
										width={2278}
										height={626}
										alt="l"
										src="/images/appgradientoverlay.png"
									/>
								</AppFrameInner>
							</AppFrameOuter>
						</div>
					</div>

					{/* <WormHole /> */}
					{/* <BentoBoxes /> */}
					{/* <CloudStorage /> */}
					{/* <DownloadToday isWindows={deviceOs?.isWindows} /> */}
					<div className="h-[200px] w-full" />
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

const Platform = ({ icon: Icon, url, label }: Props) => {
	return (
		<Tooltip label={label}>
			<Link aria-label={label} href={url} target="_blank">
				<Icon size={25} className="h-[25px] w-full opacity-80" weight="fill" />
			</Link>
		</Tooltip>
	);
};
