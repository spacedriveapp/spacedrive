import {useDndContext, useDroppable} from '@dnd-kit/core';
import {
	SortableContext,
	useSortable,
	verticalListSortingStrategy
} from '@dnd-kit/sortable';
import {CSS} from '@dnd-kit/utilities';
import {
	ArrowsClockwise,
	ArrowsOut,
	CircleNotch,
	FunnelSimple,
	GearSix,
	ListBullets,
	Palette
} from '@phosphor-icons/react';
import {useLibraryMutation, useSidebarStore} from '@sd/ts-client';
import type {
	SpaceGroup as SpaceGroupType,
	SpaceItem as SpaceItemType
} from '@sd/ts-client';
import {CircleButton, Popover, usePopover} from '@spaceui/primitives';
import clsx from 'clsx';
import {motion} from 'framer-motion';
import {memo, useEffect, useState} from 'react';
import {useNavigate} from 'react-router-dom';
import {usePlatform} from '../../contexts/PlatformContext';
import {useSpacedriveClient} from '../../contexts/SpacedriveContext';
import {useLibraries} from '../../hooks/useLibraries';
import {JobList} from '../JobManager/components/JobList';
import {useJobsContext} from '../JobManager/hooks/JobsContext';
import {CARD_HEIGHT} from '../JobManager/types';
import {ActivityFeed} from '../SyncMonitor/components/ActivityFeed';
import {PeerList} from '../SyncMonitor/components/PeerList';
import {useSyncCount} from '../SyncMonitor/hooks/useSyncCount';
import {useSyncMonitor} from '../SyncMonitor/hooks/useSyncMonitor';
import {AddGroupButton} from './AddGroupButton';
import {useSpaceLayout, useSpaces} from './hooks/useSpaces';
import {SpaceCustomizationPanel} from './SpaceCustomizationPanel';
import {SpaceGroup} from './SpaceGroup';
import {SpaceItem} from './SpaceItem';
import {SpaceSwitcher} from './SpaceSwitcher';

// Wrapper that adds a space-level drop zone before each group and makes it sortable
function SpaceGroupWithDropZone({
	group,
	items,
	spaceId,
	isFirst
}: {
	group: SpaceGroupType;
	items: SpaceItemType[];
	spaceId?: string;
	isFirst: boolean;
}) {
	const {active} = useDndContext();

	// Disable drop zone when dragging groups or space items (they have 'label' in their data)
	// This allows sortable collision detection to work for reordering
	const isDraggingSortableItem = active?.data?.current?.label != null;

	const {setNodeRef: setDropRef, isOver} = useDroppable({
		id: `space-root-before-${group.id}`,
		disabled: !spaceId || isDraggingSortableItem,
		data: {
			action: 'add-to-space',
			spaceId,
			groupId: null
		}
	});

	// Sortable for group reordering
	const {
		attributes,
		listeners,
		setNodeRef: setSortableRef,
		transform,
		transition,
		isDragging,
		setActivatorNodeRef
	} = useSortable({
		id: group.id,
		data: {
			label: group.name
		}
	});

	const style = {
		transform: CSS.Transform.toString(transform),
		transition
	};

	return (
		<div
			ref={setSortableRef}
			style={style}
			className={clsx('relative', isDragging && 'z-50 opacity-50')}
		>
			{/* Drop zone before this group (for adding root-level items) */}
			<div
				ref={setDropRef}
				className="absolute -top-2.5 left-0 right-0 z-10 h-5"
			>
				{isOver && !isDragging && !isDraggingSortableItem && (
					<div className="bg-accent absolute left-2 right-2 top-1/2 h-[2px] -translate-y-1/2 rounded-full" />
				)}
			</div>
			<SpaceGroup
				group={group}
				items={items}
				spaceId={spaceId}
				sortableAttributes={attributes}
				sortableListeners={listeners}
			/>
		</div>
	);
}

