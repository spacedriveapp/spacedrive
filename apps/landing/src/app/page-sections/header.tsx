import Image from 'next/image';
import {
	getLatestRelease,
	getReleaseFrontmatter,
	getRepoStats,
	githubFetch
} from '~/app/api/github';
import { GoldenBadge } from '~/components/golden-badge';
import { HeroImage } from '~/components/hero-image'; // Import the client-side component
import { HomeCtaButtons } from '~/components/home-cta-buttons';
import Particles from '~/components/particles';
import { PlatformIcons } from '~/components/platform-icons';
import { toTitleCase } from '~/utils/misc';

import { Icon } from '../Icon';

export async function Header() {
	const [release, repoStats] = await Promise.all([
		githubFetch(getLatestRelease),
		githubFetch(getRepoStats)
	]);
	const { frontmatter } = getReleaseFrontmatter(release);
	const starCount = (repoStats.stargazers_count / 1000).toFixed(1);
	return (
		<div className="flex w-full flex-col items-center px-4">
			<div className="mt-22 lg:mt-28" id="content" aria-hidden="true" />
			<div className="mt-24 lg:mt-8" aria-hidden="true" />

			<GoldenBadge
				headline={`${starCount}k stars on GitHub`}
				className="mt-[50px] lg:mt-0"
				href={`https://github.com/spacedriveapp/spacedrive`}
			/>

			<h1 className="fade-in-heading z-30 mb-3 text-center text-3xl font-bold leading-[1.3] tracking-tight md:text-5xl lg:text-6xl">
				<span className="inline bg-gradient-to-b from-[#EFF1FB_15%] to-[#B8CEE0_85%] bg-clip-text text-transparent">
					Sync, manage, & discover.
					<br />
					Across all your devices.
				</span>
			</h1>

			<p className="animation-delay-1 fade-in-heading text-md leading-2 z-30 mb-8 mt-1 max-w-4xl text-balance text-center text-gray-450 lg:text-lg lg:leading-8">
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

			<PlatformIcons />

			<div>
				<div className="xl2:relative z-30 flex h-[255px] w-full px-6 sm:h-[428px] md:mt-12 md:h-[428px] lg:h-auto">
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
					{/* 1st light */}
					<Image
						loading="eager"
						className="absolute-horizontal-center animation-delay-2 top-[300px] -z-10 select-none fade-in xs:top-[180px] md:top-[130px]"
						width={1200}
						height={626}
						alt="l"
						src="/images/app/gradient.webp"
					/>
					{/* 2nd light */}
					<div className="animation-delay-2 absolute-horizontal-center top-[450px] size-[150px] rounded-full bg-gradient-to-t from-transparent to-[#328FDD]/40 blur-[20px] fade-in xs:top-[180px] md:top-[500px] md:h-[500px] md:w-[240px] md:blur-2xl" />
					<div className="relative">
						<Icon
							name="FolderNoSpace"
							size={200}
							className="absolute -left-32 top-[400px] -z-[1] rotate-[-30deg] fade-in"
						/>
						<HeroImage
							src="/images/app/AppHero.png"
							alt="Spacedrive App Image"
							width={1200}
							height={800}
						/>
					</div>
				</div>
			</div>
		</div>
	);
}
