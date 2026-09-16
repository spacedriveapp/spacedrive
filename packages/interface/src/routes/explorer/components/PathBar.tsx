import {
	CaretRight,
	CircleDashed,
	Database,
	Eye,
	HardDrive,
	House,
	Plus,
	PushPin,
	SpinnerGap,
	Stack,
	WarningCircle
} from '@phosphor-icons/react';
import type {Icon as PhosphorIcon} from '@phosphor-icons/react';
import FolderIcon from '@sd/assets/icons/Folder.webp';
import LaptopIcon from '@sd/assets/icons/Laptop.webp';
import type {Device, SdPath} from '@sd/ts-client';
import {
	getDeviceIcon,
	useLibraryMutation,
	useLibraryQuery
} from '@sd/ts-client';
import {
	CircleButton,
	Popover,
	Tooltip,
	usePopover
} from '@spacedrive/primitives';
import {useQueryClient} from '@tanstack/react-query';
import clsx from 'clsx';
import {motion} from 'framer-motion';
import {useEffect, useState} from 'react';
import {useNavigate} from 'react-router-dom';
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
				'flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[10px] font-medium',
				tone === 'active' &&
					'border-accent/20 bg-accent/10 text-accent',
				tone === 'warning' &&
					'border-amber-400/20 bg-amber-400/10 text-amber-400',
				tone === 'muted' && 'border-app-line bg-app/40 text-ink-faint',
				tone === 'neutral' && 'border-app-line bg-app/50 text-ink-dull'
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
			<div className="border-app-line bg-app/30 rounded-xl border p-3">
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
	const navigate = useNavigate();
	const queryClient = useQueryClient();
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
		void queryClient.invalidateQueries({
			predicate: (query) => {
				const key = query.queryKey[0];
				return (
					key === 'locations.list' || key === 'query:locations.list'
				);
			}
		});
	};
	const addLocation = useLibraryMutation('locations.add', {
		onSuccess: refresh
	});
	const removeLocation = useLibraryMutation('locations.remove', {
		onSuccess: refresh
	});

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
	const exactLocation = context?.location?.exact ? context.location : null;
	const pathLabel = getCurrentDirectoryName(path);
	const pathDetail =
		'Physical' in path ? path.Physical.path : sdPathToUri(path);
	const canAddLocation = Boolean(
		context?.source &&
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
	const storageLabel = context?.storage.source_store
		? 'Index on disk'
		: context?.storage.restart_cache
			? 'Cached locally'
			: context?.storage.memory
				? 'Browsing only'
				: 'Not indexed';
	const storageTone = context?.storage.source_store
		? 'active'
		: context?.storage.restart_cache || context?.storage.memory
			? 'neutral'
			: 'muted';
	const storageDetail = context?.storage.source_store
		? `${context.source?.name ?? 'This source'} has a durable index on disk.`
		: context?.storage.restart_cache
			? 'A rebuildable cache is available on this device after restart.'
			: context?.storage.memory
				? 'This browsing index lasts until the daemon stops.'
				: 'Spacedrive has not indexed this path.';

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
					<div className="flex shrink-0 items-center gap-1.5">
						{exactLocation ? (
							<Tooltip label="Remove from Places">
								<button
									type="button"
									disabled={removeLocation.isPending}
									onClick={() => {
										removeLocation.mutate({
											location_id: exactLocation.id
										});
										popover.setOpen(false);
									}}
									className="border-app-line bg-app/50 text-ink hover:border-accent/40 hover:text-accent flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[10px] font-medium transition-colors"
								>
									<PushPin size={12} weight="fill" />
									{exactLocation.name}
								</button>
							</Tooltip>
						) : context?.system_place ? (
							<div className="bg-app/50 text-ink flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[10px] font-medium">
								<House size={12} weight="fill" />
								{context.system_place}
							</div>
						) : context?.location ? (
							<div className="bg-app/50 text-ink-dull flex items-center gap-1.5 rounded-full px-2.5 py-1 text-[10px] font-medium">
								<PushPin size={12} />
								{context.location.name}
							</div>
						) : null}

						{context && canAddLocation && !exactLocation && (
							<Tooltip label="Add to Places">
								<button
									type="button"
									disabled={addLocation.isPending}
									onClick={() => {
										addLocation.mutate({
											path: context.canonical_path,
											name: null
										});
										popover.setOpen(false);
									}}
									className="border-app-line text-ink-faint hover:border-accent/40 hover:text-accent flex size-7 items-center justify-center rounded-full border border-dashed transition-colors"
								>
									<Plus size={13} />
								</button>
							</Tooltip>
						)}
					</div>
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
						<div className="px-4 py-4">
							{context.availability !== 'available' && (
								<div className="mb-3 flex items-center gap-2 rounded-lg bg-amber-400/10 px-2.5 py-2 text-[11px] font-medium text-amber-400">
									<WarningCircle size={14} />
									{statusLabel(context.availability)}
								</div>
							)}

							{context.source ? (
								<div className="border-accent/25 bg-accent/[0.04] rounded-xl border p-3">
									<button
										type="button"
										onClick={openStorage}
										className="group flex w-full items-center gap-2 text-left"
									>
										<span className="bg-accent/10 text-accent flex size-7 items-center justify-center rounded-lg">
											<Stack size={15} weight="fill" />
										</span>
										<span className="text-ink min-w-0 flex-1 truncate text-xs font-semibold">
											{context.source.name}
										</span>
										<span className="text-ink-faint text-[9px] font-semibold uppercase tracking-wider">
											Source
										</span>
										<CaretRight
											size={12}
											className="text-ink-faint group-hover:text-ink transition-colors"
										/>
									</button>

									<div className="border-app-line bg-app/50 mt-3 flex items-center gap-2.5 rounded-lg border px-2.5 py-2">
										<img
											src={FolderIcon}
											className="size-7"
											alt=""
										/>
										<div className="min-w-0">
											<div className="text-ink truncate text-[11px] font-medium">
												{pathLabel}
											</div>
											<div className="text-ink-faint text-[9px]">
												Current folder
											</div>
										</div>
									</div>

									<div className="mt-3 flex flex-wrap gap-2">
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
							) : (
								<div className="border-app-line bg-app/30 flex flex-col items-center rounded-xl border px-4 py-5 text-center">
									<span className="bg-app/70 text-ink-dull flex size-10 items-center justify-center rounded-xl">
										{context.storage.restart_cache ? (
											<Database size={19} />
										) : (
											<Eye size={19} />
										)}
									</span>
									<div className="text-ink mt-2.5 text-xs font-semibold">
										{storageLabel}
									</div>
									<div className="text-ink-faint mt-0.5 text-[10px]">
										Not kept by a source
									</div>
									{canAddSource && (
										<button
											type="button"
											onClick={addSource}
											className="bg-accent hover:bg-accent-deep mt-3 flex items-center gap-1.5 rounded-full px-3 py-1.5 text-[10px] font-semibold text-white transition-colors"
										>
											<Plus size={12} />
											Add as a source
										</button>
									)}
								</div>
							)}

							{context.volume && (
								<button
									type="button"
									onClick={openStorage}
									className="border-app-line hover:bg-app/50 mt-3 flex w-full items-center gap-2.5 rounded-lg border px-2.5 py-2 text-left transition-colors"
								>
									<span className="bg-app/60 text-ink-dull flex size-7 items-center justify-center rounded-lg">
										<HardDrive size={15} />
									</span>
									<div className="min-w-0 flex-1">
										<div className="text-ink truncate text-[11px] font-medium">
											{context.volume.name}
										</div>
										<div className="text-ink-faint text-[9px]">
											Volume
										</div>
									</div>
									<CaretRight
										size={12}
										className="text-ink-faint"
									/>
								</button>
							)}

							<details className="group mt-3">
								<summary className="text-ink-faint hover:text-ink flex cursor-pointer list-none items-center gap-1 text-[10px] font-medium transition-colors [&::-webkit-details-marker]:hidden">
									<CaretRight
										size={10}
										className="transition-transform group-open:rotate-90"
									/>
									Technical details
								</summary>
								<div className="border-app-line mt-2 grid grid-cols-[76px_1fr] gap-x-3 gap-y-1.5 border-t pt-2 text-[9px]">
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
										Runtime map
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
							</details>
						</div>
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
