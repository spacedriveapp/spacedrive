import {
	ArrowClockwise,
	CaretRight,
	CircleDashed,
	Database,
	Eye,
	HardDrive,
	House,
	Info,
	Plus,
	PushPin,
	SpinnerGap,
	Stack,
	WarningCircle
} from '@phosphor-icons/react';
import type {Icon as PhosphorIcon} from '@phosphor-icons/react';
import DatabaseIcon from '@sd/assets/icons/Database.webp';
import FolderIcon from '@sd/assets/icons/Folder.webp';
import HDDIcon from '@sd/assets/icons/HDD.webp';
import LaptopIcon from '@sd/assets/icons/Laptop.webp';
import type {Device, SdPath} from '@sd/ts-client';
import {
	getDeviceIcon,
	useLibraryMutation,
	useLibraryQuery,
	useSidebarStore
} from '@sd/ts-client';
import {
	CircleButton,
	Popover,
	Tooltip,
	usePopover
} from '@spacedrive/primitives';
import clsx from 'clsx';
import {motion} from 'framer-motion';
import {useEffect, useState} from 'react';
import {useNavigate} from 'react-router-dom';
import {isPathItem, useSpaceLayout, useSpaces} from '../../../components/SpacesSidebar/hooks';
import {useExplorer} from '../context';
import {sdPathToUri} from '../utils';
import {useAddStorageDialog} from './AddStorageModal';

interface PathBarProps {
	path: SdPath;
	devices: Map<string, Device>;
	onNavigate: (path: SdPath) => void;
}

interface PathSegment {
	name: string;
	path: SdPath;
}

function getCurrentDirectoryName(sdPath: SdPath): string {
	if ('Physical' in sdPath) {
		const parts = sdPath.Physical.path.split('/').filter(Boolean);
		return parts[parts.length - 1] || '/';
	}

	if ('Cloud' in sdPath) {
		const parts = sdPath.Cloud.path.split('/').filter(Boolean);
		return parts[parts.length - 1] || sdPath.Cloud.identifier;
	}

	if ('Content' in sdPath) {
		return 'Content';
	}

	return '';
}

function parsePathSegments(sdPath: SdPath): PathSegment[] {
	if ('Physical' in sdPath) {
		const {device_slug, path} = sdPath.Physical;
		const parts = path.split('/').filter(Boolean);

		return [
			{
				name: `/`,
				path: {
					Physical: {
						device_slug,
						path: '/'
					}
				}
			},
			...parts.map((part, index) => ({
				name: part,
				path: {
					Physical: {
						device_slug,
						path: '/' + parts.slice(0, index + 1).join('/')
					}
				}
			}))
		];
	}

	if ('Cloud' in sdPath) {
		const {service, identifier, path} = sdPath.Cloud;
		const parts = path.split('/').filter(Boolean);

		return [
			{
				name: identifier,
				path: {
					Cloud: {
						service,
						identifier,
						path: ''
					}
				}
			},
			...parts.map((part, index) => ({
				name: part,
				path: {
					Cloud: {
						service,
						identifier,
						path: parts.slice(0, index + 1).join('/')
					}
				}
			}))
		];
	}

	return [];
}

function statusLabel(value: string): string {
	return value
		.split('_')
		.map((part) => part.charAt(0).toUpperCase() + part.slice(1))
		.join(' ');
}

function StateChip({
	icon: Icon,
	detail,
	label,
	tone = 'neutral'
}: {
	icon: PhosphorIcon;
	detail: string;
	label: string;
	tone?: 'neutral' | 'active' | 'warning' | 'muted';
}) {
	const content = (
		<div
			className={clsx(
				'border-app-line flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[10px] font-medium',
				tone === 'active' && 'text-accent',
				tone === 'warning' && 'text-amber-400',
				tone === 'muted' && 'text-ink-faint',
				tone === 'neutral' && 'text-ink-dull'
			)}
		>
			<Icon size={12} weight={tone === 'active' ? 'fill' : 'regular'} />
			{label}
		</div>
	);

	return <Tooltip label={detail}>{content}</Tooltip>;
}

function PathStatusSkeleton() {
	return (
		<div className="animate-pulse px-4 py-4">
			<div className="p-3">
				<div className="bg-app-line/60 h-3 w-24 rounded" />
				<div className="bg-app-line/60 mt-3 h-12 rounded-lg" />
				<div className="mt-3 flex gap-2">
					<div className="bg-app-line/60 h-6 w-20 rounded-full" />
					<div className="bg-app-line/60 h-6 w-24 rounded-full" />
				</div>
			</div>
		</div>
	);
}

