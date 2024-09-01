import { ArrowCircleDown, Icon } from '@phosphor-icons/react';
import clsx from 'clsx';
import Link from 'next/link';
import { ReactNode, useId } from 'react';
import { Platform } from '~/utils/current-platform';

type CtaButtonProps = { iconComponent?: Icon; glow?: 'lg' | 'sm' | 'none' } & (
	| {
			href: string;
			children: ReactNode;
	  }
	| {
			platform: Platform | null;
	  }
);

export function CtaPrimaryButton({
	iconComponent: Icon = ArrowCircleDown,
	glow = 'lg',
	...props
}: CtaButtonProps) {
	const href =
		'href' in props
			? props.href
			: `https://spacedrive.com/api/releases/desktop/stable/${props.platform?.os ?? 'linux'}/x86_64`;
	const platformName =
		'platform' in props
			? props.platform?.name === 'macOS'
				? 'Mac'
				: props.platform?.name
			: undefined;
	const children =
		'children' in props ? props.children : `Download for ${platformName ?? 'Linux'}`;

	const id = useId();

	return (
		<Link
			href={href}
			className={clsx(
				'noise with-rounded-2px-border-images inline-flex w-52 cursor-pointer flex-row items-center justify-center gap-x-2 overflow-hidden rounded-xl px-3 py-2',
				'bg-gradient-to-b from-[#42B2FD] to-[#0078F0] [--border-image:linear-gradient(to_bottom,hsl(200_100%_77%/100%),hsl(200_0%_100%/5%)75%)]',
				{
					'shadow-[0_0px_2.5rem_hsl(207_100%_65%/50%)]': glow === 'lg',
					'shadow-[0_0.125rem_1.25rem_hsl(207_50%_65%/50%)]': glow === 'sm'
				}
			)}
		>
			<Icon weight="bold" size={22} fill={`url(#${id}-cta-gradient)`}>
				<linearGradient id={`${id}-cta-gradient`} x1="0%" y1="0%" x2="0%" y2="100%">
					<stop stopColor="hsl(0 100% 100% / 100%)" offset="0%" />
					<stop stopColor="hsl(0 100% 100% / 70%)" offset="100%" />
				</linearGradient>
			</Icon>
			<span className="text-center font-sans text-base font-semibold leading-normal text-white drop-shadow-md">
				{children}
			</span>
		</Link>
	);
}
