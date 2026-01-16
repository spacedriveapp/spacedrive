import {
	horizontalListSortingStrategy,
	SortableContext,
	useSortable
} from '@dnd-kit/sortable';
import {CSS} from '@dnd-kit/utilities';
import {Plus, X} from '@phosphor-icons/react';
import clsx from 'clsx';
import {LayoutGroup, motion} from 'framer-motion';
import {useMemo} from 'react';
import type {Tab} from '.';
import {useTabManager} from './useTabManager';

interface SortableTabProps {
	tab: Tab;
	isActive: boolean;
	onSwitch: (tabId: string) => void;
	onClose: (tabId: string) => void;
}

function SortableTab({tab, isActive, onSwitch, onClose}: SortableTabProps) {
	const {
		attributes,
		listeners,
		setNodeRef,
		transform,
		transition,
		isDragging
	} = useSortable({
		id: tab.id,
		data: {
			type: 'tab',
			tabId: tab.id
		}
	});

	const style = {
		transform: CSS.Transform.toString(transform),
		transition
	};

	return (
		<button
			ref={setNodeRef}
			style={style}
			{...attributes}
			{...listeners}
			onClick={() => onSwitch(tab.id)}
			className={clsx(
				'group relative flex min-w-0 flex-1 items-center justify-center rounded-full py-1.5 text-[13px]',
				isActive
					? 'text-ink'
					: 'text-ink-dull hover:text-ink hover:bg-app-hover/50',
				isDragging && 'z-50 opacity-50'
			)}
		>
			{isActive && (
				<motion.div
					layoutId="activeTab"
					className="bg-app-selected absolute inset-0 rounded-full shadow-sm"
					initial={false}
					transition={{
						type: 'spring',
						stiffness: 500,
						damping: 35
					}}
				/>
			)}
			{/* Close button - absolutely positioned left */}
			<span
				onClick={(e) => {
					e.stopPropagation();
					onClose(tab.id);
				}}
				className={clsx(
					'absolute left-1.5 z-10 flex size-5 cursor-pointer items-center justify-center rounded-full transition-all',
					isActive
						? 'hover:bg-app-hover opacity-60 hover:opacity-100'
						: 'hover:bg-app-hover opacity-0 hover:!opacity-100 group-hover:opacity-60'
				)}
				title="Close tab"
			>
				<X size={10} weight="bold" />
			</span>
			<span className="relative z-10 truncate px-6">{tab.title}</span>
		</button>
	);
}

export function TabBar() {
	const {tabs, activeTabId, switchTab, closeTab, createTab} = useTabManager();

	// Ensure activeTabId exists in tabs array, fallback to first tab
	// Memoize to prevent unnecessary rerenders during rapid state updates
	const safeActiveTabId = useMemo(() => {
		return tabs.find((t) => t.id === activeTabId)?.id ?? tabs[0]?.id;
	}, [tabs, activeTabId]);

	// Don't show tab bar if only one tab
	if (tabs.length <= 1) {
		return null;
	}

	return (
		<div className="bg-app-box/80 mx-2 flex h-9 shrink-0 items-center gap-1 rounded-full px-1 shadow-sm backdrop-blur-sm">
			<LayoutGroup id="tab-bar">
				<SortableContext
					items={tabs.map((tab) => tab.id)}
					strategy={horizontalListSortingStrategy}
				>
					<div className="flex min-w-0 flex-1 items-center gap-1">
						{tabs.map((tab) => {
							const isActive = tab.id === safeActiveTabId;

							return (
								<SortableTab
									key={tab.id}
									tab={tab}
									isActive={isActive}
									onSwitch={switchTab}
									onClose={closeTab}
								/>
							);
						})}
					</div>
				</SortableContext>
			</LayoutGroup>
			<button
				onClick={() => createTab()}
				className="hover:bg-app-hover text-ink-dull hover:text-ink flex size-7 shrink-0 items-center justify-center rounded-full transition-colors"
				title="New tab (⌘T)"
			>
				<Plus size={14} weight="bold" />
			</button>
		</div>
	);
}
