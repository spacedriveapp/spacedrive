import { Github } from '@sd/assets/svgs/brands';
import Image from 'next/image';
import { BentoBox } from '~/components/bento-box';
import { CTAButtons } from '~/components/cta-buttons';
import CyclingImage from '~/components/cycling-image';
import { GoldenBadge } from '~/components/golden-badge';
import { toTitleCase } from '~/utils/misc';

import { getLatestRelease, getReleaseFrontmatter, githubFetch } from './api/github';

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
				<GoldenBadge
					headline={`30k+ stars on GitHub`}
					className="mt-[50px] lg:mt-0"
					href={`https://github.com/spacedriveapp/spacedrive`}
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
				<CTAButtons
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
						<div className="relative m-auto mt-10 flex w-full max-w-7xl overflow-hidden rounded-[10px] transition-transform duration-700 ease-in-out md:mt-0">
							<div className="flex flex-col items-center justify-center">
								<div className="z-30 flex w-full rounded-[10px] border-t border-app-line/50 backdrop-blur">
									<div className="relative h-[635px] w-[1402px]">
										<div className="absolute left-0 top-0 z-[110] h-[635px] w-[1402px] rounded-[10px] bg-gradient-to-b from-transparent to-[#0E0E12]" />
										<Image
											loading="eager"
											width={1400}
											height={800}
											alt="Spacedrive App Image"
											className="absolute left-0 h-[800px] w-[1400px] rounded-[10px]"
											src="/images/app/wip/MultiDeviceOverview.png"
										/>
									</div>
								</div>
							</div>
						</div>
					</div>
				</div>
				{/* <WormHole /> */}
				{/* <BentoBoxes /> */}
				{/* <DownloadToday isWindows={deviceOs?.isWindows} /> */}
				{/* <div className="h-[100px] sm:h-[200px] w-full" /> */}
			</div>
			{/* Explorer Section */}
			<div className="flex w-full flex-col items-center justify-center p-4">
				<div className="pb-6 xs:pb-24">
					<h1 className="ml-6 text-3xl font-semibold">
						Explorer.{' '}
						<span className="bg-gradient-to-r from-[#A3A3AE] to-[#434348] bg-clip-text text-transparent">
							Browse and manage <br />
							your data like never before.
						</span>
					</h1>

					<div className="mt-5 flex flex-row gap-[20px]">
						<BentoBox
							imageSrc="/images/bento/library.webp"
							imageAlt="Library"
							title="Seamless Sync & Access"
							titleColor="#63C3F3"
							description="Whether online or offline, instantly access your data anytime, anywhere. Keeping everything updated and available across your devices."
						/>
						<BentoBox
							imageSrc="/images/bento/lock.webp"
							imageAlt="Lock"
							title="Privacy & Control"
							titleColor="#6368F3"
							description="Your data is yours. With Spacedrive’s top-notch security, only you can access your information — no third parties, no exceptions."
						/>
						<BentoBox
							imageSrc="/images/bento/tags.webp"
							imageAlt="Tags"
							title="Effortless Organization"
							titleColor="#DF63F3"
							description="Keep your digital life organized with automatic categorization and smart structuring, making it easy to find what you need instantly."
						/>
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
					<div className="inline-flex items-center justify-center gap-[10px] rounded-full border-2 border-[#FC79E7] bg-[rgba(43,43,59,0.50)] px-[10px] py-[11px]">
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
