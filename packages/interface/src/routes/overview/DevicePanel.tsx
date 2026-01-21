import {
	CaretLeft,
	CaretRight,
	Cpu,
	HardDrive,
	Memory
} from '@phosphor-icons/react';
import DatabaseIcon from '@sd/assets/icons/Database.png';
import DriveAmazonS3Icon from '@sd/assets/icons/Drive-AmazonS3.png';
import DriveDropboxIcon from '@sd/assets/icons/Drive-Dropbox.png';
import DriveGoogleDriveIcon from '@sd/assets/icons/Drive-GoogleDrive.png';
import DriveIcon from '@sd/assets/icons/Drive.png';
import HDDIcon from '@sd/assets/icons/HDD.png';
import LocationIcon from '@sd/assets/icons/Location.png';
import ServerIcon from '@sd/assets/icons/Server.png';
import type {
	Device,
	JobListItem,
	ListLibraryDevicesInput,
	Location,
	LocationsListOutput,
	LocationsListQueryInput,
	Volume,
	VolumeListOutput,
	VolumeListQueryInput
} from '@sd/ts-client';
import {TopBarButton} from '@sd/ui';
import clsx from 'clsx';
import {useEffect, useRef, useState} from 'react';
import Masonry from 'react-masonry-css';
import {JobCard} from '../../components/JobManager/components/JobCard';
import {useJobsContext} from '../../components/JobManager/hooks/JobsContext';
import {
	getDeviceIcon,
	useCoreQuery,
	useNormalizedQuery
} from '../../contexts/SpacedriveContext';
import {VolumeBar} from './VolumeBar';

// Temporary type extension until types are regenerated
type DeviceWithConnection = Device & {
	connection_method?: 'Direct' | 'Relay' | 'Mixed' | null;
};

