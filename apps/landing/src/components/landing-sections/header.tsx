import Image from 'next/image';
import React from 'react';
import { getLatestRelease, getReleaseFrontmatter, githubFetch } from '~/app/api/github';
import Particles from '~/particles';
import { toTitleCase } from '~/utils/misc';

import { GoldenBadge } from '../golden-badge';
import { HomeCtaButtons } from '../home-cta-buttons';

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

export async function Header() {
	const release = await githubFetch(getLatestRelease);
	const { frontmatter } = getReleaseFrontmatter(release);
	return (
		<div className="flex w-full flex-col items-center px-4">
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

			<p className="animation-delay-1 fade-in-heading text-md leading-2 z-30 mb-8 mt-1 max-w-4xl text-center text-gray-450 lg:text-lg lg:leading-8">
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

			<div>
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
							<div className="z-30 flex w-full justify-center backdrop-blur">
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
		</div>
	);
}
