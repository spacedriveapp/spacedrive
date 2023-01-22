import { ReactComponent as Ellipsis } from '@sd/assets/svgs/ellipsis.svg';
import {
	DndContext,
	DragEndEvent,
	DragOverlay,
	DragStartEvent,
	KeyboardSensor,
	PointerSensor,
	UniqueIdentifier,
	closestCenter,
	useSensor,
	useSensors
} from '@dnd-kit/core';
import {
	SortableContext,
	arrayMove,
	sortableKeyboardCoordinates,
	useSortable,
	verticalListSortingStrategy
} from '@dnd-kit/sortable';
import clsx from 'clsx';
import { CheckCircle, CirclesFour, Gear, Lock, Planet, Plus } from 'phosphor-react';
import React, { PropsWithChildren, useMemo, useState } from 'react';
import { NavLink, NavLinkProps } from 'react-router-dom';
import {
	LocationCreateArgs,
	Tag as TagType,
	getDebugState,
	useBridgeQuery,
	useCurrentLibrary,
	useDebugState,
	useLibraryMutation,
	useLibraryQuery
} from '@sd/client';
import {
	Button,
	ButtonLink,
	CategoryHeading,
	Dropdown,
	Loader,
	OverlayPanel,
	Select,
	SelectOption,
	Switch,
	cva,
	dialogManager,
	tw
} from '@sd/ui';
import { useOperatingSystem } from '~/hooks/useOperatingSystem';
import { usePlatform } from '~/util/Platform';
import AddLocationDialog from '../dialog/AddLocationDialog';
import CreateLibraryDialog from '../dialog/CreateLibraryDialog';
import { Folder } from '../icons/Folder';
import { JobsManager } from '../jobs/JobManager';
import { MacTrafficLights } from '../os/TrafficLights';
import { InputContainer } from '../primitive/InputContainer';
import { Tooltip } from '../tooltip/Tooltip';

const SidebarBody = tw.div`flex relative flex-col flex-grow-0 flex-shrink-0 w-44 min-h-full border-r border-sidebar-divider bg-sidebar`;

const SidebarContents = tw.div`flex flex-col px-2.5 flex-grow pt-1 pb-10 overflow-x-hidden overflow-y-scroll no-scrollbar mask-fade-out`;

const SidebarFooter = tw.div`flex flex-col mb-3 px-2.5`;

export function Sidebar() {
	// DO NOT DO LIBRARY QUERIES OR MUTATIONS HERE. This is rendered before a library is set.
	const os = useOperatingSystem();
	const { library, libraries, isLoading: isLoadingLibraries, switchLibrary } = useCurrentLibrary();
	const debugState = useDebugState();

	return (
		<SidebarBody className={macOnly(os, 'bg-opacity-[0.75]')}>
			<WindowControls />
			<Dropdown.Root
				className="mt-2 mx-2.5"
				// we override the sidebar dropdown item's hover styles
				// because the dark style clashes with the sidebar
				itemsClassName="dark:bg-sidebar-box dark:border-sidebar-line mt-1 dark:divide-menu-selected/30 shadow-none"
				button={
					<Dropdown.Button
						variant="gray"
						className={clsx(
							`w-full text-ink `,
							// these classname overrides are messy
							// but they work
							`!bg-sidebar-box !border-sidebar-line/50 active:!border-sidebar-line active:!bg-sidebar-button ui-open:!bg-sidebar-button ui-open:!border-sidebar-line ring-offset-sidebar`,
							(library === null || isLoadingLibraries) && '!text-ink-faint'
						)}
					>
						<span className="truncate">
							{isLoadingLibraries ? 'Loading...' : library ? library.config.name : ' '}
						</span>
					</Dropdown.Button>
				}
			>
				<Dropdown.Section>
					{libraries?.map((lib) => (
						<Dropdown.Item
							selected={lib.uuid === library?.uuid}
							key={lib.uuid}
							onClick={() => switchLibrary(lib.uuid)}
						>
							{lib.config.name}
						</Dropdown.Item>
					))}
				</Dropdown.Section>
				<Dropdown.Section>
					<Dropdown.Item
						icon={Plus}
						onClick={() => {
							dialogManager.create((dp) => <CreateLibraryDialog {...dp} />);
						}}
					>
						New Library
					</Dropdown.Item>
					<Dropdown.Item icon={Gear} to="settings/library">
						Manage Library
					</Dropdown.Item>
					<Dropdown.Item icon={Lock} onClick={() => alert('TODO: Not implemented yet!')}>
						Lock
					</Dropdown.Item>
				</Dropdown.Section>
			</Dropdown.Root>
			<SidebarContents>
				<div className="pt-1">
					<SidebarLink to="/overview">
						<Icon component={Planet} />
						Overview
					</SidebarLink>
					{/* <SidebarLink to="photos">
						<Icon component={ShareNetwork} />
						Nodes
					</SidebarLink> */}
					<SidebarLink to="content">
						<Icon component={CirclesFour} />
						Spaces
					</SidebarLink>
				</div>
				{library && <LibraryScopedSection />}
				<div className="flex-grow" />
			</SidebarContents>

			<SidebarFooter>
				<div className="flex">
					<ButtonLink
						to="/settings/general"
						size="icon"
						variant="outline"
						className="text-ink-faint ring-offset-sidebar"
					>
						<Tooltip label="Settings">
							<Gear className="w-5 h-5" />
						</Tooltip>
					</ButtonLink>
					<OverlayPanel
						transformOrigin="bottom left"
						disabled={!library}
						trigger={
							<Button
								size="icon"
								variant="outline"
								className="radix-state-open:bg-sidebar-selected/50 text-ink-faint ring-offset-sidebar"
							>
								{library && (
									<Tooltip label="Recent Jobs">
										<IsRunningJob />
									</Tooltip>
								)}
							</Button>
						}
					>
						<div className="block w-[430px] h-96">
							<JobsManager />
						</div>
					</OverlayPanel>
				</div>
				{debugState.enabled && <DebugPanel />}
			</SidebarFooter>
		</SidebarBody>
	);
}

