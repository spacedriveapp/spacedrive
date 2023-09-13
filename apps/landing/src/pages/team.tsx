import Head from 'next/head';
import Link from 'next/link';
import { ArrowRight } from '@phosphor-icons/react';

import Markdown from '~/components/Markdown';
import PageWrapper from '~/components/PageWrapper';
import { TeamMember, TeamMemberProps } from '~/components/TeamMember';

export const teamMembers: Array<TeamMemberProps> = [
	{
		name: 'Jamie Pine',
		role: 'Founder, Engineer & Designer',
		imageUrl: '/images/team/jamie.jpg',
		socials: {
			twitter: 'https://x.com/jamiepine',
			twitch: 'https://twitch.tv/jamiepinelive',
			github: 'https://github.com/jamiepine'
		}
	},
	{
		name: 'Brendan Allan',
		role: 'Rust Engineer',
		imageUrl: '/images/team/brendan.jpg',
		socials: {
			twitter: 'https://x.com/brendonovichdev',
			twitch: 'https://twitch.tv/brendonovich',
			github: 'https://github.com/brendonovich'
		}
	},
	{
		name: 'Oscar Beaumont',
		role: 'Rust Engineer',
		imageUrl: '/images/team/oscar.jpg',
		socials: {
			twitter: 'https://x.com/oscartbeaumont',
			twitch: 'https://twitch.tv/oscartbeaumont',
			github: 'https://github.com/oscartbeaumont'
		}
	},
	{
		name: 'Ericson Soares',
		role: 'Rust Engineer',
		imageUrl: '/images/team/ericson.jpg',
		socials: {
			twitter: 'https://x.com/fogodev',
			github: 'https://github.com/fogodev'
		}
	},
	{
		name: 'Utku Bakır',
		role: 'React Native Engineer',
		imageUrl: '/images/team/utku.jpg',
		socials: {
			github: 'https://github.com/utkubakir'
		}
	},
	{
		name: 'Jake Robinson',
		role: 'Rust Engineer',
		imageUrl: '/images/team/jake.jpg',
		socials: {
			github: 'https://github.com/brxken128'
		}
	},
	{
		name: 'Mihail Dounaev',
		role: 'Graphic Designer',
		imageUrl: '/images/team/mihail.jpg',
		socials: {
			twitter: 'https://x.com/mmmintdesign',
			dribbble: 'https://dribbble.com/mmmint'
		}
	},
	{
		name: 'Ameer Al Ashhab',
		role: 'React Engineer & Designer',
		imageUrl: '/images/team/ameer.jpg',
		socials: {
			github: 'https://github.com/ameer2468'
		}
	},
	{
		name: 'Vítor Vasconcellos',
		role: 'React Engineer & Designer',
		imageUrl: '/images/team/vitor.jpg',
		socials: {
			github: 'https://github.com/HeavenVolkoff'
		}
	},
	{
		name: 'Nik Elšnik',
		role: 'React Engineer & Designer',
		imageUrl: '/images/team/nikec.jpg',
		socials: {
			github: 'https://github.com/niikeec',
			twitter: 'https://x.com/nikec_'
		}
	}
];

