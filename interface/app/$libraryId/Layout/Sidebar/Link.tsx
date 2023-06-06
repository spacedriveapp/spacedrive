/* eslint-disable tailwindcss/classnames-order */
import { cva } from 'class-variance-authority';
import clsx from 'clsx';
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

export default ({
	className,
	onClick,
	disabled,
	...props
}: PropsWithChildren<NavLinkProps & { disabled?: boolean }>) => {
	const os = useOperatingSystem();

	return (
		<NavLink
			onClick={(e) => (disabled ? e.preventDefault() : onClick?.(e))}
			className={({ isActive }) =>
				clsx(
					styles({ active: isActive, transparent: os === 'macOS' }),
					disabled && 'pointer-events-none opacity-50',
					className
				)
			}
			{...props}
		>
			{props.children}
		</NavLink>
	);
};
