import { cva } from 'class-variance-authority';
import clsx from 'clsx';
import { forwardRef, PropsWithChildren, useEffect } from 'react';
import { NavLink, NavLinkProps } from 'react-router-dom';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import { usePlatform } from '~/util/Platform';

const styles = cva(
	[
		'max-w flex grow flex-row items-center gap-0.5 truncate rounded px-2 py-1 font-plex text-sm font-medium tracking-wide outline-none',
		'ring-inset ring-transparent ring-offset-0 focus:ring-1 focus:ring-accent focus:ring-offset-0'
	],
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
	PropsWithChildren<Omit<NavLinkProps, 'onClick'> & { disabled?: boolean }>
>(({ className, disabled, ...props }, ref) => {
	const os = useOperatingSystem();
	const { platform } = usePlatform();

	//prevents middle click from opening new tab
	useEffect(() => {
		const handleClick = (e: MouseEvent) => {
			if (e.button === 1) e.preventDefault();
		};
		document.addEventListener('auxclick', handleClick);
		return () => {
			document.removeEventListener('auxclick', handleClick);
		};
	}, []);

	return (
		<NavLink
			onClick={(e) => {
				const shouldOpenNewTab = e.metaKey || e.ctrlKey || e.shiftKey;

				const shouldOverrideNewTabBehaviour = shouldOpenNewTab && platform === 'tauri';

				if (shouldOverrideNewTabBehaviour || disabled) {
					e.preventDefault();

					if (shouldOverrideNewTabBehaviour) e.currentTarget.click();
				}
			}}
			className={({ isActive }) =>
				clsx(
					'relative ring-0', // ring-0 to remove ugly outline ring on Chrome Windows & Linux
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
