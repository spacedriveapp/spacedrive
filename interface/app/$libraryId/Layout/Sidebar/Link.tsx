import { cva } from 'class-variance-authority';
import clsx from 'clsx';
import { forwardRef } from 'react';
import { PropsWithChildren } from 'react';
import { NavLink, NavLinkProps } from 'react-router-dom';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';

const styles = cva(
	'max-w flex grow flex-row items-center gap-0.5 truncate rounded px-2 py-1 ring-inset ring-transparent text-sm font-medium outline-none ring-offset-0 focus:ring-1 focus:ring-accent focus:ring-offset-0',
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
			onClick={(e) => (disabled ? e.preventDefault() : onClick?.(e))}
			className={({ isActive }) =>
				clsx(
					"ring-0", // Remove ugly outline ring on Chrome Windows & Linux
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
