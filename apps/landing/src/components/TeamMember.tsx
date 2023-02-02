import { Dribbble, Github, Twitch, Twitter } from '@icons-pack/react-simple-icons';
import clsx from 'clsx';
import { PropsWithChildren } from 'react';

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
		dribbble?: string;
	};

	// Which round an investor joined at
	investmentRound?: string;
}

interface LinkProps {
	href: string;
}

function Link(props: PropsWithChildren<LinkProps>) {
	return (
		<a
			className="duration-300 hover:scale-105 hover:opacity-80"
			href={props.href}
			rel="noreferrer"
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
				className={clsx('m-0 inline-flex rounded-md', {
					'!xs:w-36 !xs:h-36 !sm:w-40 !sm:h-40 h-32 w-32': !props.investmentRound,
					'lg:h-28 lg:w-28': props.investmentRound
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
				<p className="text-gray-450 mt-0 mb-0 text-sm font-semibold">{props.investmentRound}</p>
			)}
			<div className="mt-auto flex flex-row space-x-2">
				{props.socials?.twitter && (
					<Link href={props.socials.twitter}>
						<Twitter className="h-[20px] w-[20px]" />
					</Link>
				)}
				{props.socials?.github && (
					<Link href={props.socials.github}>
						<Github className="h-[20px] w-[20px]" />
					</Link>
				)}
				{props.socials?.twitch && (
					<Link href={props.socials.twitch}>
						<Twitch className="h-[20px] w-[20px]" />
					</Link>
				)}
				{props.socials?.dribbble && (
					<Link href={props.socials.dribbble}>
						<Dribbble className="h-[20px] w-[20px]" />
					</Link>
				)}
			</div>
		</div>
	);
}
