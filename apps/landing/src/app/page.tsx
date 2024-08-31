import { ArrowUp } from '@phosphor-icons/react/dist/ssr';
import { Github } from '@sd/assets/svgs/brands';
import Image from 'next/image';
import CyclingImage from '~/components/CyclingImage';
import { toTitleCase } from '~/utils/util';

import { getLatestRelease, getReleaseFrontmatter, githubFetch } from './api/github';
import { Background } from './Background';
import { Banner } from './Banner';
import { Downloads } from './Downloads';

export const metadata = {
	title: 'Spacedrive — A file manager from the future.',
	description:
		'Combine your drives and clouds into one database that you can organize and explore from any device. Designed for creators, hoarders and the painfully disorganized.',

	keywords:
		'files,file manager,spacedrive,file explorer,vdfs,distributed filesystem,cas,content addressable storage,virtual filesystem,photos app, video organizer,video encoder,tags,tag based filesystem',
	authors: {
		name: 'Spacedrive Technology Inc.',
		url: 'https://spacedrive.com'
	}
};

export default async function Page() {
	const release = await githubFetch(getLatestRelease);
	const { frontmatter } = getReleaseFrontmatter(release);

	return (
		<>
			{/* <Background /> */}
			{/* <Image
				loading="eager"
				className="absolute-horizontal-center fade-in"
				width={1278}
				height={626}
				alt="l"
				src="/images/misc/header-gradient.webp"
			/> */}
			<div className="flex w-full flex-col items-center px-4">
				<div className="mt-22 lg:mt-28" id="content" aria-hidden="true" />
				<div className="mt-24 lg:mt-8" />
				{/* <NewBanner
					headline={`Alpha ${release.tag_name} is out!`}
					className="mt-[50px] lg:mt-0"
					href={`/docs/changelog/alpha/${release.tag_name}`}
				/> */}
				<Banner
					headline={`30k+ stars on GitHub`}
					className="mt-[50px] lg:mt-0"
					href={`/docs/changelog/alpha/${release.tag_name}`}
				/>
				<h1 className="fade-in-heading z-30 mb-3 bg-gradient-to-b from-[#EFF1FB] to-[#B8CEE0] bg-[length:100%_50%] bg-clip-text bg-repeat px-2 text-center text-4xl font-bold leading-[1.15] tracking-tight text-transparent md:text-5xl lg:text-6xl">
					Sync, manage, and discover.
					<br />
					Across all your devices.
				</h1>
				<p className="animation-delay-1 fade-in-heading text-md leading-2 z-30 mb-8 mt-1 max-w-4xl text-center text-gray-450 lg:text-lg lg:leading-8">
					Your files, always within reach. Experience seamless synchronization, intuitive
					management, and powerful discovery tools — all in one place.
				</p>
				<Downloads
					latestVersion={[
						frontmatter.category && toTitleCase(frontmatter.category),
						`v${release.tag_name}`
					]
						.filter(Boolean)
						.join(' ')}
				/>
				<div className="pb-6 xs:pb-24">
					<div className="xl2:relative z-30 flex h-[255px] w-full px-6 sm:h-[428px] md:mt-[75px] md:h-[428px] lg:h-auto">
						<Image
							loading="eager"
							className="absolute-horizontal-center animation-delay-2 top-[380px] fade-in xs:top-[180px] md:top-[130px]"
							width={1200}
							height={626}
							alt="l"
							src="/images/app/gradient.webp"
						/>
						<div className="relative m-auto mt-10 flex w-full max-w-7xl overflow-hidden rounded-lg transition-transform duration-700 ease-in-out md:mt-0">
							<div className="flex flex-col items-center justify-center">
								<div className="z-30 flex w-full rounded-lg border-t border-app-line/50 backdrop-blur">
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
										src="/images/app/gradient-overlay.png"
									/>
								</div>
								{/* <ArrowUp className="invisible size-7 pt-2 text-white/40 md:visible" />
								<p className="invisible pt-2 text-xs text-white/40 md:visible">
									Hover to see more
								</p> */}
							</div>
						</div>
					</div>
				</div>
				{/* <WormHole /> */}
				{/* <BentoBoxes /> */}
				{/* <CloudStorage /> */}
				{/* <DownloadToday isWindows={deviceOs?.isWindows} /> */}
				{/* <div className="h-[100px] sm:h-[200px] w-full" /> */}
			</div>
			{/* Explorer Section */}
			<div className="flex w-full flex-col items-center justify-center p-4">
				<div className="pb-6 xs:pb-24">
					<h1 className="text-xl">
						Explorer. Browse and manage your data like never before.
					</h1>
					<div className="flex flex-row gap-[20px]">
						{/* Bento Box 1 */}
						<div className="h-[440px] w-[400px] flex-shrink-0 rounded-[10px] border border-[#16171D] bg-[radial-gradient(66.79%_83.82%_at_0%_3.69%,#1B1D25_0%,#15161C_100%)] p-[29px]">
							<Image
								loading="eager"
								className="flex items-center justify-center fade-in"
								width={200}
								height={200}
								alt="l"
								src="/images/bento/library.webp"
							/>
							<div className="inline-flex items-center justify-center gap-2 pb-[10px]">
								<div className="h-[15px] w-[4px] rounded-[11px] bg-[#63C3F3]" />
								<h3 className="text-[20px]">Seamless Sync & Access</h3>
							</div>
							<div className="text-md inline-flex items-center justify-center gap-2 text-ink-faint">
								Whether online or offline, instantly access your data anytime,
								anywhere. Keeping everything updated and available across your
								devices.
							</div>
						</div>
						{/* Bento Box 2 */}
						<div className="h-[440px] w-[400px] flex-shrink-0 rounded-[10px] border border-[#16171D] bg-[radial-gradient(66.79%_83.82%_at_0%_3.69%,#1B1D25_0%,#15161C_100%)] p-[29px]">
							<Image
								loading="eager"
								className="flex items-center justify-center fade-in"
								width={200}
								height={200}
								alt="l"
								src="/images/bento/lock.webp"
							/>
							<div className="inline-flex items-center justify-center gap-2 pb-[10px]">
								<div className="h-[15px] w-[4px] rounded-[11px] bg-[#6368F3]" />
								<h3 className="text-[20px]">Privacy & Control</h3>
							</div>
							<div className="text-md inline-flex items-center justify-center gap-2 text-ink-faint">
								Your data is yours. With Spacedrive’s top-notch security, only you
								can access your information — no third parties, no exceptions.
							</div>
						</div>
						{/* Bento Box 3 */}
						<div className="h-[440px] w-[400px] flex-shrink-0 rounded-[10px] border border-[#16171D] bg-[radial-gradient(66.79%_83.82%_at_0%_3.69%,#1B1D25_0%,#15161C_100%)] p-[29px]">
							<Image
								loading="eager"
								className="flex items-center justify-center fade-in"
								width={200}
								height={200}
								alt="l"
								src="/images/bento/tags.webp"
							/>
							<div className="inline-flex items-center justify-center gap-2 pb-[10px]">
								<div className="h-[15px] w-[4px] rounded-[11px] bg-[#DF63F3]" />
								<h3 className="text-[20px]">Effortless Organization</h3>
							</div>
							<div className="text-md inline-flex items-center justify-center gap-2 text-ink-faint">
								Keep your digital life organized with automatic categorization and
								smart structuring, making it easy to find what you need instantly.
							</div>
						</div>
					</div>
				</div>
			</div>
			{/* 4 Corners Section */}
			<div className="flex w-full flex-col items-center justify-center p-4">
				<div className="pb-6 xs:pb-24">
					<h1 className="text-xl">TODO 4 Corners Section</h1>
				</div>
			</div>
			{/* Search Section */}
			<div className="flex w-full flex-col items-center justify-center p-4">
				<div className="pb-6 xs:pb-24">
					<h1 className="text-xl">
						Search. Find what you’re looking for with ease using advanced filters.
					</h1>
					<Image
						loading="eager"
						className="flex items-center justify-center fade-in"
						width={500}
						height={500}
						alt="l"
						src="/images/bento/search.webp"
					/>
				</div>
			</div>
			{/* Assistant Section */}
			<div className="flex w-full flex-col p-4">
				<div className="pb-6 xs:pb-24">
					<div className="inline-flex items-center justify-center gap-[10px] rounded-full border-[2px] border-[#FC79E7] bg-[rgba(43,43,59,0.50)] px-[10px] py-[11px]">
						<p className="text-center text-[14px] font-[500] leading-[125%]">
							COMING NEXT YEAR
						</p>
					</div>
					<h1 className="pt-[16px] text-xl">
						Assistant. Mighty-powerful AI — no cloud needed.
					</h1>
					<h2 className="pt-[16px] text-lg text-ink-faint">
						Details to be revealed soon...
					</h2>
				</div>
			</div>
			{/* Github Section */}
			<div className="flex w-full flex-col p-4">
				<div className="pb-6 xs:pb-24">
					<h1 className="pt-[16px] text-xl">Free and open-source. See for yourself.</h1>
					<h2 className="pb-[36px] pt-[16px] text-lg text-ink-faint">
						When we promise strong privacy and encryption, we mean it. Our app’s source
						code is entirely open-source and available on GitHub, so if you’re wondering
						what Spacedrive does with your data or have an improvement to share, you’re
						welcome to do so — we welcome and appreciate contributions!
					</h2>
					<a href="https://github.com/spacedriveapp/spacedrive" target="_blank">
						<button className="inline-flex items-center justify-center gap-[10px] rounded-xl bg-slate-800 px-[16px] py-[10px]">
							<Github />
							<span>View source on GitHub</span>
						</button>
					</a>
				</div>
			</div>
		</>
	);
}
