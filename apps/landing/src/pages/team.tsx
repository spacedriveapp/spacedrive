import React from 'react';
import { Helmet } from 'react-helmet';

import { ReactComponent as ArrowRight } from '../../../../packages/interface/src/assets/svg/arrow-right.svg';
import Markdown from '../components/Markdown';
import { TeamMember, TeamMemberProps } from '../components/TeamMember';

const teamMembers: Array<TeamMemberProps> = [
	{
		name: 'Jamie Pine',
		role: 'Founder, Engineer & Designer',
		image: 'team/jamie.jpg',
		socials: {
			twitter: 'https://twitter.com/jamiepine',
			twitch: 'https://twitch.tv/jamiepinelive',
			github: 'https://github.com/jamiepine'
		}
	},
	{
		name: 'Brendan Allan',
		role: 'Rust Backend Engineer',
		image: 'team/brendan.jpg',
		socials: {
			twitter: 'https://twitter.com/brendonovichdev',
			twitch: 'https://twitch.tv/brendonovich',
			github: 'https://github.com/brendonovich'
		}
	},
	{
		name: 'Oscar Beaumont',
		role: 'Rust Backend Engineer',
		image: 'team/oscar.jpg',
		socials: {
			twitter: 'https://twitter.com/oscartbeaumont',
			twitch: 'https://twitch.tv/oscartbeaumont',
			github: 'https://github.com/oscartbeaumont'
		}
	},
	{
		name: 'Haden Fletcher',
		role: 'Engineer & Designer',
		image: 'team/haden.jpg',
		socials: {
			twitter: 'https://twitter.com/heymaxichrome',
			twitch: 'https://twitch.tv/maxichrome',
			github: 'https://github.com/maxichrome'
		}
	},
	{
		name: 'Benjamin Akar',
		role: 'Engineer & Designer',
		image: 'team/benja.jpg',
		socials: {
			twitter: 'https://twitter.com/benjaminakar',
			twitch: 'https://twitch.tv/akawr',
			github: 'https://github.com/benja'
		}
	},
	{
		name: 'Haris Mehrzad',
		role: 'Engineer Intern',
		image: 'team/haris.jpg',
		socials: {
			twitter: 'https://twitter.com/xPolarrr',
			github: 'https://github.com/xPolar'
		}
	}
];

const investors: Array<TeamMemberProps> = [
	{
		name: 'Joseph Jacks',
		role: 'Founder, OSSC',
		investmentRound: 'Lead Seed',
		image: 'investors/josephjacks.jpg'
	},
	{
		name: 'Guillermo Rauch',
		role: 'CEO, Vercel',
		investmentRound: 'Co-Lead Seed',
		image: 'investors/guillermo.jpg'
	},
	{
		name: 'Naval Ravikant',
		role: 'Founder, AngelList',
		investmentRound: 'Co-Lead Seed',
		image: 'investors/naval.jpg'
	},
	{
		name: 'Neha Narkhede',
		role: 'Confluent, Apache Kafka',
		investmentRound: 'Seed',
		image: 'investors/neha.jpg'
	},
	{
		name: 'Austen Allred',
		role: 'CEO, Bloom Institute of Technology',
		investmentRound: 'Seed',
		image: 'investors/austen.jpg'
	},
	{
		name: 'Tom Preston-Werner',
		role: 'Founder, GitHub',
		investmentRound: 'Seed',
		image: 'investors/TOM.jpg'
	},
	{
		name: 'Tobias Lütke',
		role: 'CEO, Shopify',
		investmentRound: 'Seed',
		image: 'investors/tobiaslutke.jpg'
	},
	{
		name: 'Justin Hoffman',
		role: 'Former VP Sales, Elasticsearch',
		investmentRound: 'Seed',
		image: 'investors/justinhoffman.jpg'
	},
	{
		name: 'Ry Walker',
		role: 'Founder, Astronomer',
		investmentRound: 'Seed',
		image: 'investors/rywalker.jpg'
	},
	{
		name: 'Zachary Smith',
		role: 'Head of Edge Infrastructure, Equinix',
		investmentRound: 'Seed',
		image: 'investors/zacharysmith.jpg'
	},
	{
		name: 'Sanjay Poonen',
		role: 'Former COO, VMware',
		investmentRound: 'Seed',
		image: 'investors/sanjay.jpg'
	},
	{
		name: 'David Mytton',
		role: 'CEO, console.dev',
		investmentRound: 'Seed',
		image: 'investors/davidmytton.jpg'
	},
	{
		name: 'Peer Richelsen',
		role: 'CEO, Cal.com',
		investmentRound: 'Seed',
		image: 'investors/peer.jpg'
	},
	{
		name: 'Lester Lee',
		role: 'Founder, Slapdash',
		investmentRound: 'Seed',
		image: 'investors/lesterlee.jpg'
	},
	{
		name: 'Haoyuan Li',
		role: 'Founder, Alluxio',
		investmentRound: 'Seed',
		image: 'investors/haoyuan.jpg'
	},
	{
		name: 'Augusto Marietti',
		role: 'CEO, Kong',
		investmentRound: 'Seed',
		image: 'investors/augusto.jpg'
	},
	{
		name: 'Vijay Sharma',
		role: 'CEO, Belong',
		investmentRound: 'Seed',
		image: 'investors/sharma.jpg'
	},
	{
		name: 'Naveen R',
		role: 'NocoDB',
		investmentRound: 'Seed',
		image: 'investors/naveen.jpg'
	}
];

