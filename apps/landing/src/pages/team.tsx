import React from 'react';
import { Helmet } from 'react-helmet';

import { ReactComponent as ArrowRight } from '../../../../packages/interface/src/assets/svg/arrow-right.svg';
import Markdown from '../components/Markdown';
import { TeamMember } from '../components/TeamMember';

function Page() {
	const teamMembers = [
		{
			name: 'Jamie Pine',
			role: 'Founder, Developer & Designer',
			image: '/team/jamiepine.jpeg',
			socials: {
				twitter: 'https://twitter.com/jamiepine',
				twitch: 'https://twitch.tv/jamiepinelive',
				github: 'https://github.com/jamiepine'
			}
		},
		{
			name: 'Brendan Allan',
			role: 'Backend Developer',
			image: '/team/brendanallan.jpeg',
			socials: {
				twitter: 'https://twitter.com/brendonovichdev',
				twitch: 'https://twitch.tv/brendonovich',
				github: 'https://github.com/brendonovich'
			}
		},
		{
			name: 'Oscar Beaumont',
			role: 'Backend Developer',
			image: '/team/oscarbeaumont.jpeg',
			socials: {
				twitter: 'https://twitter.com/oscartbeaumont',
				twitch: 'https://twitch.tv/oscartbeaumont',
				github: 'https://github.com/oscartbeaumont'
			}
		},
		{
			name: 'Haden Fletcher',
			role: 'Developer & Designer',
			image: '/team/hadenfletcher.jpeg',
			socials: {
				twitter: 'https://twitter.com/heymaxichrome',
				twitch: 'https://twitch.tv/maxichrome',
				github: 'https://github.com/maxichrome'
			}
		},
		{
			name: 'Benjamin Akar',
			role: 'Developer & Designer',
			image: '/team/benjaminakar.jpg',
			socials: {
				twitter: 'https://twitter.com/benjaminakar',
				twitch: 'https://twitch.tv/akawr',
				github: 'https://github.com/benja'
			}
		}
	];

	const investors = [
		{
			name: 'Joseph Jacks',
			role: 'Founder, OSSC',
			joined: 'Seed',
			image: '/investors/josephjacks.jpeg'
		},
		{
			name: 'Guillermo Rauch',
			role: 'CEO Vercel',
			joined: 'Seed',
			image: '/investors/rauchg.jpeg'
		},
		{
			name: 'Naval Ravikant',
			role: 'Founder, AngelList',
			joined: 'Seed',
			image: '/investors/naval.jpeg'
		},
		{
			name: 'Neha Narkhede',
			role: 'Confluent, Apache Kafka',
			joined: 'Seed',
			image: '/investors/neha.jpeg'
		},
		{
			name: 'Austen Allred',
			role: 'CEO Bloom Institute of Technology',
			joined: 'Seed',
			image: '/investors/austen.jpeg'
		},
		{
			name: 'Tom Preston-Werner',
			role: 'Co-founder GitHub',
			joined: 'Seed',
			image: '/investors/TOM.png'
		},
		{
			name: 'Justin Hoffman',
			role: 'Elasticsearch',
			joined: 'Seed',
			image: '/investors/justinhoffman.webp'
		},
		{
			name: 'Tobias Lütke',
			role: 'CEO Shopify',
			joined: 'Seed',
			image: '/investors/tobiaslutke.jpeg'
		},
		{
			name: 'Ry Walker',
			role: 'Astronomer',
			joined: 'Seed',
			image: '/investors/rywalker.jpeg'
		},
		{
			name: 'Zachary Smith',
			role: 'Equinix',
			joined: 'Seed',
			image: '/investors/zacharysmith.jpeg'
		},
		{
			name: 'Sanjay Poonen',
			role: 'COO VMware',
			joined: 'Seed',
			image: '/investors/sanjay.jpeg'
		},
		{
			name: 'David Mytton',
			role: 'CEO console.dev',
			joined: 'Seed',
			image: '/investors/davidmytton.jpeg'
		},
		{
			name: 'Peer Rich',
			role: 'CEO Cal.com',
			joined: 'Seed',
			image: '/investors/peer.jpeg'
		},
		{
			name: 'Lester Lee',
			role: 'Slapdash',
			joined: 'Seed',
			image: '/investors/lesterlee.jpeg'
		},
		{
			name: 'Haoyuan Li',
			role: 'Alluxio',
			joined: 'Seed',
			image: '/investors/haoyuan.jpeg'
		},
		{
			name: 'Augusto Marietti',
			role: 'CEO Kong',
			joined: 'Seed',
			image: '/investors/augusto.webp'
		},
		{
			name: 'Vijay Sharma',
			role: 'CEO Belong',
			joined: 'Seed',
			image: '/investors/sharma.jpeg'
		}
	];

	return (
		<Markdown>
			<Helmet>
				<title>Our Team - Spacedrive</title>
				<meta name="description" content="Who's behind Spacedrive?" />
			</Helmet>
			<div className="team-page relative">
				<div
					className="absolute -top-60 -right-[400px] opacity-60 w-[1000px] h-[800px]"
					style={{
						background:
							'linear-gradient(180deg, rgba(180, 180, 180, 0.76) 0%, rgba(19, 4, 168, 0.41) 95.73%)',
						filter: 'blur(300px)',
						transform: 'rotate(56.81deg)'
					}}
				></div>
				<div className="z-10 relative">
					<h1 className="leading-relaxed text-2xl">
						We believe data should be <span className="title-gradient">interoperable</span>
					</h1>
					<p className="text-[#979BAE]">
						Data shouldn't be stuck in a device ecosystem. It should be OS agnostic, permanent and
						personally owned.
					</p>
					<p className="text-[#979BAE]">
						Data we create is our legacy, that will long outlive us—open source technology is the
						only way to ensure we retain absolute control over the data that defines our lives, at
						unlimited scale.
					</p>
					<a
						href="/faq"
						className="text-[#979BAE] duration-150 opacity-50 hover:opacity-100 text-underline underline-offset-4 flex flex-row items-center"
					>
						<ArrowRight className="mr-2" />
						Read more
					</a>
					<h2 className="leading-relaxed text-xl mt-10 sm:mt-20">
						Meet the minds coming together to form the future of data sharing
					</h2>
					<div className="my-10 grid grid-cols-2 sm:grid-cols-3 gap-x-5 gap-y-10">
						{teamMembers.map((member) => (
							<TeamMember {...member} />
						))}
					</div>
					<p className="text-[#979BAE] text-sm">
						... and{' '}
						<a
							href="https://github.com/spacedriveapp/spacedrive/graphs/contributors"
							target="_blank"
							rel="noreferer"
							className="oss-credit-gradient hover:opacity-75 duration-200"
						>
							all the awesome contributors
						</a>{' '}
						to OSS technology.
					</p>
					<h2 className="leading-relaxed text-xl mt-10 sm:mt-20">People who believe in us</h2>
					<div className="my-10 grid grid-cols-2 sm:grid-cols-3 gap-x-5 gap-y-10">
						{investors.map((investor) => (
							<TeamMember {...investor} />
						))}
					</div>
				</div>
			</div>
		</Markdown>
	);
}

export default Page;
