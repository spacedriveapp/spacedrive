'use client';

import { cva, cx, VariantProps } from 'class-variance-authority';
import clsx from 'clsx';
import { ComponentProps, forwardRef } from 'react';
import { Link } from 'react-router-dom';

export type ButtonBaseProps = VariantProps<typeof buttonStyles>;

export type ButtonProps = ButtonBaseProps &
	React.ButtonHTMLAttributes<HTMLButtonElement> & {
		href?: undefined;
	};

export type LinkButtonProps = ButtonBaseProps &
	React.AnchorHTMLAttributes<HTMLAnchorElement> & {
		href?: string;
	};

type Button = {
	(props: ButtonProps): JSX.Element;
	(props: LinkButtonProps): JSX.Element;
};

const hasHref = (props: ButtonProps | LinkButtonProps): props is LinkButtonProps => 'href' in props;

export const buttonStyles = cva(
	[
		'cursor-default items-center rounded-md border font-plex font-semibold tracking-wide outline-none transition-colors duration-100',
		'disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-70',
		'focus:ring-none focus:ring-offset-none cursor-pointer ring-offset-app-box'
	],
	{
		variants: {
			size: {
				icon: '!p-1',
				lg: 'text-md px-3 py-1.5 font-medium',
				md: 'px-2.5 py-1.5 text-sm font-medium',
				sm: 'px-2 py-1 text-sm font-medium',
				xs: 'px-1.5 py-0.5 text-xs font-normal'
			},
			variant: {
				default: [
					'bg-transparent hover:bg-app-hover active:bg-app-selected',
					'border-transparent hover:border-app-line active:border-app-line'
				],
				subtle: [
					'border-transparent hover:border-app-line/50 active:border-app-line active:bg-app-box/30'
				],
				outline: [
					'border-sidebar-line/60 hover:border-sidebar-line active:border-sidebar-line/30'
				],
				dotted: [
					`rounded border border-dashed border-sidebar-line/70 text-center text-xs font-medium text-ink-faint transition hover:border-sidebar-line hover:bg-sidebar-selected/5`
				],
				gray: [
					'bg-app-button hover:bg-app-hover focus:bg-app-selected',
					'border-app-line hover:border-app-line focus:ring-1 focus:ring-accent'
				],
				accent: [
					'border border-accent bg-accent text-white shadow-md shadow-app-shade/10 hover:bg-accent-faint focus:outline-none',
					'focus:ring-1 focus:ring-accent focus:ring-offset-2 focus:ring-offset-app-selected'
				],
				colored: ['text-white shadow-sm hover:bg-opacity-90 active:bg-opacity-100'],
				bare: ''
			},
			rounding: {
				none: 'rounded-none',
				left: 'rounded-l-md rounded-r-none',
				right: 'rounded-l-none rounded-r-md',
				both: 'rounded-md'
			}
		},
		defaultVariants: {
			size: 'sm',
			variant: 'default'
		}
	}
);

export const Button = forwardRef<
	HTMLButtonElement | HTMLAnchorElement,
	ButtonProps | LinkButtonProps
>(({ className, ...props }, ref) => {
	className = cx(buttonStyles(props), className);
	return hasHref(props) ? (
		<a {...props} ref={ref as any} className={cx(className, 'inline-block no-underline')} />
	) : (
		<button type="button" {...(props as ButtonProps)} ref={ref as any} className={className} />
	);
});

export const ButtonLink = forwardRef<
	HTMLAnchorElement,
	ButtonBaseProps & ComponentProps<typeof Link>
>(({ className, size, variant, ...props }, ref) => {
	return (
		<Link
			ref={ref}
			className={buttonStyles({
				size,
				variant,
				className: clsx(
					'no-underline disabled:cursor-not-allowed disabled:opacity-50',
					className
				)
			})}
			{...props}
		/>
	);
});
