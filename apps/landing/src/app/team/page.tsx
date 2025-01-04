import { ArrowRight } from '@phosphor-icons/react/dist/ssr';
import Link from 'next/link';
import { CtaSecondaryButton } from '~/components/cta-secondary-button';

import { investors, teamMembers } from './people';
import { TeamMember } from './TeamMember';

export const metadata = {
	title: 'Our Team - Spacedrive',
	description: "Who's behind Spacedrive?"
};

export default function Page() {
	return (
		<div className="mt-[200px] flex w-full flex-col items-center overflow-x-hidden px-4 md:overflow-visible">
			<div className="relative mx-auto h-auto w-full max-w-[1200px]">
				<div
					className="absolute -left-1/4 top-[10%] mx-auto size-[750px] blur-[5px] fade-in xs:inset-x-0 xs:left-[-16%] xs:top-[-50px] sm:left-0 sm:top-[-175px] sm:mx-auto md:inset-x-0 md:top-[-250px] md:size-[900px] lg:top-[-420px] lg:size-[1100px]"
					style={{
						backgroundImage: 'url(/images/circlebg.webp',
						backgroundRepeat: 'no-repeat',
						backgroundSize: 'contain',
						backgroundPosition: 'center top'
					}}
				/>
				<h1 className="fade-in-heading mx-auto w-full max-w-[750px] text-center text-5xl font-bold leading-tight sm:leading-snug">
					We believe file management should be{' '}
					<span className="bg-gradient-to-r from-fuchsia-500 to-indigo-500 bg-clip-text text-transparent">
						universal
					</span>
					.
				</h1>
				<div className="mt-10 space-y-4">
					<p className="animation-delay-2 fade-in-heading mx-auto max-w-[750px] text-center text-ink-faint">
						Your priceless personal data shouldn't be stuck in a device ecosystem. It
						should be OS agnostic, permanent and owned by you.
					</p>
					<p className="animation-delay-2 fade-in-heading mx-auto max-w-[600px] text-center text-ink-faint">
						The data we create daily is our legacy—that will long outlive us. Open
						source technology is the only way to ensure we retain absolute control over
						the files that define our lives, at unlimited scale.
					</p>
				</div>
				<div className="mt-12 flex w-full justify-center">
					<CtaSecondaryButton
						className="2 animation-delay-3 fade-in-heading min-w-fit px-5 duration-150"
						icon={<ArrowRight weight="bold" />}
						href="/docs/product/resources/faq"
					>
						Read more
					</CtaSecondaryButton>
				</div>
			</div>
			<div className="fade-in-heading animation-delay-5">
				<div className="mx-auto mt-40 w-full max-w-[700px]">
					<h2 className="text-2xl font-bold leading-relaxed">Meet the team</h2>
					<div className="my-10 grid grid-cols-2 gap-x-0 gap-y-10 xs:grid-cols-3 sm:grid-cols-4">
						{teamMembers.map((member) => (
							<TeamMember key={member.name} {...member} />
						))}
					</div>
					<p className="text-sm text-gray-400">
						... and all the awesome{' '}
						<Link
							href="https://github.com/spacedriveapp/spacedrive/graphs/contributors"
							target="_blank"
							rel="noreferrer"
							className="oss-credit-gradient duration-200 hover:opacity-75"
						>
							open source contributors
						</Link>{' '}
						on GitHub.
					</p>
				</div>
				<h2
					id="investors"
					className="mb-2 mt-10 text-2xl font-bold leading-relaxed sm:mt-20"
				>
					Our investors
				</h2>
				<p className="text-sm text-gray-400">
					We're backed by some of the greatest leaders in the technology industry.
				</p>
				<div className="my-10 grid w-full max-w-[700px] grid-cols-3 gap-x-5 gap-y-10 sm:grid-cols-5">
					{investors.map((investor) => (
						<TeamMember key={investor.name + investor.investmentRound} {...investor} />
					))}
				</div>
			</div>
		</div>
	);
}
