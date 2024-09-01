import { Star } from '@phosphor-icons/react/dist/ssr';
import clsx from 'clsx';
import Link from 'next/link';
import { useId } from 'react';

export interface GoldenBadgeProps {
	headline: string;
	href?: string;
	className?: string;
}

export function GoldenBadge({ className, headline, href }: GoldenBadgeProps) {
	const componentId = useId();

	return (
		<Link
			href={href ?? '/'}
			className={clsx(
				className,
				'animation-delay-1 fade-in-whats-new z-10 mb-5 flex w-fit flex-row rounded-full px-5 py-2.5 text-xs backdrop-blur-md transition sm:w-auto sm:text-base'
			)}
		>
			<div
				className="inline-flex items-center justify-center gap-[8px] rounded-[20.9px] border-[1.5px] border-solid border-[rgba(255,164,13,0.53)] px-3.5 py-2.5 shadow-[0px_0px_25.4px_0px_rgba(255,203,75,0.50)]"
				style={{
					backgroundImage: `
    linear-gradient(
      0deg,
      rgba(0, 0, 0, 0.16) 0%,
      rgba(0, 0, 0, 0.16) 100%
    ),
    url('images/misc/NoisePattern.png'),
    linear-gradient(
      85deg,
      #be7900 3.85%,
      #ffdf94 13.34%,
      #e0a633 30.97%,
      #dfa431 52.92%,
      #be7900 84.26%
    )`,
					backgroundSize: '100px 100px, cover, cover',
					backgroundPosition: '0% 0%, center, center',
					backgroundRepeat: 'repeat, no-repeat, no-repeat',
					backgroundColor: 'lightgray',
					backgroundBlendMode: 'darken, overlay, normal'
				}}
			>
				<div className="flex items-center gap-2 text-white">
					<Star
						weight="fill"
						className="mix-blend-overlay"
						size={20}
						filter={`url(#goldstar-${componentId}-shadow)`}
					>
						<filter
							id={`goldstar-${componentId}-shadow`}
							color-interpolation-filters="sRGB"
						>
							<feDropShadow
								dx="0"
								dy="20"
								stdDeviation="20"
								color="red"
								flood-opacity="1.0"
							/>
						</filter>
					</Star>
					<p className="text-center text-base font-bold leading-[115%] drop-shadow-[0_0.2rem_0.2rem_hsla(35,100%,25%,100%)]">
						{headline}
					</p>
				</div>
			</div>
		</Link>
	);
}
