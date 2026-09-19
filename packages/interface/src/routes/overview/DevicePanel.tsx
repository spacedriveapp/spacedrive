import {
	Cpu,
	HardDrive,
	Memory,
	WifiHigh,
	WifiSlash
} from '@phosphor-icons/react';
import DatabaseIcon from '@sd/assets/icons/Database.webp';
import DriveAmazonS3Icon from '@sd/assets/icons/Drive-AmazonS3.webp';
import DriveDropboxIcon from '@sd/assets/icons/Drive-Dropbox.webp';
import DriveGoogleDriveIcon from '@sd/assets/icons/Drive-GoogleDrive.webp';
import DriveIcon from '@sd/assets/icons/Drive.webp';
import HDDIcon from '@sd/assets/icons/HDD.webp';
import ServerIcon from '@sd/assets/icons/Server.webp';
import type {
	Device,
	JobListItem,
	ListLibraryDevicesInput,
	SourceInfo,
	Volume,
	VolumeListOutput,
	VolumeListQueryInput
} from '@sd/ts-client';
import {Tooltip} from '@spacedrive/primitives';
import clsx from 'clsx';
import Masonry from 'react-masonry-css';
import {JobCard} from '../../components/JobManager/components/JobCard';
import {useJobsContext} from '../../components/JobManager/hooks/JobsContext';
import {
	getDeviceIcon,
	useCoreQuery,
	useLibraryQuery,
	useNormalizedQuery
} from '../../contexts/SpacedriveContext';
import {VolumeBar} from './VolumeBar';

