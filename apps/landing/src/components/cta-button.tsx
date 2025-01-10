import clsx from 'clsx';
import Link from 'next/link';
import { ComponentProps, ReactNode } from 'react';

interface CtaButtonProps extends ComponentProps<'button'> {
	icon?: ReactNode;
	glow?: 'lg' | 'sm' | 'none';
	href: string;
	children: ReactNode;
	highlighted?: boolean;
}

export function CtaButton({
	icon,
	glow = 'lg',
	href,
	children,
	highlighted = false,
	...props
}: CtaButtonProps) {
	return (
		<Link
			href={href}
			className={clsx(
				props.className,
				'noise with-rounded-2px-border-images inline-flex flex-row items-center justify-center gap-x-2 overflow-hidden rounded-xl py-2 pe-4 ps-3 transition-all hover:brightness-110',
				{
					'bg-gradient-to-b from-[#42B2FD] to-[#0078F0] [--border-image:linear-gradient(to_bottom,hsl(200_100%_77%/100%),hsl(200_0%_100%/5%)75%)]':
						highlighted,
					'bg-[#213448]/40 [--border-image:linear-gradient(to_bottom,hsl(210_22%_37%/40%),hsl(220_7%_68%/0%)75%)]':
						!highlighted,
					'shadow-[0_0px_2.5rem_hsl(207_100%_65%/50%)]': highlighted && glow === 'lg',
					'shadow-[0_0.125rem_1.25rem_hsl(207_50%_65%/50%)]': highlighted && glow === 'sm'
				}
			)}
		>
			{icon}
			<span
				className={clsx('text-center font-sans text-base font-semibold leading-normal', {
					'text-white drop-shadow-md': highlighted,
					'text-white/80': !highlighted
				})}
			>
				{children}
			</span>
		</Link>
	);
}
