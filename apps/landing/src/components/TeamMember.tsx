import { Github, Twitch, Twitter } from '@icons-pack/react-simple-icons';
import clsx from 'clsx';
import React from 'react';

interface TeamMemberProps {
	// Name of team member
	name: string;

	// Member's role
	role: string;

	// Member's avatar
	image: string;

	// Socials
	socials?: {
		twitter?: string;
		twitch?: string;
		github?: string;
	};

	// When person joined
	joined?: string;
}

interface LinkProps {
	// Elements inside anchor tag
	children: React.ReactNode;

	// Anchor href
	href: string;
}

function Link(props: LinkProps) {
	return (
		<a
			className="hover:scale-105 hover:opacity-80 duration-300"
			href={props.href}
			rel="noreferer"
			target="_blank"
		>
			{props.children}
		</a>
	);
}

export function TeamMember(props: TeamMemberProps) {
	return (
		<div>
			<div
				role="img"
				aria-label={`Image of ${props.name}`}
				className={clsx(' bg-cover bg-center rounded-md', {
					'w-40 h-40 xs:w-32 xs:h-32 sm:w-36 sm:h-36': !props.joined,
					'w-28 h-28': props.joined
				})}
				style={{
					backgroundImage: `url('${props.image}')`
				}}
			/>
			<h3 className="text-base mb-0 mt-4">{props.name}</h3>
			<p
				className={clsx('text-sm', {
					'mb-0': props.joined
				})}
			>
				{props.role}
			</p>
			{props.joined && <p className="text-sm font-semibold mt-1 mb-0">{props.joined}</p>}
			<div className="flex flex-row space-x-2 mt-3">
				{props.socials?.twitter && (
					<Link href={props.socials.twitter}>
						<Twitter className="w-[20px] h-[20px]" />
					</Link>
				)}
				{props.socials?.github && (
					<Link href={props.socials.github}>
						<Github className="w-[20px] h-[20px]" />
					</Link>
				)}
				{props.socials?.twitch && (
					<Link href={props.socials.twitch}>
						<Twitch className="w-[20px] h-[20px]" />
					</Link>
				)}
			</div>
		</div>
	);
}