function IsRunningJob() {
	const { data: isRunningJob } = useLibraryQuery(['jobs.isRunning']);

	return isRunningJob ? (
		<Loader className="w-[20px] h-[20px]" />
	) : (
		<CheckCircle className="w-5 h-5" />
	);
}

function DebugPanel() {
	const buildInfo = useBridgeQuery(['buildInfo']);
	const nodeState = useBridgeQuery(['nodeState']);
	const debugState = useDebugState();
	const platform = usePlatform();

	return (
		<OverlayPanel
			className="p-4 focus:outline-none"
			transformOrigin="bottom left"
			trigger={
				<h1 className="w-full ml-1 mt-1 text-[7pt] text-ink-faint/50">
					v{buildInfo.data?.version || '-.-.-'} - {buildInfo.data?.commit || 'dev'}
				</h1>
			}
		>
			<div className="block w-[430px] h-96">
				<InputContainer
					mini
					title="rspc Logger"
					description="Enable the logger link so you can see what's going on in the browser logs."
				>
					<Switch
						checked={debugState.rspcLogger}
						onClick={() => (getDebugState().rspcLogger = !debugState.rspcLogger)}
					/>
				</InputContainer>
				{platform.openPath && (
					<InputContainer
						mini
						title="Open Data Directory"
						description="Quickly get to your Spacedrive database"
					>
						<div className="mt-2">
							<Button
								size="sm"
								variant="gray"
								onClick={() => {
									if (nodeState?.data?.data_path) platform.openPath!(nodeState?.data?.data_path);
								}}
							>
								Open
							</Button>
						</div>
					</InputContainer>
				)}
				<InputContainer
					mini
					title="React Query Devtools"
					description="Configure the React Query devtools."
				>
					<Select
						value={debugState.reactQueryDevtools}
						size="sm"
						onChange={(value) => (getDebugState().reactQueryDevtools = value as any)}
					>
						<SelectOption value="disabled">Disabled</SelectOption>
						<SelectOption value="invisible">Invisible</SelectOption>
						<SelectOption value="enabled">Enabled</SelectOption>
					</Select>
				</InputContainer>

				{/* {platform.showDevtools && (
					<InputContainer
						mini
						title="Devtools"
						description="Allow opening browser devtools in a production build"
					>
						<div className="mt-2">
							<Button size="sm" variant="gray" onClick={platform.showDevtools}>
								Show
							</Button>
						</div>
					</InputContainer>
				)} */}
			</div>
		</OverlayPanel>
	);
}