// Sync Monitor Button with Popover
const SyncButton = memo(function SyncButton() {
	const popover = usePopover();
	const navigate = useNavigate();
	const [showActivityFeed, setShowActivityFeed] = useState(false);
	const {onlinePeerCount, isSyncing} = useSyncCount();
	const sync = useSyncMonitor();

	useEffect(() => {
		if (popover.open) {
			setShowActivityFeed(false);
		}
	}, [popover.open]);

	const getStateColor = (state: string) => {
		switch (state) {
			case 'Ready':
				return 'bg-green-500';
			case 'Backfilling':
				return 'bg-yellow-500';
			case 'CatchingUp':
				return 'bg-accent';
			case 'Uninitialized':
				return 'bg-ink-faint';
			case 'Paused':
				return 'bg-ink-dull';
			default:
				return 'bg-ink-faint';
		}
	};

	return (
		<Popover.Root open={popover.open} onOpenChange={popover.setOpen}>
			<Popover.Trigger asChild>
				<CircleButton
					icon={({className, ...props}) =>
						isSyncing ? (
							<CircleNotch
								className={clsx(className, 'animate-spin')}
								{...props}
							/>
						) : (
							<ArrowsClockwise className={className} {...props} />
						)
					}
					title="Sync Monitor"
				/>
			</Popover.Trigger>
			<Popover.Content
				side="top"
				align="start"
				sideOffset={8}
				className="!bg-app z-50 max-h-[520px] w-[380px] !rounded-xl !p-0"
			>
				<div className="border-app-line flex items-center justify-between border-b px-4 py-3">
					<h3 className="text-ink text-sm font-semibold">
						Sync Monitor
					</h3>

					<div className="flex items-center gap-2">
						{onlinePeerCount > 0 && (
							<span className="text-ink-dull text-xs">
								{onlinePeerCount}{' '}
								{onlinePeerCount === 1 ? 'peer' : 'peers'}{' '}
								online
							</span>
						)}

						<CircleButton
							icon={ArrowsOut}
							onClick={() => navigate('/sync')}
							title="Open full sync monitor"
						/>

						<CircleButton
							icon={FunnelSimple}
							active={showActivityFeed}
							onClick={() =>
								setShowActivityFeed(!showActivityFeed)
							}
							title={
								showActivityFeed
									? 'Show peers'
									: 'Show activity feed'
							}
						/>
					</div>
				</div>

				{popover.open && (
					<>
						<div className="border-app-line bg-app-box/50 border-b px-4 py-2">
							<div className="flex items-center gap-2">
								<div
									className={`size-2 rounded-full ${getStateColor(sync.currentState)}`}
								/>
								<span className="text-ink-dull text-xs font-medium">
									{sync.currentState}
								</span>
							</div>
						</div>
						<motion.div
							className="no-scrollbar overflow-y-auto"
							initial={false}
							animate={{
								height: showActivityFeed
									? Math.min(
											sync.recentActivity.length * 40 +
												16,
											400
										)
									: Math.min(sync.peers.length * 80 + 16, 400)
							}}
							transition={{
								duration: 0.2,
								ease: [0.25, 1, 0.5, 1]
							}}
						>
							{showActivityFeed ? (
								<ActivityFeed
									activities={sync.recentActivity}
								/>
							) : (
								<PeerList
									peers={sync.peers}
									currentState={sync.currentState}
								/>
							)}
						</motion.div>
					</>
				)}
			</Popover.Content>
		</Popover.Root>
	);
});

