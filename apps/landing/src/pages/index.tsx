import { AndroidLogo, Globe, LinuxLogo, WindowsLogo } from '@phosphor-icons/react';
import { Apple, Github } from '@sd/assets/svgs/brands';
import { motion } from 'framer-motion';
import dynamic from 'next/dynamic';
import Head from 'next/head';
import Image from 'next/image';
import { ComponentProps, useEffect, useState } from 'react';
import { Tooltip, TooltipProvider, tw } from '@sd/ui';
import NewBanner from '~/components/NewBanner';
import PageWrapper from '~/components/PageWrapper';
import { detectWebGLContext, getLatestSpacedriveVersion, getWindow } from '~/utils/util';

import CyclingImage from '../components/CyclingImage';

const HomeCTA = dynamic(() => import('~/components/HomeCTA'), {
	ssr: false
});

const AppFrameOuter = tw.div`relative m-auto flex w-full max-w-7xl rounded-lg transition-opacity`;
const AppFrameInner = tw.div`z-30 flex w-full rounded-lg border-t border-app-line/50 backdrop-blur`;

const BASE_DL_LINK = '/api/releases/desktop/stable';
const downloadEntries = {
	linux: {
		name: 'Linux',
		icon: <LinuxLogo />,
		links: 'linux/x86_64'
	},
	macOS: {
		name: 'MacOS',
		icon: <Apple />,
		links: {
			'Intel': 'darwin/x86_64',
			'Apple Silicon': 'darwin/aarch64'
		}
	},
	windows: {
		name: 'Windows',
		icon: <WindowsLogo />,
		links: 'windows/x86_64'
	}
} as const;

const platforms = [
	{ name: 'MacOS', icon: Apple, clickable: true, version: '12+' },
	{
		name: 'Windows',
		icon: WindowsLogo,
		href: `${BASE_DL_LINK}/${downloadEntries.windows.links}`,
		version: '10+'
	},
	{
		name: 'Linux',
		icon: LinuxLogo,
		href: `${BASE_DL_LINK}/${downloadEntries.linux.links}`,
		version: 'AppImage'
	},
	{ name: 'Android', icon: AndroidLogo, version: '10+' },
	{ name: 'Web', icon: Globe }
] as const;

