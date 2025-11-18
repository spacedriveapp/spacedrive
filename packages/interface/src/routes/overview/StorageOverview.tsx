import { motion } from "framer-motion";
import { HardDrive, Plus } from "@phosphor-icons/react";
import DriveIcon from "@sd/assets/icons/Drive.png";
import HDDIcon from "@sd/assets/icons/HDD.png";
import ServerIcon from "@sd/assets/icons/Server.png";
import DatabaseIcon from "@sd/assets/icons/Database.png";
import DriveAmazonS3Icon from "@sd/assets/icons/Drive-AmazonS3.png";
import DriveGoogleDriveIcon from "@sd/assets/icons/Drive-GoogleDrive.png";
import DriveDropboxIcon from "@sd/assets/icons/Drive-Dropbox.png";
import { useNormalizedCache, useLibraryMutation } from "../../context";
import type {
	VolumeListOutput,
	VolumeListQueryInput,
	VolumeItem,
	LibraryDeviceInfo,
	ListLibraryDevicesInput,
} from "@sd/ts-client/generated/types";

function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB", "PB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

function getVolumeColor(volumeType: string): string {
	const colors: Record<string, string> = {
		Primary: "from-blue-500 to-blue-600",
		External: "from-green-500 to-emerald-600",
		Cloud: "from-purple-500 to-violet-600",
		Network: "from-orange-500 to-amber-600",
		System: "from-gray-500 to-slate-600",
		Virtual: "from-cyan-500 to-sky-600",
		TimeMachine: "from-indigo-500 to-purple-600",
		Backup: "from-yellow-500 to-amber-600",
		Archive: "from-amber-600 to-orange-600",
	};
	return colors[volumeType] || "from-gray-500 to-slate-600";
}

function getVolumeIcon(volumeType: string, name?: string): string {
	// Check for cloud providers by name
	if (name?.includes("S3")) return DriveAmazonS3Icon;
	if (name?.includes("Google")) return DriveGoogleDriveIcon;
	if (name?.includes("Dropbox")) return DriveDropboxIcon;

	// By type
	if (volumeType === "Cloud") return DriveIcon;
	if (volumeType === "Network") return ServerIcon;
	if (volumeType === "Virtual") return DatabaseIcon;
	return HDDIcon;
}

function getDiskTypeLabel(diskType: string): string {
	return diskType === "SSD" ? "SSD" : diskType === "HDD" ? "HDD" : diskType;
}

