import { Icon } from '@phosphor-icons/react';
import { Star } from '@phosphor-icons/react/dist/ssr';
import clsx from 'clsx';
import Link from 'next/link';
import { useId } from 'react';
import { ExternalLinkRegex } from '~/utils/regex-external-link';

export interface GoldenBadgeProps {
	headline: string;
	href?: string;
	target?: string;
	className?: string;
	iconComponent?: Icon;
}

export function GoldenBadge({
	headline,
	href = '/',
	target = href.match(ExternalLinkRegex)?.length ? '_blank' : undefined,
	className,
	iconComponent: Icon = Star
}: GoldenBadgeProps) {
	const id = useId();

	return (
		<Link
			href={href}
			target={target}
			className={clsx(
				className,
				'animation-delay-1 fade-in-whats-new mb-5 transition',
				'overflow-hidden rounded-full border-2 border-solid border-[hsl(38_100%_62%/30%)] bg-[url(/images/misc/gold-bg.png)] bg-[length:100%_100%] shadow-[0_0_1.625rem_hsla(43_100%_65%/50%)]',
				'box-border inline-flex w-fit flex-row px-3 py-2 pr-3.5 tracking-[0.0375em]'
			)}
		>
			<span className="inline-flex items-center gap-x-1.5 text-white drop-shadow-[0_0.2rem_0.2rem_hsla(35,100%,25%,100%)]">
				<Icon
					weight="fill"
					className="size-4 opacity-90"
					fill={`url(#${id}-star-gradient)`}
				>
					<linearGradient id={`${id}-star-gradient`} x1="0%" y1="65%" x2="100%" y2="30%">
						<stop stopColor="hsl(60 100% 83%)" offset="0%" />
						<stop stopColor="hsl(60 100% 99%)" offset="50%" />
						<stop stopColor="hsl(60 100% 90%)" offset="100%" />
					</linearGradient>
				</Icon>
				<span className="text-[0.875rem] font-semibold leading-none text-[hsl(60_100%_95%)]">
					{headline}
				</span>
			</span>
		</Link>
	);
}