// Jobs Button with Popover
const JobsButton = memo(
	function JobsButton({
		activeJobCount,
		hasRunningJobs,
		jobs,
		pause,
		resume,
		cancel,
		getSpeedHistory,
		navigate
	}: {
		activeJobCount: number;
		hasRunningJobs: boolean;
		jobs: any[];
		pause: (jobId: string) => Promise<void>;
		resume: (jobId: string) => Promise<void>;
		cancel: (jobId: string) => Promise<void>;
		getSpeedHistory: (jobId: string) => any[];
		navigate: any;
	}) {
		const popover = usePopover();
		const [showOnlyRunning, setShowOnlyRunning] = useState(true);

		useEffect(() => {
			if (popover.open) {
				setShowOnlyRunning(true);
			}
		}, [popover.open]);

		const filteredJobs = showOnlyRunning
			? jobs.filter(
					(job) => job.status === 'running' || job.status === 'paused'
				)
			: jobs;

		return (
			<Popover.Root open={popover.open} onOpenChange={popover.setOpen}>
				<Popover.Trigger asChild>
					<CircleButton
						icon={({className, ...props}) =>
							hasRunningJobs ? (
								<CircleNotch
									className={clsx(className, 'animate-spin')}
									{...props}
								/>
							) : (
								<ListBullets className={className} {...props} />
							)
						}
						title="Job Manager"
					/>
				</Popover.Trigger>
				<Popover.Content
					side="top"
					align="start"
					sideOffset={8}
					className="!bg-app z-50 max-h-[480px] w-[360px] !rounded-xl !p-0"
				>
					<div className="border-app-line flex items-center justify-between border-b px-4 py-3">
						<h3 className="text-ink text-sm font-semibold">
							Job Manager
						</h3>

						<div className="flex items-center gap-2">
							{activeJobCount > 0 && (
								<span className="text-ink-dull text-xs">
									{activeJobCount} active
								</span>
							)}

							<CircleButton
								icon={ArrowsOut}
								onClick={() => navigate('/jobs')}
								title="Open full jobs screen"
							/>

							<CircleButton
								icon={FunnelSimple}
								active={showOnlyRunning}
								onClick={() =>
									setShowOnlyRunning(!showOnlyRunning)
								}
								title={
									showOnlyRunning
										? 'Show all jobs'
										: 'Show only active jobs'
								}
							/>
						</div>
					</div>

					{popover.open && (
						<motion.div
							className="no-scrollbar overflow-y-auto"
							initial={false}
							animate={{
								height:
									filteredJobs.length === 0
										? CARD_HEIGHT + 16
										: Math.min(
												filteredJobs.length *
													(CARD_HEIGHT + 8) +
													16,
												400
											)
							}}
							transition={{
								duration: 0.2,
								ease: [0.25, 1, 0.5, 1]
							}}
						>
							<JobList
								jobs={filteredJobs}
								onPause={pause}
								onResume={resume}
								onCancel={cancel}
								getSpeedHistory={getSpeedHistory}
							/>
						</motion.div>
					)}
				</Popover.Content>
			</Popover.Root>
		);
	},
	(prevProps, nextProps) => {
		// Only re-render if these specific values change
		return (
			prevProps.activeJobCount === nextProps.activeJobCount &&
			prevProps.hasRunningJobs === nextProps.hasRunningJobs
		);
	}
);

interface SpacesSidebarProps {
	isPreviewActive?: boolean;
}