function PathStatusButton({path}: {path: SdPath}) {
	const popover = usePopover();
	const [showDetails, setShowDetails] = useState(false);
	const navigate = useNavigate();
	const {
		data: context,
		isLoading,
		isError,
		refetch
	} = useLibraryQuery(
		{
			type: 'paths.context',
			input: {path}
		},
		{
			// Index and watcher transitions are short-lived. Keep the explanation
			// current while somebody is looking at it without polling every tab.
			refetchInterval: popover.open ? 1000 : false
		}
	);
	const refresh = () => {
		void refetch();
	};
	const reindexSource = useLibraryMutation('sources.track', {
		onSuccess: refresh
	});

	// Pinning is navigation: a space-level Path item in the current space,
	// which the sidebar lists under Pinned. It never indexes anything.
	const {currentSpaceId} = useSidebarStore();
	const {data: spacesData} = useSpaces();
	const currentSpace =
		spacesData?.spaces.find((space) => space.id === currentSpaceId) ??
		spacesData?.spaces[0];
	const {data: layout} = useSpaceLayout(currentSpace?.id ?? null);
	const addSpaceItem = useLibraryMutation('spaces.add_item');
	const deleteSpaceItem = useLibraryMutation('spaces.delete_item');
	const pinnedItem = context
		? layout?.space_items.find(
				(item) =>
					isPathItem(item.item_type) &&
					sdPathToUri(item.item_type.Path.sd_path) ===
						sdPathToUri(context.canonical_path)
			)
		: undefined;

	const hasWarning =
		context?.availability === 'permission_denied' ||
		context?.availability === 'unavailable' ||
		(context?.source?.attached &&
			context.map_state !== 'indexing' &&
			context.watcher_state !== 'active');
	const isRemote =
		context?.availability === 'remote' ||
		context?.availability === 'missing';
	const Icon = isLoading
		? CircleDashed
		: context?.map_state === 'indexing'
			? SpinnerGap
			: hasWarning
				? WarningCircle
				: Stack;
	const pathLabel = getCurrentDirectoryName(path);
	const pathDetail =
		'Physical' in path ? path.Physical.path : sdPathToUri(path);
	const canPin = Boolean(
		currentSpace &&
		context &&
		!context.system_place &&
		context.availability === 'available'
	);
	const canAddSource = Boolean(
		context &&
		!context.source &&
		context.availability === 'available' &&
		'Physical' in path
	);
	const openStorage = () => {
		navigate('/sources');
		popover.setOpen(false);
	};
	const openSource = () => {
		if (context?.source) {
			navigate(`/sources/${context.source.id}`);
			popover.setOpen(false);
		}
	};
	const addSource = () => {
		if ('Physical' in path) {
			useAddStorageDialog(undefined, path.Physical.path);
			popover.setOpen(false);
		}
	};
	const watcherLabel =
		context?.watcher_state === 'active'
			? 'Watching'
			: context?.watcher_state === 'inactive'
				? 'Not watching'
				: 'Watch unavailable';
	const watcherTone =
		context?.watcher_state === 'active'
			? 'active'
			: context?.watcher_state === 'inactive'
				? 'warning'
				: 'muted';
	const watcherDetail =
		context?.watcher_state === 'active'
			? `Watching changes from ${context.watcher_root ?? pathDetail}.`
			: context?.watcher_state === 'inactive'
				? 'Changes under this path are not being watched.'
				: 'Live updates are unavailable for this path.';
	const storageLabel = context?.storage.source_record
		? 'Source records'
		: context?.storage.source_store && context.map_state === 'indexing'
			? 'Recording'
			: context?.storage.restart_cache && context.storage.memory
				? 'Restart cache'
				: context?.storage.memory
					? 'Memory only'
					: context?.storage.source_store
						? 'Not recorded'
						: 'Not indexed';
	const storageTone = context?.storage.source_record
		? 'active'
		: storageLabel === 'Not recorded'
			? 'warning'
			: context?.storage.memory
				? 'neutral'
				: 'muted';
	const storageDetail = context?.storage.source_record
		? `This path has committed records in ${context.source?.name ?? 'the source'} database.`
		: context?.storage.source_store && context.map_state === 'indexing'
			? 'This source is being walked. Records appear here as its batches commit.'
			: context?.storage.restart_cache && context.storage.memory
				? 'A rebuildable cache is available on this device after restart.'
				: context?.storage.memory
					? 'This path exists only in the live arena until the daemon stops.'
					: context?.storage.source_store
						? 'The source database exists, but this path has no committed record.'
						: 'Spacedrive has not indexed this path.';
	const placeLabel = context?.system_place
		? `${context.system_place} is a system Place`
		: pinnedItem
			? `Unpin ${pathLabel}`
			: canPin
				? 'Pin this folder'
				: 'This folder is unavailable';
	const canChangePlace = Boolean(pinnedItem || canPin);
	const sourceRoot = context?.source?.root;
	const canReindex = Boolean(
		sourceRoot &&
		context?.source?.attached &&
		context.availability === 'available'
	);

	return (
		<Popover.Root open={popover.open} onOpenChange={popover.setOpen}>
			<Popover.Trigger asChild>
				<CircleButton
					icon={Icon}
					className={clsx(
						context?.map_state === 'indexing' &&
							'[&_svg]:animate-spin',
						hasWarning && '!text-amber-400',
						isRemote && '!text-ink-faint',
						context?.source &&
							context.storage.source_store &&
							context.watcher_state === 'active' &&
							'!text-accent'
					)}
					title="Path status"
				/>
			</Popover.Trigger>
			<Popover.Content
				side="bottom"
				align="start"
				sideOffset={8}
				className="!bg-app-box z-50 w-[360px] !rounded-2xl !p-0"
			>
				<div>
					<div className="flex items-center gap-3 px-4 pb-3 pt-4">
						<img src={FolderIcon} className="size-10" alt="" />
						<div className="min-w-0 flex-1">
							<div className="text-ink truncate text-sm font-semibold">
								{pathLabel}
							</div>
							<div
								className="text-ink-faint mt-0.5 truncate text-[10px]"
								title={pathDetail}
							>
								{pathDetail}
							</div>
						</div>
					</div>

					{context?.source && !isError && (
						<div className="border-app-line flex items-center gap-3 border-t px-4 py-3">
							<img
								src={DatabaseIcon}
								className="size-10 object-contain"
								alt=""
							/>
							<div className="min-w-0 flex-1">
								<div className="text-ink truncate text-xs font-semibold">
									{context.source.name}
								</div>
								<div className="text-ink-faint text-[9px] font-semibold uppercase tracking-wider">
									Source
								</div>
								<div className="mt-2 flex flex-wrap gap-2">
									<StateChip
										icon={Eye}
										label={watcherLabel}
										tone={watcherTone}
										detail={watcherDetail}
									/>
									<StateChip
										icon={Database}
										label={storageLabel}
										tone={storageTone}
										detail={storageDetail}
									/>
								</div>
							</div>
						</div>
					)}
				</div>

				<div className="border-app-line border-t">
					{isError ? (
						<div className="px-4 py-4 text-xs">
							<div className="flex items-center gap-2 text-amber-400">
								<WarningCircle size={16} />
								Path status is unavailable
							</div>
							<button
								onClick={() => void refetch()}
								className="text-accent mt-1 font-medium hover:underline"
							>
								Try again
							</button>
						</div>
					) : isLoading || !context ? (
						<PathStatusSkeleton />
					) : (
						<>
							<div className="px-4 py-3">
								{context.availability !== 'available' && (
									<div className="mb-3 flex items-center gap-2 text-[11px] font-medium text-amber-400">
										<WarningCircle size={14} />
										{statusLabel(context.availability)}
									</div>
								)}

								{!context.source && (
									<div className="flex items-center gap-3 py-1">
										<Eye
											size={22}
											className="text-ink-dull shrink-0"
										/>
										<div>
											<div className="text-ink text-xs font-medium">
												{storageLabel}
											</div>
											<div className="text-ink-faint text-[10px]">
												Not kept by a source
											</div>
										</div>
									</div>
								)}

								{context.volume && (
									<div className="mt-2 flex items-center gap-3">
										<img
											src={HDDIcon}
											className="size-10 object-contain"
											alt=""
										/>
										<div className="min-w-0 flex-1">
											<div className="text-ink truncate text-xs font-medium">
												{context.volume.name}
											</div>
											<div className="text-ink-faint text-[9px] font-semibold uppercase tracking-wider">
												Volume
											</div>
										</div>
									</div>
								)}
							</div>

							<div className="border-app-line flex items-center gap-2 border-t px-4 py-3">
								<Tooltip
									label={placeLabel}
									asChild
									tooltipClassName="!z-[1000]"
								>
									<CircleButton
										icon={
											context.system_place
												? House
												: PushPin
										}
										size="lg"
										active={Boolean(
											context.system_place || pinnedItem
										)}
										title={placeLabel}
										aria-disabled={
											!context.system_place &&
											!canChangePlace
										}
										className={clsx(
											!context.system_place &&
												!canChangePlace &&
												'cursor-default opacity-40'
										)}
										onClick={() => {
											if (context.system_place) return;
											if (pinnedItem) {
												deleteSpaceItem.mutate({
													item_id: pinnedItem.id
												});
											} else if (canPin && currentSpace) {
												addSpaceItem.mutate({
													space_id: currentSpace.id,
													group_id: null,
													item_type: {
														Path: {
															sd_path:
																context.canonical_path
														}
													}
												});
											}
										}}
									/>
								</Tooltip>

								{context.source ? (
									<>
										<Tooltip
											label={
												canReindex
													? `Reindex ${context.source.name}`
													: 'Source is unavailable'
											}
											asChild
											tooltipClassName="!z-[1000]"
										>
											<CircleButton
												icon={ArrowClockwise}
												size="lg"
												active={
													context.map_state ===
													'indexing'
												}
												title={`Reindex ${context.source.name}`}
												aria-disabled={
													!canReindex ||
													reindexSource.isPending ||
													context.map_state ===
														'indexing'
												}
												className={clsx(
													(!canReindex ||
														reindexSource.isPending ||
														context.map_state ===
															'indexing') &&
														'cursor-default opacity-40'
												)}
												onClick={() => {
													if (
														!canReindex ||
														!sourceRoot ||
														reindexSource.isPending ||
														context.map_state ===
															'indexing'
													)
														return;
													reindexSource.mutate({
														path: sourceRoot,
														name: null,
														unfiltered: false
													});
												}}
											/>
										</Tooltip>
										<Tooltip
											label="Open source"
											asChild
											tooltipClassName="!z-[1000]"
										>
											<CircleButton
												icon={Database}
												size="lg"
												title="Open source"
												onClick={openSource}
											/>
										</Tooltip>
									</>
								) : canAddSource ? (
									<Tooltip
										label="Add as a source"
										asChild
										tooltipClassName="!z-[1000]"
									>
										<CircleButton
											icon={Plus}
											size="lg"
											title="Add as a source"
											onClick={addSource}
										/>
									</Tooltip>
								) : null}

								{context.volume && (
									<Tooltip
										label="Open volume in Storage"
										asChild
										tooltipClassName="!z-[1000]"
									>
										<CircleButton
											icon={HardDrive}
											size="lg"
											title="Open volume in Storage"
											onClick={openStorage}
										/>
									</Tooltip>
								)}

								<Tooltip
									label={
										showDetails
											? 'Hide details'
											: 'Show details'
									}
									asChild
									tooltipClassName="!z-[1000]"
								>
									<CircleButton
										icon={Info}
										size="lg"
										active={showDetails}
										title={
											showDetails
												? 'Hide details'
												: 'Show details'
										}
										onClick={() =>
											setShowDetails(
												(visible) => !visible
											)
										}
									/>
								</Tooltip>
							</div>

							{showDetails && (
								<div className="border-app-line grid grid-cols-[88px_1fr] gap-x-3 gap-y-1.5 border-t px-4 py-3 text-[9px]">
									<span className="text-ink-faint">
										Source root
									</span>
									<span
										className="text-ink-dull truncate text-right"
										title={
											context.source?.root ?? undefined
										}
									>
										{context.source?.root ?? 'None'}
									</span>
									<span className="text-ink-faint">
										Source record
									</span>
									<span className="text-ink-dull text-right">
										{context.storage.source_record
											? 'Committed'
											: 'None'}
									</span>
									<span className="text-ink-faint">
										Watch root
									</span>
									<span
										className="text-ink-dull truncate text-right"
										title={
											context.watcher_root ?? undefined
										}
									>
										{context.watcher_root ?? 'None'}
									</span>
									<span className="text-ink-faint">
										Volume mount
									</span>
									<span
										className="text-ink-dull truncate text-right"
										title={
											context.volume?.mount_point ??
											undefined
										}
									>
										{context.volume?.mount_point ?? 'None'}
									</span>
									<span className="text-ink-faint">
										Arena
									</span>
									<span className="text-ink-dull text-right">
										{statusLabel(context.map_state)}
									</span>
									<span className="text-ink-faint">
										Restart cache
									</span>
									<span className="text-ink-dull text-right">
										{context.storage.restart_cache
											? 'Available'
											: 'None'}
									</span>
								</div>
							)}
						</>
					)}
				</div>
			</Popover.Content>
		</Popover.Root>
	);
}

