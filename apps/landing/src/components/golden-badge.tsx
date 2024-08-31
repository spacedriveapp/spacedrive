import { Star } from '@phosphor-icons/react/dist/ssr';
import clsx from 'clsx';
import Link from 'next/link';

export interface GoldenBadgeProps {
	headline: string;
	href?: string;
	link?: string;
	className?: string;
}

export function GoldenBadge(props: GoldenBadgeProps) {
	const { headline, href, link } = props;

	return (
		<Link
			href={href ?? '/'}
			className={clsx(
				props.className,
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
				<div className="flex items-center gap-2">
					<Star weight="fill" className="text-white" size={20} />
					<p className="text-center text-[16px] font-[700] font-normal leading-[115%] text-white">
						{headline}
					</p>
				</div>
			</div>
		</Link>
	);
}
