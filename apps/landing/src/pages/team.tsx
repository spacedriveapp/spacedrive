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
						href="https://google.com/"
						className="text-[#979BAE] duration-150 opacity-50 hover:opacity-100 text-underline underline-offset-4 flex flex-row items-center"
						target="_blank"
						rel="noreferrer"
					>
						<ArrowRight className="mr-2" />
						Read our philosophy
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
				</div>
			</div>
		</Markdown>
	);
}

export default Page;
