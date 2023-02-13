import { VariantProps, cva, cx } from 'class-variance-authority';
import { forwardRef } from 'react';
import { Link, LinkProps } from 'react-router-dom';

export interface ButtonBaseProps extends VariantProps<typeof styles> {}

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
	[
		'cursor-default items-center rounded-md border outline-none transition-colors duration-100',
		'disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-70',
		'ring-offset-app-box focus:ring-accent focus:ring-2 focus:ring-offset-2'
	],
	{
		variants: {
			pressEffect: {
				true: 'active:translate-y-[1px]'
			},
			size: {
				icon: '!p-1',
				md: 'text-md py-1 px-3 font-medium',
				sm: 'py-1 px-2 text-sm font-medium'
			},
			variant: {
				default: [
					'active:bg-app-selected hover:bg-app-hover bg-transparent',
					'hover:border-app-line active:border-app-line border-transparent'
				],
				subtle: [
					'hover:border-app-line/50 active:border-app-line active:bg-app-box/30 border-transparent'
				],
				outline: [
					'border-sidebar-line/60 hover:border-sidebar-line active:border-sidebar-line active:border-sidebar-line/30'
				],
				gray: [
					'bg-app-button active:bg-app-selected hover:bg-app-hover',
					'border-app-line hover:border-app-line active:border-app-active'
				],
				accent: [
					'bg-accent active:bg-accent hover:bg-accent-faint border-accent-deep hover:border-accent active:border-accent-deep shadow-app-shade/10 text-white shadow-md'
				],
				colored: ['hover:bg-opacity/90 active:bg-opacity/100 text-white shadow-sm'],
				bare: ''
			}
		},
		defaultVariants: {
			size: 'md',
			variant: 'default'
		}
	}
);

export const Button = forwardRef<
	HTMLButtonElement | HTMLAnchorElement,
	ButtonProps | LinkButtonProps
>(({ className, ...props }, ref) => {
	className = cx(styles(props), className);
	return hasHref(props) ? (
		<a {...props} ref={ref as any} className={cx(className, 'inline-block no-underline')} />
	) : (
		<button type="button" {...(props as ButtonProps)} ref={ref as any} className={className} />
	);
});

export const ButtonLink = forwardRef<
	HTMLLinkElement,
	ButtonBaseProps & LinkProps & React.RefAttributes<HTMLAnchorElement>
>(({ className, to, ...props }, ref) => {
	className = cx(
		styles(props),
		'no-underline disabled:opacity-50 disabled:cursor-not-allowed',
		className
	);

	return (
		<Link to={to} ref={ref as any} className={className}>
			{props.children}
		</Link>
	);
});
