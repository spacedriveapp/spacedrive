import { Github, Twitch, Twitter } from '@icons-pack/react-simple-icons';
import clsx from 'clsx';
import React, { useEffect } from 'react';

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
	const [image, setImage] = React.useState<string | null>(null);

	useEffect(() => {
		import(`../assets/images/${props.image}`).then(({ default: path }) => {
			setImage(path);
		});
	}, [props.image]);

	return (
		<div>
			<img
				src={image ?? ''}
				role="img"
				alt={`Portrait of ${props.name}`}
				className={clsx('rounded-md', {
					'w-40 h-40 xs:w-32 xs:h-32 sm:w-36 sm:h-36': !props.investmentRound,
					'w-28 h-28': props.investmentRound
				})}
			/>
			<h3 className="text-base mb-0 mt-4">{props.name}</h3>
			<p
				className={clsx('text-sm', {
					'mb-0': props.investmentRound
				})}
			>
				{props.role}
			</p>
			{props.investmentRound && (
				<p className="text-sm font-semibold mt-1 mb-0">{props.investmentRound}</p>
			)}
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
