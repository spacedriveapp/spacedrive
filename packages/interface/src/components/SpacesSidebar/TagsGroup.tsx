import { CaretRight, Tag as TagIcon } from '@phosphor-icons/react';
import clsx from 'clsx';

interface TagsGroupProps {
	isCollapsed: boolean;
	onToggle: () => void;
}

export function TagsGroup({ isCollapsed, onToggle }: TagsGroupProps) {
	// TODO: Fetch tags from backend when tags.list query is available
	const tags: any[] = [];

	return (
		<div>
			{/* Header */}
			<button
				onClick={onToggle}
				className="mb-1 flex w-full items-center gap-2 px-1 text-xs font-semibold uppercase tracking-wider text-sidebar-ink-faint hover:text-sidebar-ink"
			>
				<CaretRight
					className={clsx('transition-transform', !isCollapsed && 'rotate-90')}
					size={10}
					weight="bold"
				/>
				<span>Tags</span>
			</button>

			{/* Items */}
			{!isCollapsed && (
				<div className="space-y-0.5">
					{tags.length === 0 ? (
						<div className="px-2 py-1 text-xs text-sidebar-ink-faint">No tags yet</div>
					) : (
						tags.map((tag) => (
							<button
								key={tag.id}
								className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-sm font-medium text-sidebar-ink-dull hover:bg-sidebar-selected hover:text-sidebar-ink"
							>
								<TagIcon size={18} weight="bold" />
								<span className="flex-1 truncate text-left">{tag.name}</span>
							</button>
						))
					)}
				</div>
			)}
		</div>
	);
}
