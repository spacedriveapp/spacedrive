import { motion } from "framer-motion";
import clsx from "clsx";
import DriveIcon from "@sd/assets/icons/Drive.png";
import HDDIcon from "@sd/assets/icons/HDD.png";
import ServerIcon from "@sd/assets/icons/Server.png";
import DatabaseIcon from "@sd/assets/icons/Database.png";
import DriveAmazonS3Icon from "@sd/assets/icons/Drive-AmazonS3.png";
import DriveGoogleDriveIcon from "@sd/assets/icons/Drive-GoogleDrive.png";
import DriveDropboxIcon from "@sd/assets/icons/Drive-Dropbox.png";

interface StorageOverviewProps {
	volumes: any[];
	devices: any[];
}

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
	return diskType === "SSD"
		? "⚡ SSD"
		: diskType === "HDD"
			? "💿 HDD"
			: diskType;
}

export function StorageOverview({ volumes, devices }: StorageOverviewProps) {
	// Group volumes by device
	const volumesByDevice = volumes.reduce(
		(acc, vol) => {
			const deviceId = vol.device_id || "unknown";
			if (!acc[deviceId]) acc[deviceId] = [];
			acc[deviceId].push(vol);
			return acc;
		},
		{} as Record<string, any[]>,
	);

	return (
		<div className="bg-app-box border border-app-line rounded-xl overflow-hidden">
			<div className="px-6 py-4 border-b border-app-line">
				<h2 className="text-base font-semibold text-ink">
					Storage Overview
				</h2>
				<p className="text-sm text-ink-dull mt-1">
					All volumes across {Object.keys(volumesByDevice).length}{" "}
					devices
				</p>
			</div>

			<div className="p-6 space-y-6">
				{Object.entries(volumesByDevice).map(
					([deviceId, deviceVolumes]) => {
						const device = devices.find((d) => d.id === deviceId);
						const deviceName = device?.name || "Unknown Device";

						return (
							<div key={deviceId} className="space-y-3">
								{/* Device header */}
								<div className="flex items-center gap-2 px-2">
									<span className="text-sm font-medium text-ink-dull">
										{deviceName}
									</span>
									{device?.is_online && (
										<span className="size-2 rounded-full bg-green-500" />
									)}
								</div>

								{/* Volumes for this device */}
								<div className="space-y-2">
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

				{volumes.length === 0 && (
					<div className="text-center py-12 text-ink-faint">
						<HardDrive className="size-12 mx-auto mb-3 opacity-20" />
						<p className="text-sm">No volumes detected</p>
					</div>
				)}
			</div>
		</div>
	);
}

interface VolumeBarProps {
	volume: any;
	index: number;
}

function VolumeBar({ volume, index }: VolumeBarProps) {
	const usedBytes = volume.total_capacity - volume.available_space;
	const usagePercent =
		volume.total_capacity > 0
			? (usedBytes / volume.total_capacity) * 100
			: 0;

	const iconSrc = getVolumeIcon(volume.volume_type, volume.name);

	return (
		<motion.div
			initial={{ opacity: 0, x: -20 }}
			animate={{ opacity: 1, x: 0 }}
			transition={{ delay: index * 0.05 }}
			className="group flex items-center gap-4 p-3 hover:bg-app-selected rounded-lg transition-colors cursor-pointer"
		>
			<img
				src={iconSrc}
				alt={volume.volume_type}
				className="size-8 opacity-80"
			/>

			<div className="flex-1 min-w-0">
				{/* Name and stats */}
				<div className="flex items-center justify-between mb-2">
					<div className="flex items-center gap-2 min-w-0">
						<span className="font-medium text-ink truncate">
							{volume.name}
						</span>
						{!volume.is_mounted && (
							<span className="px-1.5 py-0.5 bg-sidebar-box text-sidebar-ink-dull text-xs rounded border border-sidebar-line">
								Offline
							</span>
						)}
					</div>
					<span className="text-sm text-ink-dull whitespace-nowrap ml-3">
						{formatBytes(usedBytes)} /{" "}
						{formatBytes(volume.total_capacity)}
					</span>
				</div>

				{/* Progress bar */}
				<div className="h-1.5 bg-sidebar-box/30 rounded-full overflow-hidden mb-2">
					<motion.div
						initial={{ width: 0 }}
						animate={{ width: `${usagePercent}%` }}
						transition={{
							duration: 1,
							ease: "easeOut",
							delay: index * 0.05,
						}}
						className="h-full bg-accent rounded-full"
					/>
				</div>

				{/* Badges */}
				<div className="flex items-center gap-2 text-xs text-ink-dull">
					<span>{getDiskTypeLabel(volume.disk_type)}</span>
					<span className="text-ink-faint">•</span>
					<span>{volume.file_system}</span>
					<span className="text-ink-faint">•</span>
					<span>{usagePercent.toFixed(0)}% used</span>

					{volume.read_speed_mbps && (
						<>
							<span className="text-ink-faint">•</span>
							<span className="ml-auto">
								{volume.read_speed_mbps} MB/s
							</span>
						</>
					)}
				</div>
			</div>
		</motion.div>
	);
}