export function PathBar({path, devices, onNavigate}: PathBarProps) {
	const [isExpanded, setIsExpanded] = useState(false);
	const [isShiftHeld, setIsShiftHeld] = useState(false);
	const [isEditing, setIsEditing] = useState(false);
	const [editValue, setEditValue] = useState('');
	const [editingAsUri, setEditingAsUri] = useState(false);
	const {navigateToView} = useExplorer();
	const uri = sdPathToUri(path);
	const currentDir = getCurrentDirectoryName(path);
	const segments = parsePathSegments(path);

	// Get device icon and device info based on the device_slug
	const deviceInfo = (() => {
		if ('Physical' in path) {
			const deviceSlug = path.Physical.device_slug;
			// Find device by slug
			const device = Array.from(devices.values()).find(
				(d) => d.slug === deviceSlug
			);
			return {
				icon: device ? getDeviceIcon(device) : LaptopIcon,
				device
			};
		}
		// For Cloud paths, we don't have a device
		return {icon: LaptopIcon, device: undefined};
	})();

	const handleDeviceClick = () => {
		if (deviceInfo.device) {
			navigateToView('device', deviceInfo.device.id);
		}
	};

	const enterEditMode = (initialValue: string, asUri: boolean) => {
		setIsEditing(true);
		setEditValue(initialValue);
		setEditingAsUri(asUri);
	};

	const exitEditMode = () => {
		setIsEditing(false);
		setEditValue('');
		setEditingAsUri(false);
	};

	const handleContainerClick = (e: React.MouseEvent) => {
		// Only enter edit mode if clicking the container itself, not buttons/segments
		if (
			e.target === e.currentTarget ||
			(e.target as HTMLElement).tagName === 'INPUT'
		) {
			const isUriMode = showUri;
			const valueToEdit = isUriMode
				? uri
				: 'Physical' in path
					? path.Physical.path
					: uri;
			enterEditMode(valueToEdit, isUriMode);
		}
	};

	const handleEditKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
		if (e.key === 'Enter') {
			e.preventDefault();
			submitEdit();
		} else if (e.key === 'Escape') {
			e.preventDefault();
			exitEditMode();
		}
	};

	const submitEdit = () => {
		const trimmed = editValue.trim();
		if (!trimmed) {
			exitEditMode();
			return;
		}

		try {
			if (editingAsUri) {
				// Try to parse as SdPath JSON
				const parsed = JSON.parse(trimmed) as SdPath;
				onNavigate(parsed);
			} else {
				// Parse as file path string
				if ('Physical' in path) {
					const newPath: SdPath = {
						Physical: {
							device_slug: path.Physical.device_slug,
							path: trimmed.startsWith('/')
								? trimmed
								: `/${trimmed}`
						}
					};
					onNavigate(newPath);
				}
			}
		} catch (error) {
			console.error('Failed to parse path:', error);
		}

		exitEditMode();
	};

	useEffect(() => {
		const handleKeyDown = (e: KeyboardEvent) => {
			if (e.key === 'Shift') setIsShiftHeld(true);
		};
		const handleKeyUp = (e: KeyboardEvent) => {
			if (e.key === 'Shift') setIsShiftHeld(false);
		};

		window.addEventListener('keydown', handleKeyDown);
		window.addEventListener('keyup', handleKeyUp);

		return () => {
			window.removeEventListener('keydown', handleKeyDown);
			window.removeEventListener('keyup', handleKeyUp);
		};
	}, []);

	const showUri = isExpanded && isShiftHeld;

	// Calculate widths for different states
	const collapsedWidth = currentDir.length * 8.5 + 70;
	const breadcrumbsWidth = Math.min(
		segments.reduce((sum, seg) => sum + seg.name.length * 6.5, 0) +
			(segments.length - 1) * 16 + // separators
			70, // base padding + icon
		600
	);
	const uriWidth = Math.min(uri.length * 7 + 70, 600);
	const editWidth = Math.max(200, Math.min(editValue.length * 7 + 70, 600));

	const currentWidth = isEditing
		? editWidth
		: !isExpanded
			? collapsedWidth
			: showUri
				? uriWidth
				: breadcrumbsWidth;

	return (
		<div className="flex items-center gap-2">
			<motion.div
				animate={{width: currentWidth}}
				transition={{duration: 0.2, ease: [0.25, 1, 0.5, 1]}}
				onMouseEnter={() => !isEditing && setIsExpanded(true)}
				onMouseLeave={() => !isEditing && setIsExpanded(false)}
				onClick={handleContainerClick}
				className={clsx(
					'flex h-8 items-center gap-1.5 rounded-full px-3',
					'border-app-line/50 border backdrop-blur-xl',
					'bg-app-overlay/80 transition-colors',
					'focus-within:bg-sidebar-box/30 focus-within:border-sidebar-line/40',
					!isEditing && 'cursor-text'
				)}
			>
				<button
					onClick={(e) => {
						e.stopPropagation();
						handleDeviceClick();
					}}
					disabled={!deviceInfo.device}
					title={
						deviceInfo.device
							? `Go to ${deviceInfo.device.name}`
							: 'Device'
					}
					className={clsx(
						'size-5 flex-shrink-0 transition-opacity',
						deviceInfo.device
							? 'cursor-pointer opacity-60 hover:opacity-100'
							: 'cursor-default opacity-60'
					)}
				>
					<img
						src={deviceInfo.icon}
						alt="Device"
						className="size-full"
					/>
				</button>

				{isEditing ? (
					<input
						type="text"
						value={editValue}
						onChange={(e) => setEditValue(e.target.value)}
						onKeyDown={handleEditKeyDown}
						onBlur={exitEditMode}
						autoFocus
						className={clsx(
							'min-w-0 flex-1 border-0 bg-transparent outline-none ring-0',
							'text-sidebar-ink text-xs font-medium',
							'placeholder:text-sidebar-inkFaint',
							'focus:outline-none focus:ring-0',
							editingAsUri && 'font-mono'
						)}
						placeholder={
							editingAsUri
								? 'Enter SdPath JSON...'
								: 'Enter path...'
						}
					/>
				) : showUri ? (
					<input
						type="text"
						value={uri}
						readOnly
						className={clsx(
							'min-w-0 flex-1 border-0 bg-transparent outline-none ring-0',
							'text-sidebar-ink text-xs font-medium',
							'placeholder:text-sidebar-inkFaint',
							'cursor-text select-all',
							'focus:outline-none focus:ring-0'
						)}
						placeholder="No path selected"
					/>
				) : isExpanded ? (
					<div className="flex min-w-0 flex-1 items-center gap-1 overflow-hidden">
						{segments.map((segment, index) => {
							const isLast = index === segments.length - 1;
							return (
								<div
									key={index}
									className="flex flex-shrink-0 items-center gap-1"
								>
									<button
										onClick={(e) => {
											e.stopPropagation();
											!isLast && onNavigate(segment.path);
										}}
										disabled={isLast}
										className={clsx(
											'whitespace-nowrap text-xs font-medium transition-colors',
											isLast
												? 'text-sidebar-ink cursor-default'
												: 'text-sidebar-inkDull hover:text-sidebar-ink cursor-pointer'
										)}
									>
										{segment.name}
									</button>
									{!isLast && (
										<button
											onClick={(e) => {
												e.stopPropagation();
												const valueToEdit =
													'Physical' in path
														? path.Physical.path
														: uri;
												enterEditMode(
													valueToEdit,
													false
												);
											}}
											className="cursor-text opacity-50 transition-opacity hover:opacity-100"
										>
											<CaretRight size={12} />
										</button>
									)}
								</div>
							);
						})}
					</div>
				) : (
					<input
						type="text"
						value={currentDir}
						readOnly
						className={clsx(
							'min-w-0 flex-1 border-0 bg-transparent outline-none ring-0',
							'text-sidebar-ink text-xs font-medium',
							'placeholder:text-sidebar-inkFaint',
							'cursor-text select-all',
							'focus:outline-none focus:ring-0'
						)}
						placeholder="No path selected"
					/>
				)}
			</motion.div>
			<PathStatusButton path={path} />
		</div>
	);
}