export function SpacesSidebar({isPreviewActive = false}: SpacesSidebarProps) {
	const client = useSpacedriveClient();
	const platform = usePlatform();
	const navigate = useNavigate();
	const {data: libraries} = useLibraries();
	const [currentLibraryId, setCurrentLibraryId] = useState<string | null>(
		() => client.getCurrentLibraryId()
	);
	const [customizePanelOpen, setCustomizePanelOpen] = useState(false);

	// Get sync and job status for icons
	const {onlinePeerCount, isSyncing} = useSyncCount();
	const {
		activeJobCount,
		hasRunningJobs,
		jobs,
		pause,
		resume,
		cancel,
		getSpeedHistory
	} = useJobsContext();

	const {currentSpaceId, setCurrentSpace} = useSidebarStore();
	const {data: spacesData} = useSpaces();
	const spaces = spacesData?.spaces;

	// Listen for library changes from client and update local state
	useEffect(() => {
		const handleLibraryChange = (newLibraryId: string) => {
			setCurrentLibraryId(newLibraryId);
		};

		client.on('library-changed', handleLibraryChange);
		return () => {
			client.off('library-changed', handleLibraryChange);
		};
	}, [client]);

	// Auto-select first library on mount if none selected
	useEffect(() => {
		if (libraries && libraries.length > 0 && !currentLibraryId) {
			const firstLib = libraries[0];

			// Set library ID via platform (syncs to all windows on Tauri)
			if (platform.setCurrentLibraryId) {
				platform
					.setCurrentLibraryId(firstLib.id)
					.catch((err) =>
						console.error('Failed to set library ID:', err)
					);
			} else {
				// Web fallback - just update client
				client.setCurrentLibrary(firstLib.id);
			}
		}
	}, [libraries, currentLibraryId, client, platform]);

	// Auto-select first space if none selected
	const currentSpace =
		spaces?.find((s) => s.id === currentSpaceId) ?? spaces?.[0];

	useEffect(() => {
		if (currentSpace && currentSpace.id !== currentSpaceId) {
			setCurrentSpace(currentSpace.id);
		}
	}, [currentSpace, currentSpaceId, setCurrentSpace]);

	const {data: layout} = useSpaceLayout(currentSpace?.id ?? null);

	const addItem = useLibraryMutation('spaces.add_item');

	return (
		<div className="flex h-full w-[220px] min-w-[176px] max-w-[300px] flex-col bg-transparent p-2">
			<div
				className={clsx(
					'flex h-full flex-col overflow-hidden rounded-2xl',
					isPreviewActive
						? 'bg-sidebar/80 backdrop-blur-2xl'
						: 'bg-sidebar/65'
				)}
			>
				<nav className="relative z-[51] flex h-full flex-col gap-2.5 p-2.5 pb-2 pt-[43px]">
					{/* Space Switcher */}
					<SpaceSwitcher
						spaces={spaces}
						currentSpace={currentSpace}
						onSwitch={setCurrentSpace}
					/>

					{/* Scrollable Content */}
					<div className="no-scrollbar mask-fade-out mt-3 flex grow flex-col space-y-5 overflow-x-hidden overflow-y-scroll pb-10">
						{/* Space-level items (pinned shortcuts) */}
						{layout?.space_items &&
							layout.space_items.length > 0 && (
								<SortableContext
									items={layout.space_items.map(
										(item) => item.id
									)}
									strategy={verticalListSortingStrategy}
								>
									<div className="space-y-0.5">
										{layout.space_items.map(
											(item, index) => (
												<SpaceItem
													key={item.id}
													item={item}
													isLastItem={
														index ===
														layout.space_items
															.length -
															1
													}
													allowInsertion={true}
													spaceId={currentSpace?.id}
													groupId={null}
													sortable={true}
												/>
											)
										)}
									</div>
								</SortableContext>
							)}

						{/* Groups with space-level drop zones between them */}
						{layout?.groups && (
							<SortableContext
								items={layout.groups.map(({group}) => group.id)}
								strategy={verticalListSortingStrategy}
							>
								{layout.groups.map(({group, items}, index) => (
									<SpaceGroupWithDropZone
										key={group.id}
										group={group}
										items={items}
										spaceId={currentSpace?.id}
										isFirst={index === 0}
									/>
								))}
							</SortableContext>
						)}

						{/* Add Group Button */}
						{currentSpace && (
							<AddGroupButton spaceId={currentSpace.id} />
						)}
					</div>

					{/* Sync Monitor, Job Manager, Customize & Settings (pinned to bottom) */}
					<div className="flex items-center justify-between">
						<div className="flex items-center gap-2">
							<SyncButton />
							<JobsButton
								activeJobCount={activeJobCount}
								hasRunningJobs={hasRunningJobs}
								jobs={jobs}
								pause={pause}
								resume={resume}
								cancel={cancel}
								getSpeedHistory={getSpeedHistory}
								navigate={navigate}
							/>
							<CircleButton
								icon={Palette}
								title="Customize"
								onClick={() => setCustomizePanelOpen(true)}
							/>
						</div>
						<CircleButton
							icon={GearSix}
							title="Settings"
							onClick={() => {
								if (platform.showWindow) {
									platform
										.showWindow({
											type: 'Settings',
											page: 'general'
										})
										.catch((err) =>
											console.error(
												'Failed to open settings:',
												err
											)
										);
								}
							}}
						/>
					</div>
				</nav>
			</div>

			{/* Customization Panel */}
			<SpaceCustomizationPanel
				isOpen={customizePanelOpen}
				onClose={() => setCustomizePanelOpen(false)}
				spaceId={currentSpace?.id ?? null}
			/>
		</div>
	);
}
