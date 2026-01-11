import { useState, useRef, useEffect } from "react";
import { motion } from "framer-motion";
import {
	HardDrive,
	Plus,
	Database,
	CaretLeft,
	CaretRight,
} from "@phosphor-icons/react";
import Masonry from "react-masonry-css";
import DriveIcon from "@sd/assets/icons/Drive.png";
import HDDIcon from "@sd/assets/icons/HDD.png";
import ServerIcon from "@sd/assets/icons/Server.png";
import DatabaseIcon from "@sd/assets/icons/Database.png";
import DriveAmazonS3Icon from "@sd/assets/icons/Drive-AmazonS3.png";
import DriveGoogleDriveIcon from "@sd/assets/icons/Drive-GoogleDrive.png";
import DriveDropboxIcon from "@sd/assets/icons/Drive-Dropbox.png";
import LocationIcon from "@sd/assets/icons/Location.png";
import { TopBarButton } from "@sd/ui";
import {
	useNormalizedQuery,
	useLibraryMutation,
	getDeviceIcon,
	useCoreQuery,
} from "../../contexts/SpacedriveContext";
import type {
	VolumeListOutput,
	VolumeListQueryInput,
	VolumeItem,
	Device,
	ListLibraryDevicesInput,
	JobListItem,
	LocationsListOutput,
	LocationsListQueryInput,
	Location,
} from "@sd/ts-client";
import { useJobs } from "../../components/JobManager/hooks/useJobs";
import { JobCard } from "../../components/JobManager/components/JobCard";
import clsx from "clsx";

function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB", "PB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

function getVolumeIcon(volumeType: any, name?: string): string {
	// Convert volume type to string if it's an enum variant object
	const volumeTypeStr =
		typeof volumeType === "string"
			? volumeType
			: volumeType?.Other || JSON.stringify(volumeType);

	// Check for cloud providers by name
	if (name?.includes("S3")) return DriveAmazonS3Icon;
	if (name?.includes("Google")) return DriveGoogleDriveIcon;
	if (name?.includes("Dropbox")) return DriveDropboxIcon;

	// By type
	if (volumeTypeStr === "Cloud") return DriveIcon;
	if (volumeTypeStr === "Network") return ServerIcon;
	if (volumeTypeStr === "Virtual") return DatabaseIcon;
	return HDDIcon;
}

function getDiskTypeLabel(diskType: string): string {
	return diskType === "SSD" ? "SSD" : diskType === "HDD" ? "HDD" : diskType;
}

interface DevicePanelProps {
	onLocationSelect?: (location: Location | null) => void;
}

