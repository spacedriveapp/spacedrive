import { Star } from '@phosphor-icons/react/dist/ssr';
import clsx from 'clsx';
import Link from 'next/link';
import { ReactNode, useId } from 'react';

export interface GoldenBadgeProps {
	headline: string;
	href?: string;
	className?: string;
}

export function GoldenBadge({ headline, href = '/', className }: GoldenBadgeProps) {
	const id = useId();

	return (
		<Link
			href={href}
			className={clsx(
				className,
				'animation-delay-1 fade-in-whats-new z-10 mb-5 flex w-fit flex-row rounded-full px-5 py-2.5 text-xs backdrop-blur-md transition sm:w-auto sm:text-base'
			)}
		>
			<div className="noise noise-strongest moise-lg inline-flex items-center justify-center gap-[8px] overflow-hidden rounded-full border-2 border-solid border-[rgba(255,164,13,0.53)] bg-[url(/images/misc/gold-bg.png)] bg-[length:100%_100%] px-3.5 py-2.5">
				<div className="flex items-center gap-2 text-white drop-shadow-[0_0.2rem_0.2rem_hsla(35,100%,25%,100%)]">
					<Star
						weight="fill"
						className="opacity-90"
						size={20}
						fill={`url(#${id}-star-gradient)`}
					>
						<linearGradient
							id={`${id}-star-gradient`}
							x1="0%"
							y1="65%"
							x2="100%"
							y2="30%"
						>
							<stop stopColor="hsl(60 100% 83%)" offset="0%" />
							<stop stopColor="hsl(60 100% 99%)" offset="50%" />
							<stop stopColor="hsl(60 100% 90%)" offset="100%" />
						</linearGradient>
					</Star>
					<p className="text-center text-base font-bold leading-[115%]">{headline}</p>
				</div>
			</div>
		</Link>
	);
}