// Temporary type extension until types are regenerated
type DeviceWithConnection = Device & {
	connection_method?:
		| 'LocalNetwork'
		| 'DirectInternet'
		| 'Tailscale'
		| 'RelayProxy'
		| null;
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

export function DevicePanel() {
	// Fetch all volumes using normalized cache
	const {data: volumesData, isLoading: volumesLoading} = useNormalizedQuery<
		VolumeListQueryInput,
		VolumeListOutput
	>({
		query: 'volumes.list',
		input: {filter: 'All'},
		resourceType: 'volume'
	});

	// Fetch all devices using normalized cache
	const {data: devicesData, isLoading: devicesLoading} = useNormalizedQuery<
		ListLibraryDevicesInput,
		DeviceWithConnection[]
	>({
		query: 'devices.list',
		// Paired devices live in the network registry, not this library's
		// device table; without show_paired a peer only appeared when a
		// connection event happened to announce it.
		input: {include_offline: true, include_details: false, show_paired: true},
		resourceType: 'device'
	});

	// Get all jobs with real-time updates (local jobs)
	const {jobs: localJobs} = useJobsContext();

	// Get remote device jobs
	// TODO: This should have its own hook like useJobs, this will not work reactively
	const {data: remoteJobsData} = useCoreQuery({
		type: 'jobs.remote.all_devices',
		input: {}
	});

	// Sources, including paired devices' replicas
	const {data: sourcesData} = useLibraryQuery({
		type: 'sources.list',
		input: {data_type: null}
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
	// Volumes and sources can load progressively within each device card
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

	// A local source on a listed volume is part of that volume's bar. Any
	// other filesystem source gets its own row on the owning device's card: a
	// paired device's replica, which carries its owner's id, or a local source
	// on a volume the card doesn't list. Adapter sources live on no device.
	const currentDeviceId = devices.find((device) => device.is_current)?.id;
	const listedVolumeIds = new Set(
		userVisibleVolumes.map((volume) => volume.id)
	);
	const sourcesByVolume: Record<string, SourceInfo[]> = {};
	const sourceRowsByDevice: Record<string, SourceInfo[]> = {};
	for (const source of sourcesData ?? []) {
		if (source.data_type !== 'filesystem') continue;
		if (
			!source.device_id &&
			source.volume_uuid &&
			listedVolumeIds.has(source.volume_uuid)
		) {
			(sourcesByVolume[source.volume_uuid] ??= []).push(source);
			continue;
		}
		const owner = source.device_id ?? currentDeviceId;
		if (owner) (sourceRowsByDevice[owner] ??= []).push(source);
	}

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
					const deviceSourceRows =
						sourceRowsByDevice[device.id] || [];

					return (
						<DeviceCard
							key={device.id}
							device={device}
							volumes={deviceVolumes}
							sourcesByVolume={sourcesByVolume}
							sourceRows={deviceSourceRows}
							jobs={deviceJobs}
							volumesLoading={volumesLoading}
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

interface ConnectionBadgeConfig {
	label: string;
	description: string;
	icon?: React.ComponentType<{className?: string}>;
	color?: string;
}

interface ConnectionBadgeProps {
	method: 'LocalNetwork' | 'DirectInternet' | 'Tailscale' | 'RelayProxy';
	online: boolean;
	current: boolean;
	icon?: React.ComponentType<{className?: string}>;
	color?: string;
}

function ConnectionBadge({method, online, current, icon: customIcon, color: customColor}: ConnectionBadgeProps) {
	const configs: Record<string, ConnectionBadgeConfig> = {
		LocalNetwork: {
			label: 'Local',
			description: 'Connected via local network',
			icon: WifiHigh,
			color: 'bg-green-500'
		},
		DirectInternet: {
			label: 'Direct',
			description: 'Connected directly via internet',
			color: 'bg-blue-500'
		},
		Tailscale: {
			label: 'Tailscale',
			description: 'Connected directly over your Tailscale network',
			color: 'bg-indigo-400'
		},
		RelayProxy: {
			label: 'Relay',
			description: 'Connected via relay proxy',
			color: 'bg-yellow-500'
		},
		Offline: {
			label: 'Offline',
			description: 'Device is currently offline',
			icon: WifiSlash,
			color: 'bg-ink-dull'
		},
		Current: {
			label: 'This device',
			description: 'This is your current device',
		}
	};

	const state = current ? 'Current' : online ? method : 'Offline';
	const config = configs[state];
	const Icon = customIcon || config?.icon || null;
	const dotColor = customColor || config?.color || 'bg-ink-dull';

	return (
		<Tooltip label={config.description}>
			<div className="flex items-center gap-1.5">
				{Icon ? (
					<Icon className="size-3" />
				) : !current && (
					<div className={clsx('size-2 rounded-full', dotColor)} />
				)}
				<span className="text-ink-dull text-xs font-medium">
					{config.label}
				</span>
			</div>
		</Tooltip>
	);
}

interface DeviceCardProps {
	device?: DeviceWithConnection;
	volumes: Volume[];
	sourcesByVolume: Record<string, SourceInfo[]>;
	sourceRows: SourceInfo[];
	jobs: JobListItem[];
	volumesLoading: boolean;
}

function DeviceCard({
	device,
	volumes,
	sourcesByVolume,
	sourceRows,
	jobs,
	volumesLoading
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
		? `${cpuModel}${device?.cpu_cores_physical ? ` \u00b7 ${device.cpu_cores_physical}C` : ''}`
		: null;
	const ramInfo = device?.memory_total_bytes
		? formatBytes(device.memory_total_bytes)
		: null;
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
								<ConnectionBadge
										method={device?.connection_method ?? "LocalNetwork"}
										online={device?.is_online ?? false}
										current={device?.is_current ?? false}
									/>
							</div>
							<p className="text-ink-dull text-sm">
								{volumesLoading
									? 'Loading volumes...'
									: `${volumes.length} ${volumes.length === 1 ? 'volume' : 'volumes'}`}
								{/* {device?.is_online === false && ' • Offline'} */}
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
								sources={sourcesByVolume[volume.id] || []}
								index={idx}
							/>
						))
					) : sourceRows.length > 0 ? null : (
						<div className="flex flex-col items-center justify-center py-8 text-center">
							<div className="text-ink-faint">
								<HardDrive className="mx-auto mb-2 size-8 opacity-20" />
								<p className="text-xs">No volumes</p>
							</div>
						</div>
					)}
					{sourceRows.map((source) => (
						<SourceRow key={source.id} source={source} />
					))}
				</div>
			</div>
		</div>
	);
}

// A source no volume bar on the card covers: a paired device's source
// replicated through the peer-mount plane, or a local source on an unlisted
// volume. There is no capacity bar because the row knows the source's
// contents, not the drive underneath it.
function SourceRow({source}: {source: SourceInfo}) {
	return (
		<div className="bg-app-box border-app-line/50 overflow-hidden rounded-lg border">
			<div className="flex h-[64px] items-center gap-3 px-3">
				<img
					src={HDDIcon}
					alt=""
					className="size-10 flex-shrink-0 opacity-80"
				/>
				<div className="min-w-0 flex-1">
					<div className="mb-1.5 flex items-center gap-2">
						<span className="text-ink truncate text-sm font-semibold">
							{source.name}
						</span>
					</div>
					<div className="text-ink-dull flex h-[18px] items-center gap-1.5 text-[10px]">
						<span className="bg-app-box border-app-line rounded border px-1.5 py-0.5">
							{source.device_id ? 'Replica' : 'Source'}
						</span>
						{source.item_count > 0 && (
							<span className="bg-accent/20 border-accent/30 text-accent rounded border px-1.5 py-0.5 font-medium">
								{source.item_count.toLocaleString()} files
							</span>
						)}
					</div>
				</div>
				<div className="flex flex-shrink-0 flex-col justify-between text-right">
					{source.total_bytes != null && source.total_bytes > 0 && (
						<div className="text-ink text-sm font-medium">
							{formatBytes(source.total_bytes)}
						</div>
					)}
					{source.last_synced && (
						<div className="text-ink-dull text-[10px]">
							synced{' '}
							{new Date(source.last_synced).toLocaleTimeString()}
						</div>
					)}
				</div>
			</div>
		</div>
	);
}