function Page() {
	return (
		<Markdown>
			<Helmet>
				<title>Our Team - Spacedrive</title>
				<meta name="description" content="Who's behind Spacedrive?" />
			</Helmet>
			<div className="relative team-page">
				<div
					className="absolute -top-60 -right-[400px] opacity-60 w-[1000px] h-[800px] fade-in"
					style={{
						background:
							'linear-gradient(180deg, rgba(180, 180, 180, 0.76) 0%, rgba(19, 4, 168, 0.41) 95.73%)',
						filter: 'blur(300px)',
						transform: 'rotate(56.81deg)'
					}}
				></div>
				<div className="relative z-10">
					<h1 className="text-5xl leading-snug fade-in-heading ">
						We believe file management should be <span className="title-gradient">universal</span>.
					</h1>
					<p className="text-gray-400 animation-delay-2 fade-in-heading ">
						Your priceless personal data shouldn't be stuck in a device ecosystem. It should be OS
						agnostic, permanent and owned by you.
					</p>
					<p className="text-gray-400 animation-delay-2 fade-in-heading ">
						The data we create daily is our legacy—that will long outlive us. Open source technology
						is the only way to ensure we retain absolute control over the files that define our
						lives, at unlimited scale.
					</p>
					<a
						href="/faq"
						className="flex flex-row items-center text-gray-400 duration-150 animation-delay-3 fade-in-heading hover:text-white text-underline underline-offset-4"
					>
						<ArrowRight className="mr-2" />
						Read more
					</a>
					<div className="fade-in-heading animation-delay-5">
						<h2 className="mt-10 text-2xl leading-relaxed sm:mt-20 ">Meet the team</h2>
						<div className="grid grid-cols-2 my-10 xs:grid-cols-3 sm:grid-cols-4 gap-x-5 gap-y-10">
							{teamMembers.map((member) => (
								<TeamMember key={member.name} {...member} />
							))}
						</div>
						<p className="text-sm text-gray-400">
							... and all the awesome{' '}
							<a
								href="https://github.com/spacedriveapp/spacedrive/graphs/contributors"
								target="_blank"
								rel="noreferer"
								className="duration-200 oss-credit-gradient hover:opacity-75"
							>
								open source contributors
							</a>{' '}
							on GitHub.
						</p>
						<h2 className="mt-10 mb-2 text-2xl leading-relaxed sm:mt-20 ">Our investors</h2>
						<p className="text-sm text-gray-400 ">
							We're backed by some of the greatest leaders in the technology industry.
						</p>
						<div className="grid grid-cols-3 my-10 sm:grid-cols-5 gap-x-5 gap-y-10">
							{investors.map((investor) => (
								<TeamMember key={investor.name + investor.investmentRound} {...investor} />
							))}
						</div>
					</div>
				</div>
			</div>
		</Markdown>
	);
}

export default Page;