const investors: Array<TeamMemberProps> = [
	{
		name: 'Joseph Jacks',
		role: 'Founder, OSSC',
		investmentRound: 'Lead Seed',
		imageUrl: '/images/investors/josephjacks.jpg'
	},
	{
		name: 'Guillermo Rauch',
		role: 'CEO, Vercel',
		investmentRound: 'Co-Lead Seed',
		imageUrl: '/images/investors/guillermo.jpg'
	},
	{
		name: 'Naval Ravikant',
		role: 'Founder, AngelList',
		investmentRound: 'Co-Lead Seed',
		imageUrl: '/images/investors/naval.jpg'
	},
	{
		name: 'Neha Narkhede',
		role: 'Confluent, Apache Kafka',
		investmentRound: 'Seed',
		imageUrl: '/images/investors/neha.jpg'
	},
	{
		name: 'Austen Allred',
		role: 'CEO, Bloom Institute of Technology',
		investmentRound: 'Seed',
		imageUrl: '/images/investors/austen.jpg'
	},
	{
		name: 'Tom Preston-Werner',
		role: 'Founder, GitHub',
		investmentRound: 'Seed',
		imageUrl: '/images/investors/TOM.jpg'
	},
	{
		name: 'Tobias Lütke',
		role: 'CEO, Shopify',
		investmentRound: 'Seed',
		imageUrl: '/images/investors/tobiaslutke.jpg'
	},
	{
		name: 'Justin Hoffman',
		role: 'Former VP Sales, Elasticsearch',
		investmentRound: 'Seed',
		imageUrl: '/images/investors/justinhoffman.jpg'
	},
	{
		name: 'Ry Walker',
		role: 'Founder, Astronomer',
		investmentRound: 'Seed',
		imageUrl: '/images/investors/rywalker.jpg'
	},
	{
		name: 'Zachary Smith',
		role: 'Head of Edge Infrastructure, Equinix',
		investmentRound: 'Seed',
		imageUrl: '/images/investors/zacharysmith.jpg'
	},
	{
		name: 'Sanjay Poonen',
		role: 'Former COO, VMware',
		investmentRound: 'Seed',
		imageUrl: '/images/investors/sanjay.jpg'
	},
	{
		name: 'David Mytton',
		role: 'CEO, console.dev',
		investmentRound: 'Seed',
		imageUrl: '/images/investors/davidmytton.jpg'
	},
	{
		name: 'Peer Richelsen',
		role: 'CEO, Cal.com',
		investmentRound: 'Seed',
		imageUrl: '/images/investors/peer.jpg'
	},
	{
		name: 'Lester Lee',
		role: 'Founder, Slapdash',
		investmentRound: 'Seed',
		imageUrl: '/images/investors/lesterlee.jpg'
	},
	{
		name: 'Haoyuan Li',
		role: 'Founder, Alluxio',
		investmentRound: 'Seed',
		imageUrl: '/images/investors/haoyuan.jpg'
	},
	{
		name: 'Augusto Marietti',
		role: 'CEO, Kong',
		investmentRound: 'Seed',
		imageUrl: '/images/investors/augusto.jpg'
	},
	{
		name: 'Vijay Sharma',
		role: 'CEO, Belong',
		investmentRound: 'Seed',
		imageUrl: '/images/investors/vijay.jpg'
	},
	{
		name: 'Naveen R',
		role: 'Founder, NocoDB',
		investmentRound: 'Seed',
		imageUrl: '/images/investors/naveen.jpg'
	}
];

export default function TeamPage() {
	return (
		<PageWrapper>
			<Markdown articleClassNames="mx-auto mt-32 prose-a:text-white">
				<Head>
					<title>Our Team - Spacedrive</title>
					<meta name="description" content="Who's behind Spacedrive?" />
				</Head>
				<div className="team-page relative mx-auto">
					<div
						className="bloom subtle egg-bloom-one -top-60 right-[-400px]"
						style={{ transform: 'scale(2)' }}
					/>
					<div className="relative z-10">
						<h1 className="fade-in-heading text-5xl leading-tight sm:leading-snug ">
							We believe file management should be{' '}
							<span className="title-gradient">universal</span>.
						</h1>
						<p className="animation-delay-2 fade-in-heading text-white/50 ">
							Your priceless personal data shouldn't be stuck in a device ecosystem.
							It should be OS agnostic, permanent and owned by you.
						</p>
						<p className="animation-delay-2 fade-in-heading text-white/50 ">
							The data we create daily is our legacy—that will long outlive us. Open
							source technology is the only way to ensure we retain absolute control
							over the files that define our lives, at unlimited scale.
						</p>
						<Link
							href="/docs/product/resources/faq"
							className="animation-delay-3 fade-in-heading text-underline flex flex-row items-center text-gray-400 underline-offset-4 duration-150 hover:text-white"
						>
							<ArrowRight className="mr-2" />
							Read more
						</Link>
						<div className="fade-in-heading animation-delay-5">
							<h2 className="mt-10 text-2xl leading-relaxed sm:mt-20 ">
								Meet the team
							</h2>
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
								We're backed by some of the greatest leaders in the technology
								industry.
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
		</PageWrapper>
	);
}
