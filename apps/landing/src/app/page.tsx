import { Github } from '@sd/assets/svgs/brands';
import Image from 'next/image';
import { BentoBox } from '~/components/bento-box';
import { GoldenBadge } from '~/components/golden-badge';
import { HomeCtaButtons } from '~/components/home-cta-buttons';
import Particles from '~/particles';
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
			<div className="flex flex-col items-center w-full px-4">
				<div className="mt-22 lg:mt-28" id="content" aria-hidden="true" />
				<div className="mt-24 lg:mt-8" aria-hidden="true" />

				<GoldenBadge
					headline={`30k+ stars on GitHub`}
					className="mt-[50px] lg:mt-0"
					href={`https://github.com/spacedriveapp/spacedrive`}
				/>

				<h1 className="fade-in-heading z-30 mb-3 text-center text-3xl font-bold leading-[1.3] tracking-tight md:text-5xl lg:text-6xl">
					<span className="inline bg-gradient-to-b from-[#EFF1FB] from-15% to-[#B8CEE0] to-85% bg-clip-text text-transparent">
						{`Sync, manage, & discover.`}
						<br />
						{`Across all your devices.`}
					</span>
				</h1>

				<p className="z-30 max-w-4xl mt-1 mb-8 text-center animation-delay-1 fade-in-heading text-md leading-2 text-gray-450 lg:text-lg lg:leading-8">
					Your files, always within reach. Experience seamless synchronization, intuitive
					management, and powerful discovery tools — all in one place.
				</p>

				<HomeCtaButtons
					latestVersion={[
						frontmatter.category && toTitleCase(frontmatter.category),
						`v${release.tag_name}`
					]
						.filter(Boolean)
						.join(' ')}
				/>

				<div className="pb-6 xs:pb-24">
					<div className="xl2:relative z-30 flex h-[255px] w-full px-6 sm:h-[428px] md:mt-12 md:h-[428px] lg:h-auto">
						<Image
							loading="eager"
							className="absolute-horizontal-center animation-delay-2 top-[380px] -z-10 select-none fade-in xs:top-[180px] md:top-[130px]"
							width={1200}
							height={626}
							alt="l"
							src="/images/app/gradient.webp"
						/>
						<div className="absolute inset-x-0 top-[450px] mx-auto flex size-[200px] md:size-[500px]">
							<Particles
								quantity={80}
								ease={80}
								staticity={100}
								color={'#58B3FF'}
								refresh
								vy={-0.2}
								vx={-0.05}
							/>
						</div>
						<div className="relative m-auto mt-10 flex w-full max-w-7xl overflow-hidden rounded-[10px] transition-transform duration-700 ease-in-out md:mt-0">
							<div className="flex flex-col items-center justify-center">
								<div className="z-30 flex justify-center w-full backdrop-blur">
									<div className="relative h-auto w-full max-w-[1200px]">
										<div className="h-px w-full bg-gradient-to-r from-transparent via-[#008BFF]/40 to-transparent" />
										<div className="absolute inset-x-0 top-0 z-[110] size-full bg-gradient-to-b from-transparent to-[#0E0E12]" />
										<Image
											loading="eager"
											layout="responsive"
											width={1200}
											height={800}
											alt="Spacedrive App Image"
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
			<div className="mx-auto flex w-full max-w-[1200px] flex-col flex-wrap items-center gap-10 p-4">
				<h1 className="self-start flex-1 text-2xl font-semibold leading-8 md:text-3xl md:leading-10">
					Explorer.{' '}
					<span className="text-transparent bg-gradient-to-r from-zinc-400 to-zinc-600 bg-clip-text">
						Browse and manage <br />
						your data like never before.
					</span>
				</h1>
				<div className="flex flex-col gap-5 md:flex-row">
					<BentoBox
						className="bento-card-border-left"
						imageSrc="/images/new/bento_1.svg"
						imageAlt="Library"
						title="Seamless Sync & Access"
						titleColor="#63C3F3"
						description="Whether online or offline, instantly access your data anytime, anywhere. Keeping everything updated and available across your devices."
					/>
					<BentoBox
						imageSrc="/images/new/bento_2.svg"
						imageAlt="Lock"
						title="Privacy & Control"
						titleColor="#6368F3"
						description="Your data is yours. With Spacedrive’s top-notch security, only you can access your information — no third parties, no exceptions."
					/>
					<BentoBox
						className="bento-card-border-right"
						imageSrc="/images/new/bento_3.svg"
						imageAlt="Tags"
						title="Effortless Organization"
						titleColor="#DF63F3"
						description="Keep your digital life organized with automatic categorization and smart structuring, making it easy to find what you need instantly."
					/>
				</div>
			</div>
			{/* 4 Corners Section */}
			<div className="flex flex-col items-center justify-center w-full p-4">
				<div className="pb-6 xs:pb-24">
					<h1 className="text-xl">TODO 4 Corners Section</h1>
				</div>
			</div>
			{/* Search Section */}
			<div className="flex flex-col items-center justify-center w-full p-4">
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
			<div className="flex flex-col w-full p-4">
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
			<div className="flex flex-col w-full p-4">
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