export function DevicePanel({ onLocationSelect }: DevicePanelProps = {}) {
	const [selectedLocationId, setSelectedLocationId] = useState<string | null>(
		null,
	);

	// Fetch all volumes using normalized cache
	const { data: volumesData, isLoading: volumesLoading } = useNormalizedQuery<
		VolumeListQueryInput,
		VolumeListOutput
	>({
		wireMethod: "query:volumes.list",
		input: { filter: "All" },
		resourceType: "volume",
	});

	// Fetch all devices using normalized cache
	const { data: devicesData, isLoading: devicesLoading } = useNormalizedQuery<
		ListLibraryDevicesInput,
		Device[]
	>({
		wireMethod: "query:devices.list",
		input: { include_offline: true, include_details: false },
		resourceType: "device",
	});

	// Fetch all locations using normalized cache
	const { data: locationsData, isLoading: locationsLoading } =
		useNormalizedQuery<LocationsListQueryInput, LocationsListOutput>({
			wireMethod: "query:locations.list",
			input: null,
			resourceType: "location",
		});

	// Get all jobs with real-time updates (local jobs)
	const { jobs: localJobs } = useJobs();

	// Get remote device jobs
	// TODO: This should have its own hook like useJobs, this will not work reactively
	const { data: remoteJobsData } = useCoreQuery({
		type: "jobs.remote.all_devices",
		input: {},
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
						action_context: null,
					}))
			: []),
	] as JobListItem[];

	if (volumesLoading || devicesLoading || locationsLoading) {
		return (
			<div className="bg-app-box border border-app-line rounded-xl overflow-hidden">
				<div className="px-6 py-4 border-b border-app-line">
					<h2 className="text-base font-semibold text-ink">
						Storage Volumes
					</h2>
					<p className="text-sm text-ink-dull mt-1">
						Loading volumes...
					</p>
				</div>
			</div>
		);
	}

	const volumes = volumesData?.volumes || [];
	const devices = devicesData || [];
	const locations = locationsData?.locations || [];

	// Filter to only show user-visible volumes
	const userVisibleVolumes = volumes.filter(
		(volume) => volume.is_user_visible !== false,
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
		{} as Record<string, VolumeItem[]>,
	);

	// Group locations by device slug
	const locationsByDeviceSlug = locations.reduce(
		(acc, location) => {
			// Extract device_slug from sd_path
			if (
				typeof location.sd_path === "object" &&
				"Physical" in location.sd_path
			) {
				const deviceSlug = location.sd_path.Physical.device_slug;
				if (!acc[deviceSlug]) {
					acc[deviceSlug] = [];
				}
				acc[deviceSlug].push(location);
			}
			return acc;
		},
		{} as Record<string, Location[]>,
	);

	// Create device map for quick lookup
	const deviceMap = devices.reduce(
		(acc, device) => {
			acc[device.id] = device;
			return acc;
		},
		{} as Record<string, Device>,
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
		{} as Record<string, JobListItem[]>,
	);

	const breakpointColumns = {
		default: 3,
		1600: 2,
		1000: 1,
	};

	return (
		<div className="">
			<Masonry
				breakpointCols={breakpointColumns}
				className="flex -ml-4 w-auto"
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
					<div className="bg-app-box border border-app-line rounded-xl overflow-hidden">
						<div className="text-center py-12 text-ink-faint">
							<HardDrive className="size-12 mx-auto mb-3 opacity-20" />
							<p className="text-sm">No devices detected</p>
							<p className="text-xs mt-1">
								Pair a device to get started
							</p>
						</div>
					</div>
				)}
			</Masonry>
		</div>
	);
}

interface DeviceCardProps {
	device?: Device;
	volumes: VolumeItem[];
	jobs: JobListItem[];
	locations: Location[];
	selectedLocationId: string | null;
	onLocationSelect?: (location: Location | null) => void;
}

