import { VariantProps, cva } from 'class-variance-authority';
import clsx from 'clsx';
import { forwardRef } from 'react';
import { Link, LinkProps } from 'react-router-dom';

export interface ButtonBaseProps extends VariantProps<typeof styles> {
	icon?: React.ReactNode;
}

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

const styles = cva(
	'border rounded-md items-center transition-colors duration-100 cursor-default disabled:opacity-50 disabled:cursor-not-allowed',
	{
		variants: {
			pressEffect: {
				true: 'active:translate-y-[1px]'
			},
			loading: {
				true: 'opacity-70'
			},
			padding: {
				thin: '!p-1',
				sm: '!p-1.5'
			},
			noBorder: {
				true: 'border-0'
			},
			size: {
				md: 'py-1 px-3 text-md font-medium',
				sm: 'py-1 px-2 text-xs font-medium'
			},
			justify: {
				left: 'justify-left',
				center: ''
			},
			variant: {
				default: [
					'bg-gray-50 shadow-sm hover:bg-gray-100 active:bg-gray-50 dark:bg-transparent',
					'dark:active:bg-gray-600 dark:hover:bg-gray-550 dark:active:opacity-80',
					'border-gray-100 hover:border-gray-200 active:border-gray-200',
					'dark:border-transparent dark:active:border-gray-600 dark:hover:border-gray-500',
					'text-gray-700 hover:text-gray-900 active:text-gray-600',
					'dark:text-gray-200 dark:active:text-white dark:hover:text-white'
				],
				gray: [
					'bg-gray-100 shadow-sm hover:bg-gray-200 active:bg-gray-100 dark:bg-gray-500 dark:hover:bg-gray-500 dark:bg-opacity-80 dark:hover:bg-opacity-100 dark:active:opacity-80',
					'border-gray-200 hover:border-gray-300 active:border-gray-200 dark:border-gray-500 dark:hover:border-gray-500',
					'text-gray-700 hover:text-gray-900 active:text-gray-600 dark:text-gray-200 dark:active:text-white dark:hover:text-white'
				],
				primary: [
					'bg-primary-600 text-white shadow-sm active:bg-primary-600 hover:bg-primary border-primary-500 hover:border-primary-500 active:border-primary-700'
				],
				colored: ['text-white shadow-sm hover:bg-opacity-90 active:bg-opacity-100'],
				selected: [
					'bg-gray-100 dark:bg-gray-500 text-black hover:text-black active:text-black dark:hover:text-white dark:text-white'
				]
			}
		},
		defaultVariants: {
			size: 'md',
			justify: 'center',
			variant: 'default'
		}
	}
);

export const Button = forwardRef<
	HTMLButtonElement | HTMLAnchorElement,
	ButtonProps | LinkButtonProps
>(({ className, ...props }, ref) => {
	className = clsx(styles(props), className);

	let children = (
		<>
			{props.icon}
			{props.children}
		</>
	);

	return hasHref(props) ? (
		<a {...props} ref={ref as any} className={clsx(className, 'no-underline inline-block')}>
			{children}
		</a>
	) : (
		<button {...(props as ButtonProps)} ref={ref as any} className={className}>
			{children}
		</button>
	);
});

export const ButtonLink = forwardRef<
	HTMLLinkElement,
	ButtonBaseProps & LinkProps & React.RefAttributes<HTMLAnchorElement>
>(({ className, to, ...props }, ref) => {
	className = clsx(
		styles(props),
		'no-underline disabled:opacity-50 disabled:cursor-not-allowed',
		className
	);

	return (
		<Link to={to} ref={ref as any} className={className}>
			<>
				{props.icon}
				{props.children}
			</>
		</Link>
	);
});
