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
