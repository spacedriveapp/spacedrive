import { Dribbble, Github, Gitlab, Twitch, Twitter, Website } from '@sd/assets/svgs/brands';
import clsx from 'clsx';
import Image from 'next/image';
import NextLink from 'next/link';
import { PropsWithChildren } from 'react';

export interface TeamMemberProps {
	// Name of team member
	name: string;

	// Member's role
	role: string;

	// Member's location
	location?: string;

	// Member's avatar
	imageUrl: string;

	// Socials
	socials?: {
		twitter?: string;
		twitch?: string;
		github?: string;
		gitlab?: string;
		dribbble?: string;
		website?: string;
	};

	// Which round an investor joined at
	investmentRound?: string;
}

interface LinkProps {
	href: string;
}

function Link(props: PropsWithChildren<LinkProps>) {
	return (
		<NextLink
			className="duration-300 hover:scale-105 hover:opacity-80"
			href={props.href}
			rel="noreferrer"
			target="_blank"
		>
			{props.children}
		</NextLink>
	);
}

export function TeamMember(props: TeamMemberProps) {
	const size = props.investmentRound ? 144 : 111;

	return (
		<div className="flex flex-col">
			<Image
				src={props.imageUrl}
				role="img"
				alt={`Portrait of ${props.name}`}
				width={size}
				height={size}
				className={clsx('m-0 inline-flex rounded-md object-cover', {
					'!xs:w-36 !xs:h-36 !sm:w-40 !sm:h-40 h-32 w-32': !props.investmentRound,
					'lg:h-28 lg:w-28': props.investmentRound
				})}
			/>
			<h3 className="mb-0 mt-2 text-base">{props.name}</h3>

			{props.location && (
				<p className="m-0 text-sm font-semibold text-gray-450">{props.location}</p>
			)}
			<p className="m-0 text-xs">{props.role}</p>
			{props.investmentRound && (
				<p className="m-0 text-sm font-semibold text-gray-450">{props.investmentRound}</p>
			)}
			<div className="mt-3 flex flex-row space-x-2">
				{props.socials?.twitter && (
					<Link href={props.socials.twitter}>
						<Twitter className="size-[20px]" />
					</Link>
				)}
				{props.socials?.github && (
					<Link href={props.socials.github}>
						<Github className="size-[20px]" />
					</Link>
				)}
				{props.socials?.gitlab && (
					<Link href={props.socials.gitlab}>
						<Gitlab className="size-[20px]" />
					</Link>
				)}
				{props.socials?.twitch && (
					<Link href={props.socials.twitch}>
						<Twitch className="size-[20px]" />
					</Link>
				)}
				{props.socials?.dribbble && (
					<Link href={props.socials.dribbble}>
						<Dribbble className="size-[20px]" />
					</Link>
				)}
				{props.socials?.website && (
					<Link href={props.socials.website}>
						<Website className="size-[20px]" />
					</Link>
				)}
			</div>
		</div>
	);
}
