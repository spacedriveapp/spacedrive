import {CaretRight} from '@phosphor-icons/react';
import clsx from 'clsx';
import type {ReactNode} from 'react';

interface SidebarSectionProps {
	title?: string;
	children: ReactNode;
	collapsed?: boolean;
	onToggle?: () => void;
	action?: ReactNode;
	className?: string;
}

/** A quiet label and spacing boundary for related sidebar destinations. */
export function SidebarSection({
	title,
	children,
	collapsed = false,
	onToggle,
	action,
	className
}: SidebarSectionProps) {
	return (
		<section className={clsx('space-y-1', className)}>
			{title && (
				<div className="flex h-6 items-center gap-1 px-1">
					<button
						type="button"
						onClick={onToggle}
						disabled={!onToggle}
						aria-expanded={onToggle ? !collapsed : undefined}
						className={clsx(
							'text-sidebar-ink-faint flex min-w-0 flex-1 items-center gap-1.5 rounded px-1 text-left text-[11px] font-semibold uppercase tracking-wide',
							onToggle && 'hover:text-sidebar-ink-dull'
						)}
					>
						{onToggle && (
							<CaretRight
								size={10}
								weight="bold"
								className={clsx(
									'transition-transform',
									!collapsed && 'rotate-90'
								)}
							/>
						)}
						<span className="truncate">{title}</span>
					</button>
					{action}
				</div>
			)}
			{!collapsed && <div className="space-y-0.5">{children}</div>}
		</section>
	);
}