const sidebarItemClass = cva(
	'max-w mb-[2px] rounded px-2 py-1 gap-0.5 flex flex-row flex-grow items-center font-medium truncate text-sm outline-none ring-offset-sidebar focus:ring-2 focus:ring-accent focus:ring-offset-2',
	{
		variants: {
			isActive: {
				true: 'bg-sidebar-selected/40 text-ink',
				false: 'text-ink-dull'
			},
			isTransparent: {
				true: 'bg-opacity-90',
				false: ''
			}
		}
	}
);

export const SidebarLink = (props: PropsWithChildren<NavLinkProps>) => {
	const os = useOperatingSystem();
	return (
		<NavLink
			{...props}
			className={({ isActive }) =>
				clsx(sidebarItemClass({ isActive, isTransparent: os === 'macOS' }), props.className)
			}
		>
			{props.children}
		</NavLink>
	);
};

const SidebarSection: React.FC<{
	name: string;
	actionArea?: React.ReactNode;
	children: React.ReactNode;
}> = (props) => {
	return (
		<div className="mt-5 group">
			<div className="flex items-center justify-between mb-1">
				<CategoryHeading className="ml-1">{props.name}</CategoryHeading>
				<div className="transition-all duration-300 opacity-0 text-ink-faint group-hover:opacity-30 hover:!opacity-100">
					{props.actionArea}
				</div>
			</div>
			{props.children}
		</div>
	);
};

const SidebarHeadingOptionsButton: React.FC<{ to: string; icon?: React.FC }> = (props) => {
	const Icon = props.icon ?? Ellipsis;
	return (
		<NavLink to={props.to}>
			<Button className="!p-[5px]" variant="outline">
				<Icon className="w-3 h-3" />
			</Button>
		</NavLink>
	);
};

function LibraryScopedSection() {
	const platform = usePlatform();

	const locations = useLibraryQuery(['locations.list'], { keepPreviousData: true });
	const createLocation = useLibraryMutation('locations.create');

	return (
		<>
			<div>
				<SidebarSection
					name="Locations"
					actionArea={
						<>
							{/* <SidebarHeadingOptionsButton to="/settings/locations" icon={CogIcon} /> */}
							<SidebarHeadingOptionsButton to="/settings/locations" />
						</>
					}
				>
					{locations.data?.map((location) => {
						return (
							<div key={location.id} className="flex flex-row items-center">
								<SidebarLink
									className="relative w-full group"
									to={{
										pathname: `location/${location.id}`
									}}
								>
									<div className="-mt-0.5 mr-1 flex-grow-0 flex-shrink-0">
										<Folder size={18} />
									</div>

									<span className="flex-grow flex-shrink-0">{location.name}</span>
								</SidebarLink>
							</div>
						);
					})}
					{(locations.data?.length || 0) < 4 && (
						<button
							onClick={() => {
								if (platform.platform === 'web') {
									dialogManager.create((dp) => <AddLocationDialog {...dp} />);
								} else {
									if (!platform.openDirectoryPickerDialog) {
										alert('Opening a dialogue is not supported on this platform!');
										return;
									}
									platform.openDirectoryPickerDialog().then((result) => {
										// TODO: Pass indexer rules ids to create location
										if (result)
											createLocation.mutate({
												path: result as string,
												indexer_rules_ids: []
											} as LocationCreateArgs);
									});
								}
							}}
							className={clsx(
								'w-full px-2 py-1 mt-1 text-xs font-medium text-center',
								'rounded border border-dashed border-sidebar-line hover:border-sidebar-selected',
								'cursor-normal transition text-ink-faint'
							)}
						>
							Add Location
						</button>
					)}
				</SidebarSection>
			</div>
			<Tags />
		</>
	);
}

