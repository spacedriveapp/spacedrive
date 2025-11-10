'use client';

import { ArrowCircleDown, Icon } from '@phosphor-icons/react';
import { cva, type VariantProps } from 'class-variance-authority';
import clsx from 'clsx';
import { ComponentProps, ReactNode, useId } from 'react';

const shinyButtonVariants = cva(
	[
		'noise with-rounded-2px-border-images inline-flex flex-row items-center justify-center gap-x-2 overflow-hidden',
		'bg-gradient-to-b from-[#42B2FD] to-[#0078F0] [--border-image:linear-gradient(to_bottom,hsl(200_100%_77%/100%),hsl(200_0%_100%/5%)75%)]',
		'will-change-transform will-change-[box-shadow]',
		'transition-all duration-200 ease-out',
		'cursor-pointer'
	],
	{
		variants: {
			size: {
				default: 'py-2 pe-4 ps-3 rounded-xl',
				small: 'py-1.5 px-3 text-sm rounded-xl'
			},
			glow: {
				lg: 'shadow-[0_0px_2.5rem_hsl(207_100%_65%/50%)] hover:shadow-[0_0px_3.5rem_hsl(207_100%_65%/70%)] hover:brightness-105',
				sm: 'shadow-[0_0.125rem_1.25rem_hsl(207_50%_65%/50%)] hover:shadow-[0_0.25rem_2rem_hsl(207_50%_65%/70%)] hover:brightness-105',
				none: ''
			}
		},
		defaultVariants: {
			size: 'default',
			glow: 'lg'
		}
	}
);

type BaseShinyButtonProps = VariantProps<typeof shinyButtonVariants> & {
	icon?: Icon | ReactNode;
	children: ReactNode;
	className?: string;
};

type ShinyButtonAsButton = BaseShinyButtonProps &
	Omit<ComponentProps<'button'>, 'size'> & {
		href?: never;
	};

type ShinyButtonAsLink = BaseShinyButtonProps &
	Omit<ComponentProps<'a'>, 'size'> & {
		href: string;
	};

export type ShinyButtonProps = ShinyButtonAsButton | ShinyButtonAsLink;

export function ShinyButton({
	icon: IconComponent = <ArrowCircleDown weight="bold" size={22} />,
	size,
	glow,
	children,
	className,
	href,
	...props
}: ShinyButtonProps) {
	const id = useId();
	const iconSize = size === 'small' ? 18 : 22;

	const content = (
		<>
			{typeof IconComponent === 'function' ? (
				<IconComponent weight="bold" size={iconSize} fill={`url(#${id}-cta-gradient)`}>
					<linearGradient id={`${id}-cta-gradient`} x1="0%" y1="0%" x2="0%" y2="100%">
						<stop stopColor="hsl(0 100% 100% / 100%)" offset="0%" />
						<stop stopColor="hsl(0 100% 100% / 70%)" offset="100%" />
					</linearGradient>
				</IconComponent>
			) : (
				IconComponent
			)}
			<span
				className={clsx(
					'text-center font-sans font-semibold leading-normal text-white drop-shadow-md will-change-transform',
					size === 'small' ? 'text-sm' : 'text-base'
				)}
			>
				{children}
			</span>
		</>
	);

	const classes = clsx(shinyButtonVariants({ size, glow }), className);

	if (href) {
		return (
			<a {...(props as ComponentProps<'a'>)} href={href} className={classes}>
				{content}
			</a>
		);
	}

	return (
		<button {...(props as ComponentProps<'button'>)} className={classes}>
			{content}
		</button>
	);
}