function DeviceCard({
	device,
	volumes,
	jobs,
	locations,
	selectedLocationId,
	onLocationSelect,
}: DeviceCardProps) {
	const deviceName = device?.name || "Unknown Device";
	const deviceIconSrc = device ? getDeviceIcon(device) : null;
	const { pause, resume } = useJobs();

	// Format hardware specs
	const cpuInfo = device?.cpu_model
		? `${device.cpu_model}${device.cpu_physical_cores ? ` � ${device.cpu_physical_cores}C` : ""}`
		: null;
	const ramInfo = device?.memory_total
		? formatBytes(device.memory_total)
		: null;
	// Convert form_factor enum to string
	const formFactor = device?.form_factor
		? typeof device.form_factor === "string"
			? device.form_factor
			: (device.form_factor as any)?.Other ||
				JSON.stringify(device.form_factor)
		: null;
	const manufacturer = device?.manufacturer;

	// Filter active jobs
	const activeJobs = jobs.filter(
		(j) => j.status === "running" || j.status === "paused",
	);

	return (
		<div className="bg-app-darkBox border border-app-line overflow-hidden rounded-xl mb-4">
			{/* Device Header */}
			<div className="px-6 py-4 bg-app-box border-b border-app-line">
				<div className="flex items-center gap-4">
					{/* Left: Device icon and name */}
					<div className="flex items-center gap-3 flex-1 min-w-0">
						{deviceIconSrc ? (
							<img
								src={deviceIconSrc}
								alt={deviceName}
								className="size-8 opacity-80 flex-shrink-0"
							/>
						) : (
							<HardDrive
								className="size-8 text-ink flex-shrink-0"
								weight="duotone"
							/>
						)}
						<div className="min-w-0">
							<h3 className="text-base font-semibold text-ink truncate">
								{deviceName}
							</h3>
							<p className="text-sm text-ink-dull">
								{volumes.length}{" "}
								{volumes.length === 1 ? "volume" : "volumes"}
								{device?.is_online === false && " � Offline"}
							</p>
						</div>
					</div>

					{/* Right: Hardware specs */}
					<div className="flex items-center gap-3 text-xs text-ink-dull">
						{manufacturer && formFactor && (
							<div className="text-right">
								<div className="font-medium text-ink">
									{manufacturer}
								</div>
								<div>{formFactor}</div>
							</div>
						)}
						{cpuInfo && (
							<div className="text-right">
								<div
									className="font-medium text-ink truncate max-w-[180px]"
									title={cpuInfo}
								>
									{device?.cpu_model || "CPU"}
								</div>
								<div>
									{device?.cpu_physical_cores}C /{" "}
									{device?.cpu_cores_logical}T
								</div>
							</div>
						)}
						{ramInfo && (
							<div className="text-right">
								<div className="font-medium text-ink">
									{ramInfo}
								</div>
								<div>RAM</div>
							</div>
						)}
					</div>
				</div>
			</div>

			<div>
				{/* Active Jobs Section */}
				{activeJobs.length > 0 && (
					<div className="px-3 py-3 border-b border-app-line bg-app/50 space-y-2">
						{activeJobs.map((job) => (
							<JobCard
								key={job.id}
								job={job}
								onPause={pause}
								onResume={resume}
							/>
						))}
					</div>
				)}

				{/* Locations for this device */}
				{locations.length > 0 && (
					<LocationsScroller
						locations={locations}
						selectedLocationId={selectedLocationId}
						onLocationSelect={onLocationSelect}
					/>
				)}

				{/* Volumes for this device */}
				<div className="px-3 py-3 space-y-3">
					{volumes.length > 0 ? (
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
								<HardDrive className="size-8 mx-auto mb-2 opacity-20" />
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
	onLocationSelect,
}: LocationsScrollerProps) {
	const scrollRef = useRef<HTMLDivElement>(null);
	const [canScrollLeft, setCanScrollLeft] = useState(false);
	const [canScrollRight, setCanScrollRight] = useState(false);

	const updateScrollState = () => {
		if (!scrollRef.current) return;
		const { scrollLeft, scrollWidth, clientWidth } = scrollRef.current;
		setCanScrollLeft(scrollLeft > 0);
		setCanScrollRight(scrollLeft < scrollWidth - clientWidth - 1);
	};

	useEffect(() => {
		updateScrollState();
		window.addEventListener("resize", updateScrollState);
		return () => window.removeEventListener("resize", updateScrollState);
	}, [locations]);

	const scroll = (direction: "left" | "right") => {
		if (!scrollRef.current) return;
		const scrollAmount = 200;
		scrollRef.current.scrollBy({
			left: direction === "left" ? -scrollAmount : scrollAmount,
			behavior: "smooth",
		});
	};

	return (
		<div className="px-3 py-3 border-b border-app-line">
			<div className="relative">
				{/* Left fade and button */}
				{canScrollLeft && (
					<>
						<div className="absolute left-0 top-0 bottom-0 w-12 bg-gradient-to-r from-app-darkBox to-transparent z-10 pointer-events-none" />
						<div className="absolute left-1 top-1/2 -translate-y-1/2 z-20">
							<TopBarButton
								icon={CaretLeft}
								onClick={() => scroll("left")}
							/>
						</div>
					</>
				)}

				{/* Scrollable container */}
				<div
					ref={scrollRef}
					onScroll={updateScrollState}
					className="flex gap-2 overflow-x-auto scrollbar-hide"
					style={{ scrollbarWidth: "none" }}
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
								className="flex flex-col items-center gap-2 p-1 rounded-lg transition-all min-w-[80px] flex-shrink-0"
							>
								<div
									className={clsx(
										"rounded-lg p-2",
										isSelected
											? "bg-app-box"
											: "bg-transparent",
									)}
								>
									<img
										src={LocationIcon}
										alt={location.name}
										className="size-12 opacity-80"
									/>
								</div>
								<div className="w-full flex flex-col items-center">
									<div
										className={clsx(
											"text-xs truncate px-2 py-0.5 rounded-md inline-block max-w-full",
											isSelected
												? "bg-accent text-white"
												: "text-ink",
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
						<div className="absolute right-0 top-0 bottom-0 w-12 bg-gradient-to-l from-app-darkBox to-transparent z-10 pointer-events-none" />
						<div className="absolute right-1 top-1/2 -translate-y-1/2 z-20">
							<TopBarButton
								icon={CaretRight}
								onClick={() => scroll("right")}
							/>
						</div>
					</>
				)}
			</div>
		</div>
	);
}

interface VolumeBarProps {
	volume: VolumeItem;
	index: number;
}

function VolumeBar({ volume, index }: VolumeBarProps) {
	const trackVolume = useLibraryMutation("volumes.track");
	const indexVolume = useLibraryMutation("volumes.index");

	// Get current device to check if this volume is local
	const { data: currentDevice } = useCoreQuery({
		type: "devices.current",
		input: null,
	});

	const handleTrack = async () => {
		try {
			await trackVolume.mutateAsync({
				fingerprint: volume.fingerprint,
			});
		} catch (error) {
			console.error("Failed to track volume:", error);
		}
	};

	const handleIndex = async () => {
		try {
			const result = await indexVolume.mutateAsync({
				fingerprint: volume.fingerprint,
				scope: "Recursive",
			});
			console.log("Volume indexed:", result.message);
		} catch (error) {
			console.error("Failed to index volume:", error);
		}
	};

	if (!volume.total_capacity) {
		return null;
	}

	const totalCapacity = volume.total_capacity;
	const availableBytes = volume.available_capacity || 0;
	const usedBytes = totalCapacity - availableBytes;

	const uniqueBytes = volume.unique_bytes ?? Math.floor(usedBytes * 0.7);
	const duplicateBytes = usedBytes - uniqueBytes;

	const usagePercent = (usedBytes / totalCapacity) * 100;
	const uniquePercent = (uniqueBytes / totalCapacity) * 100;
	const duplicatePercent = (duplicateBytes / totalCapacity) * 100;

	// Convert enum values to strings for safe rendering
	const fileSystem = volume.file_system
		? typeof volume.file_system === "string"
			? volume.file_system
			: (volume.file_system as any)?.Other ||
				JSON.stringify(volume.file_system)
		: "Unknown";
	const diskType = volume.disk_type
		? typeof volume.disk_type === "string"
			? volume.disk_type
			: (volume.disk_type as any)?.Other ||
				JSON.stringify(volume.disk_type)
		: "Unknown";
	const readSpeed = volume.read_speed_mbps;

	const iconSrc = getVolumeIcon(volume.volume_type, volume.name);
	const volumeTypeStr =
		typeof volume.volume_type === "string"
			? volume.volume_type
			: (volume.volume_type as any)?.Other ||
				JSON.stringify(volume.volume_type);

	return (
		<motion.div
			initial={{ opacity: 0, y: 10 }}
			animate={{ opacity: 1, y: 0 }}
			transition={{ delay: index * 0.05 }}
			className="rounded-lg bg-app-box border border-app-line/50 overflow-hidden"
		>
			{/* Top row: Info */}
			<div className="flex items-center gap-3 px-3 py-2">
				{/* Icon */}
				<img
					src={iconSrc}
					alt={volumeTypeStr}
					className="size-6 opacity-80 flex-shrink-0"
				/>

				{/* Name, actions, and badges */}
				<div className="min-w-0 flex-1">
					<div className="flex items-center gap-2 mb-1">
						<span className="font-semibold text-ink truncate text-sm">
							{volume.display_name || volume.name}
						</span>
						{!volume.is_online && (
							<span className="px-1.5 py-0.5 bg-app-box text-ink-faint text-[10px] rounded border border-app-line">
								Offline
							</span>
						)}
						{!volume.is_tracked && (
							<button
								onClick={handleTrack}
								disabled={trackVolume.isPending}
								className="px-1.5 py-0.5 bg-accent/10 hover:bg-accent/20 text-accent text-[10px] rounded border border-accent/20 hover:border-accent/30 transition-colors flex items-center gap-1 disabled:opacity-50"
								title="Track this volume"
							>
								<Plus className="size-2.5" weight="bold" />
								{trackVolume.isPending
									? "Tracking..."
									: "Track"}
							</button>
						)}
						{currentDevice &&
							volume.device_id === currentDevice.id && (
								<button
									onClick={handleIndex}
									disabled={indexVolume.isPending}
									className="px-1.5 py-0.5 bg-sidebar-box hover:bg-sidebar-selected text-sidebar-ink text-[10px] rounded border border-sidebar-line transition-colors flex items-center gap-1 disabled:opacity-50"
									title="Index this volume"
								>
									<Database
										className="size-2.5"
										weight="bold"
									/>
									{indexVolume.isPending
										? "Indexing..."
										: "Index"}
								</button>
							)}
					</div>

					{/* Badges under name */}
					<div className="flex items-center gap-1.5 text-[10px] text-ink-dull flex-wrap">
						<span className="px-1.5 py-0.5 bg-app-box rounded border border-app-line">
							{fileSystem}
						</span>
						<span className="px-1.5 py-0.5 bg-app-box rounded border border-app-line">
							{getDiskTypeLabel(diskType)}
						</span>
						<span className="px-1.5 py-0.5 bg-app-box rounded border border-app-line">
							{volumeTypeStr}
						</span>
						{volume.total_file_count != null && (
							<span className="px-1.5 py-0.5 bg-accent/10 rounded border border-accent/20 text-accent">
								{volume.total_file_count.toLocaleString()} files
							</span>
						)}
					</div>
				</div>

				{/* Capacity info */}
				<div className="text-right flex-shrink-0">
					<div className="text-sm font-medium text-ink">
						{formatBytes(totalCapacity)}
					</div>
					<div className="text-[10px] text-ink-dull">
						{formatBytes(availableBytes)} free
					</div>
				</div>
			</div>

			{/* Bottom: Full-width capacity bar with padding */}
			<div className="px-3 pb-3 pt-2">
				<div className="h-8 bg-app rounded-md overflow-hidden border border-app-line">
					<div className="h-full flex">
						<motion.div
							initial={{ width: 0 }}
							animate={{ width: `${uniquePercent}%` }}
							transition={{
								duration: 1,
								ease: "easeOut",
								delay: index * 0.05,
							}}
							className="bg-accent border-r border-accent-deep"
							title={`Unique: ${formatBytes(uniqueBytes)} (${uniquePercent.toFixed(1)}%)`}
						/>
						<motion.div
							initial={{ width: 0 }}
							animate={{ width: `${duplicatePercent}%` }}
							transition={{
								duration: 1,
								ease: "easeOut",
								delay: index * 0.05 + 0.2,
							}}
							className="bg-accent/60"
							style={{
								backgroundImage:
									"repeating-linear-gradient(45deg, transparent, transparent 4px, rgba(255,255,255,0.1) 4px, rgba(255,255,255,0.1) 8px)",
							}}
							title={`Duplicate: ${formatBytes(duplicateBytes)} (${duplicatePercent.toFixed(1)}%)`}
						/>
					</div>
				</div>
			</div>
		</motion.div>
	);
}