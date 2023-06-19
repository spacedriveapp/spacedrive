import { cva } from 'class-variance-authority';
import clsx from 'clsx';
import { PropsWithChildren, forwardRef } from 'react';
import { NavLink, NavLinkProps } from 'react-router-dom';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';

const styles = cva(
	'max-w flex grow flex-row items-center gap-0.5 truncate rounded px-2 py-1 text-sm font-medium outline-none ring-inset ring-transparent ring-offset-0 focus:ring-1 focus:ring-accent focus:ring-offset-0',
	{
		variants: {
			active: {
				true: 'bg-sidebar-selected/40 text-sidebar-ink',
				false: 'text-sidebar-inkDull'
			},
			transparent: {
				true: 'bg-opacity-90',
				false: ''
			}
		}
	}
);

const Link = forwardRef<
	HTMLAnchorElement,
	PropsWithChildren<NavLinkProps & { disabled?: boolean }>
>(({ className, onClick, disabled, ...props }, ref) => {
	const os = useOperatingSystem();

	return (
		<NavLink
			onClick={(e) => {
				// Prevent default action if Command (metaKey) or Control is pressed
				if (e.metaKey || e.ctrlKey || disabled) {
					e.preventDefault();
				} else {
					onClick?.(e);
				}
			}}
			className={({ isActive }) =>
				clsx(
					'ring-0', // Remove ugly outline ring on Chrome Windows & Linux
					styles({ active: isActive, transparent: os === 'macOS' }),
					disabled && 'pointer-events-none opacity-50',
					className
				)
			}
			ref={ref}
			{...props}
		>
			{props.children}
		</NavLink>
	);
});

export default Link;
