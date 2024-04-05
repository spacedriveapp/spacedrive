import { ArrowRight } from '@phosphor-icons/react/dist/ssr';
import Link from 'next/link';
import Markdown from '~/components/Markdown';

import { investors, teamMembers } from './people';
import { TeamMember } from './TeamMember';

export const metadata = {
	title: 'Our Team - Spacedrive',
	description: "Who's behind Spacedrive?"
};

export default function Page() {
	return (
		<Markdown articleClassNames="mx-auto mt-32 prose-a:text-white">
			<div className="team-page relative mx-auto">
				<div className="relative z-10">
					<h1 className="fade-in-heading text-5xl leading-tight sm:leading-snug ">
						We believe file management should be{' '}
						<span className="title-gradient">universal</span>.
					</h1>
					<p className="animation-delay-2 fade-in-heading text-white/50 ">
						Your priceless personal data shouldn't be stuck in a device ecosystem. It
						should be OS agnostic, permanent and owned by you.
					</p>
					<p className="animation-delay-2 fade-in-heading text-white/50 ">
						The data we create daily is our legacy—that will long outlive us. Open
						source technology is the only way to ensure we retain absolute control over
						the files that define our lives, at unlimited scale.
					</p>
					<Link
						href="/docs/product/resources/faq"
						className="animation-delay-3 fade-in-heading text-underline flex flex-row items-center text-gray-400 underline-offset-4 duration-150 hover:text-white"
					>
						<ArrowRight className="mr-2" width="1rem" height="1rem" />
						Read more
					</Link>
					<div className="fade-in-heading animation-delay-5">
						<h2 className="mt-10 text-2xl leading-relaxed sm:mt-20 ">Meet the team</h2>
						<div className="my-10 grid grid-cols-2 gap-x-5 gap-y-10 xs:grid-cols-3 sm:grid-cols-4">
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
						<h2
							id="investors"
							className="mb-2 mt-10 text-2xl leading-relaxed sm:mt-20 "
						>
							Our investors
						</h2>
						<p className="text-sm text-gray-400 ">
							We're backed by some of the greatest leaders in the technology industry.
						</p>
						<div className="my-10 grid grid-cols-3 gap-x-5 gap-y-10 sm:grid-cols-5">
							{investors.map((investor) => (
								<TeamMember
									key={investor.name + investor.investmentRound}
									{...investor}
								/>
							))}
						</div>
					</div>
				</div>
			</div>
		</Markdown>
	);
}