export default function HomePage() {
	const [opacity, setOpacity] = useState(0.6);
	const [background, setBackground] = useState<JSX.Element | null>(null);
	const [multipleDownloads, setMultipleDownloads] =
		useState<(typeof downloadEntries)['linux' | 'macOS']['links']>();
	const [downloadEntry, setDownloadEntry] =
		useState<(typeof downloadEntries)['linux' | 'macOS' | 'windows']>();
	const [isWindowResizing, setIsWindowResizing] = useState(false);

	const links = downloadEntry?.links;

	const [spacedriveVersion, setSpacedriveVersion] = useState("Alpha");

	useEffect(() => {
		import('react-device-detect').then(({ isWindows, isMacOs, isMobile }) => {
			if (isWindows) {
				setDownloadEntry(downloadEntries.windows);
			} else if (isMacOs) {
				setDownloadEntry(downloadEntries.macOS);
			} else if (!isMobile) {
				setDownloadEntry(downloadEntries.linux);
			}
		});

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

	useEffect(() => {
		let resizeTimer: NodeJS.Timeout;
		const handleResize = () => {
			setIsWindowResizing(true);
			setBackground(null);
			clearTimeout(resizeTimer);
			resizeTimer = setTimeout(() => {
				setIsWindowResizing(false);
			}, 100);
		};
		window.addEventListener('resize', handleResize);
		return () => {
			window.removeEventListener('resize', handleResize);
			clearTimeout(resizeTimer);
		};
	}, []);

	useEffect(() => {
		getLatestSpacedriveVersion().then((res) => setSpacedriveVersion(res));
	}, []);

	useEffect(() => {
		if (isWindowResizing) return;
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
	}, [background, isWindowResizing]);

	const currentPlatform =
		downloadEntry !== undefined
			? platforms.find((e) => e.name === downloadEntry.name)
			: undefined;

	const supportedVersion =
		currentPlatform && 'version' in currentPlatform ? currentPlatform.version : undefined;

	const formattedVersion =
		downloadEntry && supportedVersion && downloadEntry.name !== 'Linux'
			? `${downloadEntry.name} ${supportedVersion}`
			: supportedVersion;

	return (
		<TooltipProvider>
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

			<div style={{ opacity }}>{background}</div>

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

					<h1 className="fade-in-heading z-30 mb-3 bg-clip-text px-2 text-center text-4xl font-bold leading-tight text-white md:text-5xl lg:text-7xl">
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
						{!(downloadEntry && links) ? null : typeof links === 'string' ? (
							<a target="_blank" href={`${BASE_DL_LINK}/${links}`}>
								<HomeCTA
									icon={downloadEntry.icon}
									text={`Download for ${downloadEntry.name}`}
									className="z-5 relative"
								/>
							</a>
						) : (
							<HomeCTA
								icon={downloadEntry.icon}
								text={`Download for ${downloadEntry.name}`}
								onClick={() =>
									setMultipleDownloads(multipleDownloads ? undefined : links)
								}
							/>
						)}

						<a target="_blank" href="https://www.github.com/spacedriveapp/spacedrive">
							<HomeCTA
								icon={<Github />}
								className="z-5 relative"
								text="Star on GitHub"
							/>
						</a>
					</div>

					{multipleDownloads && (
						<div className="z-50 mb-2 mt-4 flex flex-row gap-3 fade-in">
							{Object.entries(multipleDownloads).map(([name, link]) => (
								<a key={name} target="_blank" href={`${BASE_DL_LINK}/${link}`}>
									<HomeCTA
										size="md"
										text={name}
										className="z-5 relative !py-1 !text-sm"
									/>
								</a>
							))}
						</div>
					)}
					<p
						className={
							'animation-delay-3 z-30 mt-3 px-6 text-center text-sm text-gray-400 fade-in'
						}
					>
						{spacedriveVersion}
						{formattedVersion && (
							<>
								<span className="mx-2 opacity-50">|</span>
								{formattedVersion}
							</>
						)}
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
									label={platform.name}
									href={'href' in platform ? platform.href : undefined}
									iconDisabled={
										platform.name === 'Android' || platform.name === 'Web'
											? true
											: undefined
									}
									clickable={
										'clickable' in platform ? platform.clickable : undefined
									}
									onClick={() => {
										if (platform.name === 'MacOS') {
											setMultipleDownloads(
												multipleDownloads
													? undefined
													: downloadEntries.macOS.links
											);
										}
									}}
								/>
							</motion.div>
						))}
					</div>
					<div className="pb-6 xs:pb-24">
						<div
							className="xl2:relative z-30 flex h-[255px] w-full px-6
						 sm:h-[428px] md:mt-[75px] md:h-[428px] lg:h-auto"
						>
							<Image
								loading="eager"
								className="absolute-horizontal-center animation-delay-2 top-[380px] fade-in xs:top-[180px] md:top-[130px]"
								width={1200}
								height={626}
								alt="l"
								src="/images/appgradient.webp"
							/>
							<AppFrameOuter
								className=" relative mt-10 overflow-hidden
							transition-transform duration-700 ease-in-out hover:-translate-y-4 hover:scale-[1.02] md:mt-0"
							>
								<AppFrameInner>
									<CyclingImage
										loading="eager"
										width={1278}
										height={626}
										alt="spacedrive app"
										className="rounded-lg"
										images={[
											'/images/app/1.webp',
											'/images/app/2.webp',
											'/images/app/3.webp',
											'/images/app/4.webp',
											'/images/app/5.webp',
											'/images/app/10.webp',
											'/images/app/6.webp',
											'/images/app/7.webp',
											'/images/app/8.webp',
											'/images/app/9.webp'
										]}
									/>
									<Image
										loading="eager"
										className="pointer-events-none absolute opacity-100 transition-opacity duration-1000 ease-in-out hover:opacity-0 md:w-auto"
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
					{/* <div className="h-[100px] sm:h-[200px] w-full" /> */}
				</div>
			</PageWrapper>
		</TooltipProvider>
	);
}

interface Props {
	label: string;
	icon: any;
	iconDisabled?: true;
	clickable?: true;
}

const Platform = ({ icon: Icon, label, ...props }: ComponentProps<'a'> & Props) => {
	const Outer = props.href
		? (props: any) => <a aria-label={label} target="_blank" {...props} />
		: props.clickable
		? (props: any) => <button {...props} />
		: ({ children }: any) => <>{children}</>;

	return (
		<Tooltip label={label}>
			<Outer {...props}>
				<Icon
					size={25}
					className={`h-[25px] ${props.iconDisabled ? 'opacity-20' : 'opacity-80'}`}
					weight="fill"
				/>
			</Outer>
		</Tooltip>
	);
};