export function StorageOverview() {
	// Fetch all volumes using normalized cache
	const { data: volumesData, isLoading: volumesLoading } = useNormalizedCache<
		VolumeListQueryInput,
		VolumeListOutput
	>({
		wireMethod: "query:volumes.list",
		input: { filter: "All" },
		resourceType: "volume",
	});

	// Fetch all devices using normalized cache
	const { data: devicesData, isLoading: devicesLoading } = useNormalizedCache<
		ListLibraryDevicesInput,
		LibraryDeviceInfo[]
	>({
		wireMethod: "query:devices.list",
		input: { include_offline: true, include_details: false },
		resourceType: "device",
	});

	if (volumesLoading || devicesLoading) {
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

	// Filter to only show user-visible volumes
	const userVisibleVolumes = volumes.filter(
		(volume) => volume.is_user_visible !== false,
	);

	// Group volumes by device - note: VolumeItem doesn't have device_id yet
	// So we'll just show all volumes ungrouped for now
	// TODO: Backend needs to add device_id to VolumeItem
	const volumesByDevice: Record<string, typeof userVisibleVolumes> = {};

	// For now, create a single "All Devices" group
	if (userVisibleVolumes.length > 0) {
		volumesByDevice["all"] = userVisibleVolumes;
	}

	return (
		<div className="bg-app-box border border-app-line rounded-xl overflow-hidden">
			<div className="px-6 py-4 border-b border-app-line">
				<h2 className="text-base font-semibold text-ink">
					Storage Volumes
				</h2>
				<p className="text-sm text-ink-dull mt-1">
					{userVisibleVolumes.length}{" "}
					{userVisibleVolumes.length === 1 ? "volume" : "volumes"}{" "}
					across {devices.length}{" "}
					{devices.length === 1 ? "device" : "devices"}
				</p>
			</div>

			<div className="p-6 space-y-6">
				{Object.entries(volumesByDevice).map(
					([deviceId, deviceVolumes]) => {
						return (
							<div key={deviceId} className="space-y-3">
								{/* Volumes */}
								<div className="space-y-3">
									{deviceVolumes.map((volume, idx) => (
										<VolumeBar
											key={volume.id}
											volume={volume}
											index={idx}
										/>
									))}
								</div>
							</div>
						);
					},
				)}

				{userVisibleVolumes.length === 0 && (
					<div className="text-center py-12 text-ink-faint">
						<HardDrive className="size-12 mx-auto mb-3 opacity-20" />
						<p className="text-sm">No volumes detected</p>
						<p className="text-xs mt-1">
							Track a volume to see storage information
						</p>
					</div>
				)}
			</div>
		</div>
	);
}

interface VolumeBarProps {
	volume: VolumeItem;
	index: number;
}

// Dummy data generator - used when backend doesn't provide data yet
function getDummyVolumeStats(volumeName: string) {
	const hash = volumeName
		.split("")
		.reduce((acc, char) => acc + char.charCodeAt(0), 0);
	const totalCapacity =
		[500, 1000, 2000, 4000][hash % 4] * 1024 * 1024 * 1024;
	const usedPercent = [0.3, 0.5, 0.7, 0.85, 0.95][hash % 5];
	const usedBytes = Math.floor(totalCapacity * usedPercent);
	const uniquePercent = [0.6, 0.7, 0.8, 0.9][hash % 4];
	const uniqueBytes = Math.floor(usedBytes * uniquePercent);

	return {
		totalCapacity,
		usedBytes,
		uniqueBytes,
		availableBytes: totalCapacity - usedBytes,
		fileSystem: ["APFS", "NTFS", "ext4", "exFAT"][hash % 4],
		diskType: ["SSD", "HDD", "NVMe"][hash % 3],
		readSpeed: [3500, 550, 120][hash % 3],
	};
}

function VolumeBar({ volume, index }: VolumeBarProps) {
	const trackVolume = useLibraryMutation("volumes.track");

	const handleTrack = async () => {
		try {
			await trackVolume.mutateAsync({
				fingerprint: volume.fingerprint,
			});
		} catch (error) {
			console.error("Failed to track volume:", error);
		}
	};

	// Use real data from backend, fallback to dummy data if not available
	const useDummyData = !volume.total_capacity;
	const dummy = useDummyData ? getDummyVolumeStats(volume.name) : null;

	const totalCapacity = volume.total_capacity || dummy!.totalCapacity;
	const availableBytes = volume.available_capacity || dummy!.availableBytes;
	const usedBytes = totalCapacity - availableBytes;

	// Calculate unique bytes - if backend provides it, use it; otherwise estimate
	const uniqueBytes =
		volume.unique_bytes !== null
			? volume.unique_bytes
			: useDummyData
				? dummy!.uniqueBytes
				: Math.floor(usedBytes * 0.7);

	const duplicateBytes = usedBytes - uniqueBytes;

	const usagePercent = (usedBytes / totalCapacity) * 100;
	const uniquePercent = (uniqueBytes / totalCapacity) * 100;
	const duplicatePercent = (duplicateBytes / totalCapacity) * 100;

	const fileSystem = volume.file_system || dummy?.fileSystem || "Unknown";
	const diskType = volume.disk_type || dummy?.diskType || "Unknown";
	const readSpeed = volume.read_speed_mbps || dummy?.readSpeed;

	const iconSrc = getVolumeIcon(volume.volume_type, volume.name);

	return (
		<motion.div
			initial={{ opacity: 0, y: 10 }}
			animate={{ opacity: 1, y: 0 }}
			transition={{ delay: index * 0.05 }}
			className="group p-4 hover:bg-app-selected/50 rounded-lg transition-all cursor-pointer border border-transparent hover:border-app-line"
		>
			<div className="flex items-start gap-4">
				<img
					src={iconSrc}
					alt={volume.volume_type}
					className="size-10 opacity-80 mt-1"
				/>

				<div className="flex-1 min-w-0">
					{/* Header */}
					<div className="flex items-center justify-between mb-3">
						<div className="flex items-center gap-2 min-w-0">
							<span className="font-semibold text-ink truncate text-base">
								{volume.name}
							</span>
							{!volume.is_online && (
								<span className="px-2 py-0.5 bg-app-box text-ink-faint text-xs rounded-md border border-app-line">
									Offline
								</span>
							)}
							{!volume.is_tracked && (
								<button
									onClick={handleTrack}
									disabled={trackVolume.isPending}
									className="px-2 py-0.5 bg-accent/10 hover:bg-accent/20 text-accent text-xs rounded-md border border-accent/20 hover:border-accent/30 transition-colors flex items-center gap-1 disabled:opacity-50 disabled:cursor-not-allowed"
									title="Track this volume to enable deduplication and search"
								>
									<Plus className="size-3" weight="bold" />
									{trackVolume.isPending ? "Tracking..." : "Track"}
								</button>
							)}
							{useDummyData && (
								<span className="px-2 py-0.5 bg-yellow-500/10 text-yellow-600 text-xs rounded-md border border-yellow-500/20">
									Demo Data
								</span>
							)}
						</div>
						<div className="text-right">
							<div className="text-sm font-medium text-ink">
								{formatBytes(totalCapacity)}
							</div>
							<div className="text-xs text-ink-dull">
								{formatBytes(availableBytes)} free
							</div>
						</div>
					</div>

					{/* Windows-style thick capacity bar */}
					<div className="mb-3">
						<div className="h-8 bg-app-darkBox rounded-md overflow-hidden border border-app-line shadow-inner">
							<div className="h-full flex">
								{/* Unique bytes - solid blue */}
								<motion.div
									initial={{ width: 0 }}
									animate={{ width: `${uniquePercent}%` }}
									transition={{
										duration: 1,
										ease: "easeOut",
										delay: index * 0.05,
									}}
									className="bg-gradient-to-b from-blue-500 to-blue-600 border-r border-blue-400/30"
									title={`Unique: ${formatBytes(uniqueBytes)}`}
								/>
								{/* Duplicate bytes - striped pattern */}
								<motion.div
									initial={{ width: 0 }}
									animate={{ width: `${duplicatePercent}%` }}
									transition={{
										duration: 1,
										ease: "easeOut",
										delay: index * 0.05 + 0.2,
									}}
									className="bg-gradient-to-b from-blue-400 to-blue-500 relative overflow-hidden"
									style={{
										backgroundImage:
											"repeating-linear-gradient(45deg, transparent, transparent 4px, rgba(255,255,255,0.1) 4px, rgba(255,255,255,0.1) 8px)",
									}}
									title={`Duplicate: ${formatBytes(duplicateBytes)}`}
								/>
							</div>
						</div>
					</div>

					{/* Stats row */}
					<div className="flex items-center gap-4 text-xs">
						<div className="flex items-center gap-1.5">
							<div className="size-3 rounded bg-gradient-to-b from-blue-500 to-blue-600" />
							<span className="text-ink-dull">
								Unique: {formatBytes(uniqueBytes)}
							</span>
						</div>
						<div className="flex items-center gap-1.5">
							<div
								className="size-3 rounded bg-gradient-to-b from-blue-400 to-blue-500"
								style={{
									backgroundImage:
										"repeating-linear-gradient(45deg, transparent, transparent 2px, rgba(255,255,255,0.2) 2px, rgba(255,255,255,0.2) 4px)",
								}}
							/>
							<span className="text-ink-dull">
								Duplicate: {formatBytes(duplicateBytes)}
							</span>
						</div>
						<span className="text-ink-faint">•</span>
						<span className="text-ink-dull">
							{usagePercent.toFixed(1)}% used
						</span>
						{volume.mount_point && (
							<>
								<span className="text-ink-faint">•</span>
								<span className="text-ink-faint truncate">
									{volume.mount_point}
								</span>
							</>
						)}
					</div>

					{/* Bottom badges */}
					<div className="flex items-center gap-2 text-xs text-ink-dull mt-2">
						<span className="px-2 py-0.5 bg-app-box rounded border border-app-line">
							{fileSystem}
						</span>
						<span className="px-2 py-0.5 bg-app-box rounded border border-app-line">
							{getDiskTypeLabel(diskType)}
						</span>
						{readSpeed && (
							<span className="px-2 py-0.5 bg-app-box rounded border border-app-line">
								{readSpeed} MB/s
							</span>
						)}
						<span className="px-2 py-0.5 bg-app-box rounded border border-app-line">
							{volume.volume_type}
						</span>
					</div>
				</div>
			</div>
		</motion.div>
	);
}