export function formatBytes(bytes: number): string {
	if (bytes === 0) return '0 B';
	const k = 1024;
	const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

export function getVolumeIcon(volumeType: any, name?: string): string {
	// Convert volume type to string if it's an enum variant object
	const volumeTypeStr =
		typeof volumeType === 'string'
			? volumeType
			: volumeType?.Other || JSON.stringify(volumeType);

	// Check for cloud providers by name
	if (name?.includes('S3')) return DriveAmazonS3Icon;
	if (name?.includes('Google')) return DriveGoogleDriveIcon;
	if (name?.includes('Dropbox')) return DriveDropboxIcon;

	// By type
	if (volumeTypeStr === 'Cloud') return DriveIcon;
	if (volumeTypeStr === 'Network') return ServerIcon;
	if (volumeTypeStr === 'Virtual') return DatabaseIcon;
	return HDDIcon;
}

interface DevicePanelProps {
	onLocationSelect?: (location: Location | null) => void;
}

export function DevicePanel({onLocationSelect}: DevicePanelProps = {}) {
	const [selectedLocationId, setSelectedLocationId] = useState<string | null>(
		null
	);

	// Fetch all volumes using normalized cache
	const {data: volumesData, isLoading: volumesLoading} = useNormalizedQuery<
		VolumeListQueryInput,
		VolumeListOutput
	>({
		wireMethod: 'query:volumes.list',
		input: {filter: 'All'},
		resourceType: 'volume'
	});

	// Fetch all devices using normalized cache
	const {data: devicesData, isLoading: devicesLoading} = useNormalizedQuery<
		ListLibraryDevicesInput,
		DeviceWithConnection[]
	>({
		wireMethod: 'query:devices.list',
		input: {include_offline: true, include_details: false},
		resourceType: 'device'
	});

	// Fetch all locations using normalized cache
	const {data: locationsData, isLoading: locationsLoading} =
		useNormalizedQuery<LocationsListQueryInput, LocationsListOutput>({
			wireMethod: 'query:locations.list',
			input: null,
			resourceType: 'location'
		});

	// Get all jobs with real-time updates (local jobs)
	const {jobs: localJobs} = useJobsContext();

	// Get remote device jobs
	// TODO: This should have its own hook like useJobs, this will not work reactively
	const {data: remoteJobsData} = useCoreQuery({
		type: 'jobs.remote.all_devices',
		input: {}
	});

	// Merge local and remote jobs
	const allJobs = [
		...localJobs,
		...(remoteJobsData?.jobs_by_device
			? Object.values(remoteJobsData.jobs_by_device)
					.flat()
					.map((remoteJob) => ({
						id: remoteJob.job_id,
						name: remoteJob.job_type,
						device_id: remoteJob.device_id,
						status: remoteJob.status,
						progress: remoteJob.progress || 0,
						action_type: null,
						action_context: null
					}))
			: [])
	] as JobListItem[];

	// Only block on devices loading (foundation data)
	// Volumes and locations can load progressively within each device card
	if (devicesLoading) {
		return (
			<div className="bg-app-box border-app-line overflow-hidden rounded-xl border">
				<div className="border-app-line border-b px-6 py-4">
					<h2 className="text-ink text-base font-semibold">Devices</h2>
					<p className="text-ink-dull mt-1 text-sm">Loading devices...</p>
				</div>
			</div>
		);
	}

	const volumes = volumesData?.volumes || [];
	const devices = devicesData || [];
	const locations = locationsData?.locations || [];

	// Filter to only show user-visible volumes
	const userVisibleVolumes = volumes.filter(
		(volume) => volume.is_user_visible !== false
	);

	// Group volumes by device_id
	const volumesByDevice = userVisibleVolumes.reduce(
		(acc, volume) => {
			const deviceId = volume.device_id;
			if (!acc[deviceId]) {
				acc[deviceId] = [];
			}
			acc[deviceId].push(volume);
			return acc;
		},
		{} as Record<string, Volume[]>
	);

	// Group locations by device slug
	const locationsByDeviceSlug = locations.reduce(
		(acc, location) => {
			// Extract device_slug from sd_path
			if (
				typeof location.sd_path === 'object' &&
				'Physical' in location.sd_path
			) {
				const deviceSlug = location.sd_path.Physical.device_slug;
				if (!acc[deviceSlug]) {
					acc[deviceSlug] = [];
				}
				acc[deviceSlug].push(location);
			}
			return acc;
		},
		{} as Record<string, Location[]>
	);

	// Create device map for quick lookup
	const deviceMap = devices.reduce(
		(acc, device) => {
			acc[device.id] = device;
			return acc;
		},
		{} as Record<string, DeviceWithConnection>
	);

	// Group jobs by device_id
	const jobsByDevice = allJobs.reduce(
		(acc, job) => {
			const deviceId = job.device_id;
			if (!acc[deviceId]) {
				acc[deviceId] = [];
			}
			acc[deviceId].push(job);
			return acc;
		},
		{} as Record<string, JobListItem[]>
	);

	const breakpointColumns = {
		default: 3,
		1600: 2,
		1000: 1
	};

	return (
		<div className="">
			<Masonry
				breakpointCols={breakpointColumns}
				className="-ml-4 flex w-auto"
				columnClassName="pl-4 bg-clip-padding"
			>
				{devices.map((device) => {
					const deviceVolumes = volumesByDevice[device.id] || [];
					const deviceJobs = jobsByDevice[device.id] || [];
					const deviceLocations =
						locationsByDeviceSlug[device.slug] || [];

					return (
						<DeviceCard
							key={device.id}
							device={device}
							volumes={deviceVolumes}
							jobs={deviceJobs}
							locations={deviceLocations}
							selectedLocationId={selectedLocationId}
							volumesLoading={volumesLoading}
							locationsLoading={locationsLoading}
							onLocationSelect={(location) => {
								if (location) {
									setSelectedLocationId(location.id);
								} else {
									setSelectedLocationId(null);
								}
								onLocationSelect?.(location);
							}}
						/>
					);
				})}

				{devices.length === 0 && (
					<div className="bg-app-box border-app-line overflow-hidden rounded-xl border">
						<div className="text-ink-faint py-12 text-center">
							<HardDrive className="mx-auto mb-3 size-12 opacity-20" />
							<p className="text-sm">No devices detected</p>
							<p className="mt-1 text-xs">
								Pair a device to get started
							</p>
						</div>
					</div>
				)}
			</Masonry>
		</div>
	);
}

interface ConnectionBadgeProps {
	method: 'Direct' | 'Relay' | 'Mixed';
}

function ConnectionBadge({method}: ConnectionBadgeProps) {
	const labels = {
		Direct: 'Local',
		Relay: 'Relay',
		Mixed: 'Mixed'
	};

	return (
		<div className="flex items-center gap-1.5">
			<div className="bg-ink-dull size-2 rounded-full" />
			<span className="text-ink-dull text-xs font-medium">
				{labels[method]}
			</span>
		</div>
	);
}

interface DeviceCardProps {
	device?: DeviceWithConnection;
	volumes: Volume[];
	jobs: JobListItem[];
	locations: Location[];
	selectedLocationId: string | null;
	volumesLoading: boolean;
	locationsLoading: boolean;
	onLocationSelect?: (location: Location | null) => void;
}

function DeviceCard({
	device,
	volumes,
	jobs,
	locations,
	selectedLocationId,
	volumesLoading,
	locationsLoading,
	onLocationSelect
}: DeviceCardProps) {
	const deviceName = device?.name || 'Unknown Device';
	const deviceIconSrc = device ? getDeviceIcon(device) : null;
	const {pause, resume, cancel, getSpeedHistory} = useJobsContext();
	// Format hardware specs
	// Convert form_factor enum to string
	const formFactor = device?.form_factor
		? typeof device.form_factor === 'string'
			? device.form_factor
			: (device.form_factor as any)?.Other ||
				JSON.stringify(device.form_factor)
		: null;
	// Override CPU model for Apple mobile devices when missing
	const cpuModel = device?.cpu_model ||
		(formFactor === 'Mobile' && device?.manufacturer === 'Apple'
			? 'Apple A16 Bionic'
			: null);
	const cpuInfo = cpuModel
		? `${cpuModel}${device.cpu_cores_physical ? ` � ${device.cpu_cores_physical}C` : ''}`
		: null;
	const ramInfo = device?.memory_total_bytes
		? formatBytes(device.memory_total_bytes)
		: null;
	const manufacturer = device?.manufacturer;

	// Filter active jobs
	const activeJobs = jobs.filter(
		(j) => j.status === 'running' || j.status === 'paused'
	);

	return (
		<div className="bg-app-darkBox border-app-line mb-4 overflow-hidden rounded-xl border">
			{/* Device Header */}
			<div className="bg-app-box border-app-line border-b px-6 py-4">
				<div className="flex items-center gap-4">
					{/* Left: Device icon and name */}
					<div className="flex min-w-0 flex-1 items-center gap-3">
						{deviceIconSrc ? (
							<img
								src={deviceIconSrc}
								alt={deviceName}
								className="size-8 flex-shrink-0 opacity-80"
							/>
						) : (
							<HardDrive
								className="text-ink size-8 flex-shrink-0"
								weight="duotone"
							/>
						)}
						<div className="min-w-0">
							<div className="flex items-center gap-2">
								<h3 className="text-ink truncate text-base font-semibold">
									{deviceName}
								</h3>
								{device?.connection_method && (
									<ConnectionBadge
										method={device.connection_method}
									/>
								)}
							</div>
							<p className="text-ink-dull text-sm">
								{volumesLoading
									? 'Loading volumes...'
									: `${volumes.length} ${volumes.length === 1 ? 'volume' : 'volumes'}`}
								{device?.is_online === false && ' • Offline'}
							</p>
						</div>
					</div>

					{/* Right: Hardware specs */}
					<div className="flex flex-col gap-1.5">
						{/* CPU Model */}
						{cpuInfo && (
							<div
								className="text-ink text-right text-xs font-medium"
								title={cpuInfo}
							>
								{cpuModel || 'CPU'}
							</div>
						)}

						{/* Stats row */}
						<div className="text-ink-dull flex items-center justify-end gap-3 text-[11px]">
							{device?.cpu_cores_physical && (
								<div
									className="flex items-center gap-1"
									title={`${device.cpu_cores_physical} Cores / ${device.cpu_cores_logical} Threads`}
								>
									<Cpu
										className="size-3.5 opacity-50"
										weight="duotone"
									/>
									<span>
										{Math.max(
											device.cpu_cores_physical || 0,
											device.cpu_cores_logical || 0
										)}
									</span>
								</div>
							)}
							{ramInfo && (
								<div
									className="flex items-center gap-1"
									title={`${ramInfo} Total Memory`}
								>
									<Memory
										className="size-3.5 opacity-50"
										weight="duotone"
									/>
									<span>{ramInfo}</span>
								</div>
							)}
						</div>
					</div>
				</div>
			</div>

			<div>
				{/* Active Jobs Section */}
				{activeJobs.length > 0 && (
					<div className="border-app-line bg-app/50 space-y-2 border-b px-3 py-3">
						{activeJobs.map((job) => (
							<JobCard
								key={job.id}
								job={job}
								onPause={pause}
								onResume={resume}
								onCancel={cancel}
								getSpeedHistory={getSpeedHistory}
							/>
						))}
					</div>
				)}

				{/* Locations for this device */}
				{locationsLoading ? (
					<div className="border-app-line bg-app/50 border-b px-3 py-3">
						<div className="text-ink-dull text-center text-xs">
							Loading locations...
						</div>
					</div>
				) : (
					locations.length > 0 && (
						<LocationsScroller
							locations={locations}
							selectedLocationId={selectedLocationId}
							onLocationSelect={onLocationSelect}
						/>
					)
				)}

				{/* Volumes for this device */}
				<div className="space-y-3 px-3 py-3">
					{volumesLoading ? (
						<div className="text-ink-dull text-center text-xs">
							Loading volumes...
						</div>
					) : volumes.length > 0 ? (
						volumes.map((volume, idx) => (
							<VolumeBar
								key={volume.id}
								volume={volume}
								index={idx}
							/>
						))
					) : (
						<div className="flex flex-col items-center justify-center py-8 text-center">
							<div className="text-ink-faint">
								<HardDrive className="mx-auto mb-2 size-8 opacity-20" />
								<p className="text-xs">No volumes</p>
							</div>
						</div>
					)}
				</div>
			</div>
		</div>
	);
}

interface LocationsScrollerProps {
	locations: Location[];
	selectedLocationId: string | null;
	onLocationSelect?: (location: Location | null) => void;
}

function LocationsScroller({
	locations,
	selectedLocationId,
	onLocationSelect
}: LocationsScrollerProps) {
	const scrollRef = useRef<HTMLDivElement>(null);
	const [canScrollLeft, setCanScrollLeft] = useState(false);
	const [canScrollRight, setCanScrollRight] = useState(false);

	const updateScrollState = () => {
		if (!scrollRef.current) return;
		const {scrollLeft, scrollWidth, clientWidth} = scrollRef.current;
		setCanScrollLeft(scrollLeft > 0);
		setCanScrollRight(scrollLeft < scrollWidth - clientWidth - 1);
	};

	useEffect(() => {
		updateScrollState();
		window.addEventListener('resize', updateScrollState);
		return () => window.removeEventListener('resize', updateScrollState);
	}, [locations]);

	const scroll = (direction: 'left' | 'right') => {
		if (!scrollRef.current) return;
		const scrollAmount = 200;
		scrollRef.current.scrollBy({
			left: direction === 'left' ? -scrollAmount : scrollAmount,
			behavior: 'smooth'
		});
	};

	return (
		<div className="border-app-line border-b px-3 py-3">
			<div className="relative">
				{/* Left fade and button */}
				{canScrollLeft && (
					<>
						<div className="from-app-darkBox pointer-events-none absolute bottom-0 left-0 top-0 z-10 w-12 bg-gradient-to-r to-transparent" />
						<div className="absolute left-1 top-1/2 z-20 -translate-y-1/2">
							<TopBarButton
								icon={CaretLeft}
								onClick={() => scroll('left')}
							/>
						</div>
					</>
				)}

				{/* Scrollable container */}
				<div
					ref={scrollRef}
					onScroll={updateScrollState}
					className="scrollbar-hide flex gap-2 overflow-x-auto"
					style={{scrollbarWidth: 'none'}}
				>
					{locations.map((location) => {
						const isSelected = selectedLocationId === location.id;
						return (
							<button
								key={location.id}
								onClick={() => {
									if (isSelected) {
										onLocationSelect?.(null);
									} else {
										onLocationSelect?.(location);
									}
								}}
								className="flex min-w-[80px] flex-shrink-0 flex-col items-center gap-2 rounded-lg p-1 transition-all"
							>
								<div
									className={clsx(
										'rounded-lg p-2',
										isSelected
											? 'bg-app-box'
											: 'bg-transparent'
									)}
								>
									<img
										src={LocationIcon}
										alt={location.name}
										className="size-12 opacity-80"
									/>
								</div>
								<div className="flex w-full flex-col items-center">
									<div
										className={clsx(
											'inline-block max-w-full truncate rounded-md px-2 py-0.5 text-xs',
											isSelected
												? 'bg-accent text-white'
												: 'text-ink'
										)}
									>
										{location.name}
									</div>
								</div>
							</button>
						);
					})}
				</div>

				{/* Right fade and button */}
				{canScrollRight && (
					<>
						<div className="from-app-darkBox pointer-events-none absolute bottom-0 right-0 top-0 z-10 w-12 bg-gradient-to-l to-transparent" />
						<div className="absolute right-1 top-1/2 z-20 -translate-y-1/2">
							<TopBarButton
								icon={CaretRight}
								onClick={() => scroll('right')}
							/>
						</div>
					</>
				)}
			</div>
		</div>
	);
}
