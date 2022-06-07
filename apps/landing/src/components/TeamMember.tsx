import React from 'react';

import { ReactComponent as Github } from '../../../../packages/interface/src/assets/svg/github.svg';
import { ReactComponent as Twitch } from '../../../../packages/interface/src/assets/svg/twitch.svg';
import { ReactComponent as Twitter } from '../../../../packages/interface/src/assets/svg/twitter.svg';

interface TeamMemberProps {
	// Name of team member
	name: string;

	// Member's role
	role: string;

	// Member's avatar
	image: string;

	// Socials
	socials: {
		twitter: string;
		twitch: string;
		github: string;
	};
}

export function TeamMember(props: TeamMemberProps) {
	return (
		<div>
			<div
				role="img"
				aria-label={`Image of ${props.name}`}
				className="w-36 h-36 bg-cover bg-center"
				style={{
					boxShadow: 'inset 0px 0px 0px 5px rgba(255, 255, 255, 0.13)',
					backgroundImage: `url('${props.image}')`
				}}
			/>
			<h3 className="text-base mb-0 mt-4">{props.name}</h3>
			<p className="text-sm mb-2">{props.role}</p>
			<div className="flex flex-row space-x-2">
				{props.socials.twitter && (
					<a
						className="hover:scale-105 hover:opacity-80 duration-300"
						href={props.socials.twitter}
						rel="noreferer"
						target="_blank"
					>
						<Twitter />
					</a>
				)}
				{props.socials.github && (
					<a
						className="hover:scale-105 hover:opacity-80 duration-300"
						href={props.socials.github}
						rel="noreferer"
						target="_blank"
					>
						<Github />
					</a>
				)}
				{props.socials.twitch && (
					<a
						className="hover:scale-105 hover:opacity-80 duration-300"
						href={props.socials.twitch}
						rel="noreferer"
						target="_blank"
					>
						<Twitch />
					</a>
				)}
			</div>
		</div>
	);
}
