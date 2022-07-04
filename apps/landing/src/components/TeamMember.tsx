import { Github, Twitch, Twitter } from '@icons-pack/react-simple-icons';
import clsx from 'clsx';
import React from 'react';

export interface TeamMemberProps {
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

	// Which round an investor joined at
	investmentRound?: string;
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
			className="duration-300 hover:scale-105 hover:opacity-80"
			href={props.href}
			rel="noreferer"
			target="_blank"
		>
			{props.children}
		</a>
	);
}

export function TeamMember(props: TeamMemberProps) {
	const size = props.investmentRound ? 144 : 111;

	return (
		<div className="flex flex-col">
			<img
				src={props.image}
				role="img"
				alt={`Portrait of ${props.name}`}
				width={size}
				height={size}
				className={clsx('inline-flex m-0 rounded-md', {
					'w-32 h-32 !xs:w-36 !xs:h-36 !sm:w-40 !sm:h-40': !props.investmentRound,
					'lg:w-28 lg:h-28': props.investmentRound
				})}
			/>
			<h3 className="mt-4 mb-0 text-base">{props.name}</h3>
			<p
				className={clsx('text-xs', {
					'mb-0': props.investmentRound
				})}
			>
				{props.role}
			</p>
			{props.investmentRound && (
				<p className="mt-0 mb-0 text-sm font-semibold text-gray-450">{props.investmentRound}</p>
			)}
			<div className="flex flex-row mt-auto space-x-2">
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
