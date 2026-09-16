import {
	Camera,
	CircleNotch,
	Clock,
	Database,
	GearSix,
	HardDrive,
	HardDrives,
	House,
	Images,
	ListBullets,
	ShieldCheck
} from '@phosphor-icons/react';
import FolderIcon from '@sd/assets/icons/Folder.webp';
import HomeIcon from '@sd/assets/icons/Home.webp';
import type {
	Device,
	Location,
	LocationsListOutput,
	LocationsListQueryInput,
	SourceInfo,
	SpaceGroup,
	SpaceItem as SpaceItemType
} from '@sd/ts-client';
import {useSidebarStore} from '@sd/ts-client';
import clsx from 'clsx';
import {useEffect, useMemo, useState} from 'react';
import {useNavigate} from 'react-router-dom';
import {usePlatform} from '../../contexts/PlatformContext';
import {
	useLibraryMutation,
	useLibraryQuery,
	useNormalizedQuery,
	useSpacedriveClient
} from '../../contexts/SpacedriveContext';
import {useLibraries} from '../../hooks/useLibraries';
import {
	getSpaceItemKeyFromRoute,
	useExplorer
} from '../../routes/explorer/context';
import {Thumb} from '../../routes/explorer/File/Thumb';
import {useJobsContext} from '../JobManager/hooks/JobsContext';
import {PairingModal} from '../modals/PairingModal';
import {useSyncCount} from '../SyncMonitor/hooks/useSyncCount';
import {resolveItemMetadata} from './hooks/spaceItemUtils';
import {useSpaceItemContextMenu} from './hooks/useSpaceItemContextMenu';
import {useSpaceLayout, useSpaces} from './hooks/useSpaces';
import {LibrarySwitcher} from './LibrarySwitcher';
import {SidebarItem} from './SidebarItem';
import {SidebarSection} from './SidebarSection';

interface SpacesSidebarProps {
	isPreviewActive?: boolean;
}

function routeForPath(path: Location['sd_path']): string {
	return `/explorer?path=${encodeURIComponent(JSON.stringify(path))}`;
}

function sourceRoute(
	source: SourceInfo,
	deviceSlug: string | undefined,
	deviceSlugById?: Record<string, string>
): string {
	// A replica's root lives on its owning device; navigating with that
	// device's slug is what routes the listing to the replicated index.
	const slug = source.device_id
		? deviceSlugById?.[source.device_id]
		: deviceSlug;
	if (source.data_type === 'filesystem' && source.root && slug) {
		return routeForPath({
			Physical: {device_slug: slug, path: source.root}
		});
	}

	return `/sources/${source.id}`;
}

function isSeededDestination(item: SpaceItemType): boolean {
	const type = item.item_type;
	if (typeof type === 'string') {
		return [
			'Overview',
			'Recents',
			'Favorites',
			'FileKinds',
			'Sources',
			'Redundancy',
			'Analyzer'
		].includes(type);
	}

	return 'Collection' in type && type.Collection.slug === 'screenshots';
}

function StoredItem({item, spaceId}: {item: SpaceItemType; spaceId: string}) {
	const {loadPreferencesForSpaceItem} = useExplorer();
	const {icon, label, path} = resolveItemMetadata(item);
	const contextMenu = useSpaceItemContextMenu({item, path, spaceId});
	const iconNode = item.resolved_file ? (
		<Thumb file={item.resolved_file} size={16} className="shrink-0" />
	) : icon.type === 'image' ? (
		<img src={icon.icon} alt="" className="size-4" />
	) : undefined;

	return (
		<SidebarItem
			id={item.id}
			label={label}
			icon={icon.type === 'component' ? icon.icon : undefined}
			iconNode={iconNode}
			href={path ?? undefined}
			disabled={!path}
			onSelect={() => {
				if (!path) return;
				const [pathname, query] = path.split('?');
				loadPreferencesForSpaceItem(
					getSpaceItemKeyFromRoute(
						pathname,
						query === undefined ? '' : `?${query}`
					)
				);
			}}
			onContextMenu={(event) => contextMenu.show(event)}
		/>
	);
}

