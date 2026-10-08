import type {Icon} from '@phosphor-icons/react';
import clsx from 'clsx';
import type {ReactNode} from 'react';
import {useLocation, useNavigate} from 'react-router-dom';

export type SidebarItemTone = 'default' | 'muted' | 'warning';

export interface SidebarItemProps {
	id: string;
	label: string;
	icon?: Icon;
	iconNode?: ReactNode;
	href?: string;
	activePaths?: string[];
	activePathPrefixes?: string[];
	onSelect?: () => void;
	onContextMenu?: (event: React.MouseEvent<HTMLButtonElement>) => void;
	badge?: string | number;
	badgeLabel?: string;
	/** Drawn before the badge text, for a state a glyph says faster than a word. */
	badgeIcon?: Icon;
	disabled?: boolean;
	tone?: SidebarItemTone;
	title?: string;
}

function matchesExactPath(
	pathname: string,
	search: string,
	target: string
): boolean {
	const [targetPathname, targetSearch] = target.split('?');

	if (targetSearch !== undefined) {
		return pathname === targetPathname && search === `?${targetSearch}`;
	}

	return pathname === targetPathname;
}

/** The only interactive row used by the product sidebar. */
export function SidebarItem({
	id,
	label,
	icon,
	iconNode,
	href,
	activePaths,
	activePathPrefixes = [],
	onSelect,
	onContextMenu,
	badge,
	badgeLabel,
	badgeIcon,
	disabled = false,
	tone = 'default',
	title
}: SidebarItemProps) {
	const navigate = useNavigate();
	const location = useLocation();
	const targets = activePaths ?? (href ? [href] : []);
	const isActive =
		targets.some((target) =>
			matchesExactPath(location.pathname, location.search, target)
		) ||
		activePathPrefixes.some(
			(prefix) =>
				location.pathname === prefix ||
				location.pathname.startsWith(`${prefix}/`)
		);
	const IconComponent = icon;
	const BadgeIcon = badgeIcon;

	return (
		<button
			id={`sidebar-item-${id}`}
			type="button"
			disabled={disabled}
			onClick={() => {
				if (disabled) return;
				onSelect?.();
				if (href) navigate(href);
			}}
			onContextMenu={onContextMenu}
			title={title ?? label}
			aria-current={isActive ? 'page' : undefined}
			className={clsx(
				'group flex h-8 w-full min-w-0 items-center gap-2 rounded-lg px-2 text-left text-sm font-medium outline-none transition-colors',
				'focus-visible:ring-accent/70 focus-visible:ring-2',
				isActive
					? 'bg-sidebar-selected text-sidebar-ink'
					: 'hover:bg-sidebar-box hover:text-sidebar-ink',
				!isActive && tone === 'default' && 'text-sidebar-ink-dull',
				!isActive && tone === 'muted' && 'text-sidebar-ink-faint',
				!isActive && tone === 'warning' && 'text-status-warning',
				disabled && 'cursor-not-allowed opacity-40'
			)}
		>
			<span
				className={clsx(
					'flex size-4 shrink-0 items-center justify-center',
					!isActive && 'opacity-70 group-hover:opacity-100'
				)}
			>
				{iconNode ??
					(IconComponent ? (
						<IconComponent size={16} weight="bold" />
					) : null)}
			</span>
			<span className="min-w-0 flex-1 truncate">{label}</span>
			{badge !== undefined && (
				<span
					aria-label={badgeLabel}
					className={clsx(
						'flex shrink-0 items-center gap-1 rounded-full px-1.5 py-0.5 text-[10px] font-semibold leading-none',
						tone === 'warning'
							? 'bg-status-warning/15 text-status-warning'
							: 'bg-sidebar-selected text-sidebar-ink-dull'
					)}
				>
					{BadgeIcon && <BadgeIcon size={10} weight="bold" />}
					{badge}
				</span>
			)}
		</button>
	);
}
