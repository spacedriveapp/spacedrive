import { cva } from 'class-variance-authority';
import clsx from 'clsx';
import { PropsWithChildren } from 'react';
import { NavLink, NavLinkProps } from 'react-router-dom';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';

const styles = cva(
	'max-w flex grow flex-row items-center gap-0.5 truncate rounded px-2 py-1 text-sm font-medium outline-none ring-offset-sidebar focus:ring-2 focus:ring-accent focus:ring-offset-2',
	{
		variants: {
			active: {
				true: 'bg-sidebar-selected/40 text-ink',
				false: 'text-ink-dull'
			},
			transparent: {
				true: 'bg-opacity-90',
				false: ''
			}
		}
	}
);

export default (props: PropsWithChildren<NavLinkProps & { disabled?: boolean }>) => {
	const os = useOperatingSystem();

	return (
		<NavLink
			{...props}
			onClick={(e) => (props.disabled ? e.preventDefault() : props.onClick?.(e))}
			className={({ isActive }) =>
				clsx(
					styles({ active: isActive, transparent: os === 'macOS' }),
					props.disabled && 'pointer-events-none opacity-50',
					props.className
				)
			}
		>
			{props.children}
		</NavLink>
	);
};
