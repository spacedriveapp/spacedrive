import { ArrowUp } from '@phosphor-icons/react/dist/ssr';
import Image from 'next/image';
import CyclingImage from '~/components/CyclingImage';
import { toTitleCase } from '~/utils/util';

import { getLatestRelease, getReleaseFrontmatter, githubFetch } from './api/github';
import { Background } from './Background';
import { Downloads } from './Downloads';
import { NewBanner } from './NewBanner';

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
			<Background />
			<Image
				loading="eager"
				className="absolute-horizontal-center fade-in"
				width={1278}
				height={626}
				alt="l"
				src="/images/misc/header-gradient.webp"
			/>
			<div className="flex w-full flex-col items-center px-4">
				<div className="mt-22 lg:mt-28" id="content" aria-hidden="true" />
				<div className="mt-24 lg:mt-8" />
				<NewBanner headline="Alpha 0.3 is here!" className="mt-[50px] lg:mt-0" />
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
				<Downloads
					latestVersion={[
						frontmatter.category && toTitleCase(frontmatter.category),
						`v${release.tag_name}`
					]
						.filter(Boolean)
						.join(' ')}
				/>
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
							src="/images/app/gradient.webp"
						/>
						<div className="relative m-auto mt-10 flex w-full max-w-7xl overflow-hidden rounded-lg transition-transform duration-700 ease-in-out hover:-translate-y-4 hover:scale-[1.02] md:mt-0">
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
								<ArrowUp className="invisible size-7 pt-2 text-white/40 md:visible" />
								<p className="invisible pt-2 text-xs text-white/40 md:visible">
									Hover to see more
								</p>
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
		</>
	);
}