const SortableItem = ({ id, children }: PropsWithChildren<{ id: UniqueIdentifier }>) => {
	const sortable = useSortable({ id });

	const getSortingDirection = () => (sortable.activeIndex > sortable.overIndex ? 'up' : 'down');

	return (
		<div
			ref={sortable.setNodeRef}
			{...sortable.attributes}
			{...sortable.listeners}
			className={clsx(
				'relative',
				sortable.isOver && sortable.activeIndex !== sortable.overIndex
					? getSortingDirection() === 'up'
						? 'before:bg-accent before:absolute before:-top-px before:h-px before:w-full'
						: 'after:bg-accent after:absolute after:-bottom-px after:h-px after:w-full'
					: null
			)}
		>
			{children}
		</div>
	);
};

const Tag = (props: { tag: TagType; isDragged?: boolean }) => {
	return (
		<SidebarLink
			to={`tag/${props.tag.id}`}
			className={sidebarItemClass({ isActive: props.isDragged })}
		>
			<div
				className="w-[12px] h-[12px] rounded-full"
				style={{ backgroundColor: props.tag.color || '#efefef' }}
			/>
			<span className="ml-1.5 text-sm">{props.tag.name}</span>
		</SidebarLink>
	);
};

const Tags = () => {
	const [activeSortableId, setActiveSortableId] = useState<number | null>(null);

	const tags = useLibraryQuery(['tags.list'], { keepPreviousData: true });
	const updatePosition = useLibraryMutation(['tags.updatePosition'], {
		onSuccess: () => tags.refetch()
	});

	const sensors = useSensors(
		useSensor(PointerSensor, {
			activationConstraint: {
				distance: 10
			}
		}),
		useSensor(KeyboardSensor, {
			coordinateGetter: sortableKeyboardCoordinates
		})
	);

	const sortableItems = useMemo(() => tags.data?.map((tag) => tag.id) || [], [tags.data]);
	const activeTag = useMemo(
		() => (activeSortableId ? tags.data?.find((tag) => tag.id === activeSortableId) : undefined),
		[activeSortableId]
	);

	function handleDragStart(event: DragStartEvent) {
		setActiveSortableId(event.active.id as number);
	}

	function handleDragEnd(event: DragEndEvent) {
		const { active, over } = event;

		if (tags.data && over && active.id !== over.id) {
			const oldIndex = sortableItems.indexOf(active.id as number);
			const newIndex = sortableItems.indexOf(over.id as number);

			const moved = arrayMove(tags.data, oldIndex, newIndex);

			const newOrder = moved
				.map((group, i) => ({ id: group.id, position: i }))
				.slice(
					newIndex > oldIndex ? oldIndex : newIndex,
					newIndex > oldIndex ? newIndex + 1 : oldIndex + 1
				);

			updatePosition.mutate(newOrder);
		}

		setActiveSortableId(null);
	}

	return (
		<div>
			{!!tags.data?.length && (
				<SidebarSection
					name="Tags"
					actionArea={<SidebarHeadingOptionsButton to="/settings/tags" />}
				>
					<div className="mt-1 mb-2">
						<DndContext
							sensors={sensors}
							collisionDetection={closestCenter}
							onDragStart={handleDragStart}
							onDragEnd={handleDragEnd}
						>
							<SortableContext items={sortableItems} strategy={verticalListSortingStrategy}>
								{tags.data?.map((tag) => (
									<SortableItem key={tag.id} id={tag.id}>
										<Tag tag={tag} />
									</SortableItem>
								))}
							</SortableContext>
							<DragOverlay dropAnimation={null}>
								{activeTag && <Tag tag={activeTag} isDragged />}
							</DragOverlay>
						</DndContext>
					</div>
				</SidebarSection>
			)}
		</div>
	);
};

const Icon = ({ component: Icon, ...props }: any) => (
	<Icon weight="bold" {...props} className={clsx('w-4 h-4 mr-2', props.className)} />
);

// cute little helper to decrease code clutter
const macOnly = (platform: string | undefined, classnames: string) =>
	platform === 'macOS' ? classnames : '';

function WindowControls() {
	const { platform } = usePlatform();

	const showControls = window.location.search.includes('showControls');
	if (platform === 'tauri' || showControls) {
		return (
			<div data-tauri-drag-region className="flex-shrink-0 h-7">
				{/* We do not provide the onClick handlers for 'MacTrafficLights' because this is only used in demo mode */}
				{showControls && <MacTrafficLights className="z-50 absolute top-[13px] left-[13px]" />}
			</div>
		);
	}

	return null;
}