function StoredGroup({
	group,
	items,
	spaceId
}: {
	group: SpaceGroup;
	items: SpaceItemType[];
	spaceId: string;
}) {
	const [collapsed, setCollapsed] = useState(group.is_collapsed);
	const updateGroup = useLibraryMutation('spaces.update_group');

	useEffect(() => setCollapsed(group.is_collapsed), [group.is_collapsed]);

	const toggle = () => {
		const next = !collapsed;
		setCollapsed(next);
		updateGroup
			.mutateAsync({group_id: group.id, name: null, is_collapsed: next})
			.catch(() => setCollapsed(!next));
	};

	return (
		<SidebarSection
			title={group.name}
			collapsed={collapsed}
			onToggle={toggle}
		>
			{items.map((item) => (
				<StoredItem key={item.id} item={item} spaceId={spaceId} />
			))}
		</SidebarSection>
	);
}

export function SpacesSidebar({isPreviewActive = false}: SpacesSidebarProps) {
	const client = useSpacedriveClient();
	const platform = usePlatform();
	const navigate = useNavigate();
	const {loadPreferencesForSpaceItem} = useExplorer();
	const {data: libraries} = useLibraries();
	const [currentLibraryId, setCurrentLibraryId] = useState<string | null>(
		() => client.getCurrentLibraryId()
	);
	const [isPairingOpen, setIsPairingOpen] = useState(false);
	const {activeJobCount, hasRunningJobs} = useJobsContext();
	const {isSyncing} = useSyncCount();
	const {currentSpaceId, setCurrentSpace} = useSidebarStore();
	const {data: spacesData} = useSpaces();
	const spaces = spacesData?.spaces;
	const currentSpace =
		spaces?.find((space) => space.id === currentSpaceId) ?? spaces?.[0];
	const {data: layout} = useSpaceLayout(currentSpace?.id ?? null);

	const {data: locationsData} = useNormalizedQuery<
		LocationsListQueryInput,
		LocationsListOutput
	>({
		query: 'locations.list',
		input: null,
		resourceType: 'location'
	});
	const locations = locationsData?.locations ?? [];
	const {data: suggestedLocationsData} = useLibraryQuery({
		type: 'locations.suggested',
		input: null
	});
	const systemFolders = suggestedLocationsData?.locations ?? [];

	const {data: sourcesData} = useLibraryQuery({
		type: 'sources.list',
		input: {data_type: null}
	});
	const sources: SourceInfo[] = sourcesData ? [...sourcesData] : [];

	const {data: devicesData} = useNormalizedQuery<
		{include_offline: boolean; include_details: boolean; show_paired: boolean},
		Device[]
	>({
		query: 'devices.list',
		// Paired devices come from the network registry, and replica source
		// rows need their owning device's slug to route into the explorer.
		input: {include_offline: true, include_details: false, show_paired: true},
		resourceType: 'device'
	});
	const deviceSlug = devicesData?.find((device) => device.is_current)?.slug;
	const deviceSlugById = Object.fromEntries(
		(devicesData ?? []).map((device) => [device.id, device.slug])
	);

	useEffect(() => {
		const handleLibraryChange = (libraryId: string) =>
			setCurrentLibraryId(libraryId);
		client.on('library-changed', handleLibraryChange);
		return () => client.off('library-changed', handleLibraryChange);
	}, [client]);

	useEffect(() => {
		if (libraries && libraries.length > 0 && !currentLibraryId) {
			const libraryId = libraries[0].id;
			if (platform.setCurrentLibraryId) {
				platform
					.setCurrentLibraryId(libraryId)
					.catch((error) =>
						console.error('Failed to select library:', error)
					);
			} else {
				client.setCurrentLibrary(libraryId);
			}
		}
	}, [client, currentLibraryId, libraries, platform]);

	useEffect(() => {
		if (currentSpace && currentSpace.id !== currentSpaceId) {
			setCurrentSpace(currentSpace.id);
		}
	}, [currentSpace, currentSpaceId, setCurrentSpace]);

	const switchLibrary = (libraryId: string) => {
		if (platform.setCurrentLibraryId) {
			platform
				.setCurrentLibraryId(libraryId)
				.catch((error) =>
					console.error('Failed to switch library:', error)
				);
		} else {
			client.setCurrentLibrary(libraryId);
		}
	};

	const openSettings = (page: 'general' | 'library') => {
		platform
			.showWindow?.({type: 'Settings', page})
			.catch((error) => console.error('Failed to open settings:', error));
	};

	const occupiedPlaceRoutes = useMemo(
		() =>
			new Set(
				[
					...locations.map((location) => location.sd_path),
					...systemFolders.map((folder) => folder.sd_path)
				].map((path) => routeForPath(path))
			),
		[locations, systemFolders]
	);
	const visibleSources = sources.filter(
		(source) =>
			!occupiedPlaceRoutes.has(
				sourceRoute(source, deviceSlug, deviceSlugById)
			)
	);
	const photoSource = sources.find((source) => source.data_type === 'photo');
	const otherSources = visibleSources.filter(
		(source) => source.id !== photoSource?.id
	);
	const customRootItems =
		layout?.space_items.filter((item) => !isSeededDestination(item)) ?? [];
	const customGroups =
		layout?.groups.filter(
			({group, items}) =>
				(group.group_type === 'Custom' ||
					group.group_type === 'QuickAccess') &&
				items.length > 0
		) ?? [];
	const hasPlaces =
		systemFolders.length > 0 ||
		locations.length > 0 ||
		otherSources.length > 0;
	const activityCount = activeJobCount > 0 ? activeJobCount : undefined;

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
				<nav
					aria-label="Library"
					className={clsx(
						'relative z-[51] flex h-full flex-col p-2.5 pb-2',
						platform.platform === 'tauri' && 'pt-[43px]'
					)}
				>
					<LibrarySwitcher
						libraries={libraries}
						currentLibraryId={currentLibraryId}
						spaces={spaces}
						currentSpace={currentSpace}
						onLibrarySwitch={switchLibrary}
						onSpaceSwitch={setCurrentSpace}
						onManageStorage={() => navigate('/sources')}
						onPairDevice={() => setIsPairingOpen(true)}
						onOpenSettings={() => openSettings('library')}
					/>

					<div className="no-scrollbar mask-fade-out mt-4 flex min-h-0 grow flex-col gap-4 overflow-y-auto overflow-x-hidden pb-8">
						<SidebarSection>
							<SidebarItem
								id="home"
								label="Home"
								icon={House}
								href="/"
							/>
							<SidebarItem
								id="recents"
								label="Recents"
								icon={Clock}
								href="/recents"
							/>
							{photoSource && (
								<SidebarItem
									id="photos"
									label="Photos"
									icon={Images}
									href={`/sources/${photoSource.id}`}
								/>
							)}
							<SidebarItem
								id="screenshots"
								label="Screenshots"
								icon={Camera}
								href="/collection/screenshots"
							/>
							<SidebarItem
								id="storage"
								label="Storage"
								icon={HardDrives}
								href="/sources"
								activePathPrefixes={[
									'/sources/adapters',
									'/analyzer'
								]}
							/>
							<SidebarItem
								id="protection"
								label="Protection"
								icon={ShieldCheck}
								href="/redundancy"
								activePathPrefixes={['/redundancy']}
							/>
						</SidebarSection>

						{hasPlaces && (
							<SidebarSection title="Places">
								{systemFolders.map((folder) => {
									const href = routeForPath(folder.sd_path);
									return (
										<SidebarItem
											key={href}
											id={`system-folder-${folder.name.toLowerCase()}`}
											label={folder.name}
											iconNode={
												<img
													src={
														folder.name === 'Home'
															? HomeIcon
															: FolderIcon
													}
													alt=""
													className="size-[18px] max-w-none"
												/>
											}
											href={href}
											title={folder.path}
											onSelect={() =>
												loadPreferencesForSpaceItem(
													getSpaceItemKeyFromRoute(
														href.split('?')[0],
														`?${href.split('?')[1]}`
													)
												)
											}
										/>
									);
								})}
								{locations.map((location) => {
									const href = routeForPath(location.sd_path);
									return (
										<SidebarItem
											key={location.id}
											id={`location-${location.id}`}
											label={location.name}
											iconNode={
												<img
													src={
														location.name.toLowerCase() ===
														'home'
															? HomeIcon
															: FolderIcon
													}
													alt=""
													className="size-[18px] max-w-none"
												/>
											}
											href={href}
											tone={
												location.is_available
													? 'default'
													: 'muted'
											}
											disabled={!location.is_available}
											badge={
												location.is_available
													? undefined
													: 'Offline'
											}
											title={location.name}
											onSelect={() => {
												const [pathname, query] =
													href.split('?');
												loadPreferencesForSpaceItem(
													getSpaceItemKeyFromRoute(
														pathname,
														query === undefined
															? ''
															: `?${query}`
													)
												);
											}}
										/>
									);
								})}
								{otherSources.map((source) => {
									const href = sourceRoute(
										source,
										deviceSlug,
										deviceSlugById
									);
									return (
										<SidebarItem
											key={source.id}
											id={`source-${source.id}`}
											label={source.name}
											icon={
												source.data_type ===
												'filesystem'
													? HardDrive
													: Database
											}
											href={href}
											tone={
												source.attached
													? 'default'
													: 'muted'
											}
											badge={
												source.attached
													? undefined
													: 'Offline'
											}
											badgeLabel={
												source.attached
													? undefined
													: `${source.name} is offline`
											}
											title={source.root ?? source.name}
											onSelect={() => {
												const [pathname, query] =
													href.split('?');
												loadPreferencesForSpaceItem(
													getSpaceItemKeyFromRoute(
														pathname,
														query === undefined
															? ''
															: `?${query}`
													)
												);
											}}
										/>
									);
								})}
							</SidebarSection>
						)}

						{currentSpace && customRootItems.length > 0 && (
							<SidebarSection title="Pinned">
								{customRootItems.map((item) => (
									<StoredItem
										key={item.id}
										item={item}
										spaceId={currentSpace.id}
									/>
								))}
							</SidebarSection>
						)}

						{currentSpace &&
							customGroups.map(({group, items}) => (
								<StoredGroup
									key={group.id}
									group={group}
									items={items}
									spaceId={currentSpace.id}
								/>
							))}
					</div>

					<div className="border-sidebar-line/50 space-y-0.5 border-t pt-2">
						<SidebarItem
							id="activity"
							label="Activity"
							iconNode={
								hasRunningJobs || isSyncing ? (
									<CircleNotch
										size={16}
										weight="bold"
										className="animate-spin"
									/>
								) : (
									<ListBullets size={16} weight="bold" />
								)
							}
							href="/activity"
							activePaths={['/activity', '/jobs']}
							badge={activityCount}
							badgeLabel={
								activityCount === undefined
									? undefined
									: `${activityCount} active jobs`
							}
						/>
						<SidebarItem
							id="settings"
							label="Settings"
							icon={GearSix}
							onSelect={() => openSettings('general')}
						/>
					</div>
				</nav>
			</div>

			<PairingModal
				isOpen={isPairingOpen}
				onClose={() => setIsPairingOpen(false)}
			/>
		</div>
	);
}
