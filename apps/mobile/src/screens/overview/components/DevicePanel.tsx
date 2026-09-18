import React, { useState, useEffect } from "react";
import { View, Text, Image, Pressable, Modal } from "react-native";
import DatabaseIcon from "@sd/assets/icons/Database.webp";
import DriveAmazonS3Icon from "@sd/assets/icons/Drive-AmazonS3.webp";
import DriveDropboxIcon from "@sd/assets/icons/Drive-Dropbox.webp";
import DriveGoogleDriveIcon from "@sd/assets/icons/Drive-GoogleDrive.webp";
import DriveIcon from "@sd/assets/icons/Drive.webp";
import HDDIcon from "@sd/assets/icons/HDD.webp";
import ServerIcon from "@sd/assets/icons/Server.webp";
import type {
	Device,
	JobListItem,
	Volume,
} from "@sd/ts-client";
import { getDeviceIcon } from "@sd/ts-client";
import { useNormalizedQuery, useCoreQuery, useLibraryAction, useSpacedriveClient } from "../../../client";
import { useVolumeIndexingStore } from "../../../stores";

// Temporary type extension
type DeviceWithConnection = Device & {
	connection_method?: "LocalNetwork" | "DirectInternet" | "RelayProxy" | null;
};

function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB", "PB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

function getVolumeIcon(volumeType: any, name?: string): any {
	const volumeTypeStr =
		typeof volumeType === "string"
			? volumeType
			: volumeType?.Other || JSON.stringify(volumeType);

	if (name?.includes("S3")) return DriveAmazonS3Icon;
	if (name?.includes("Google")) return DriveGoogleDriveIcon;
	if (name?.includes("Dropbox")) return DriveDropboxIcon;

	if (volumeTypeStr === "Cloud") return DriveIcon;
	if (volumeTypeStr === "Network") return ServerIcon;
	if (volumeTypeStr === "Virtual") return DatabaseIcon;
	return HDDIcon;
}

function getDiskTypeLabel(diskType: string): string {
	return diskType === "SSD" ? "SSD" : diskType === "HDD" ? "HDD" : diskType;
}

export function DevicePanel() {

	// Fetch all volumes
	const { data: volumesData, isLoading: volumesLoading } = useNormalizedQuery<
		any,
		any
	>({
		query: "volumes.list",
		input: { filter: "All" },
		resourceType: "volume",
	});

	// Fetch all devices
	const { data: devicesData, isLoading: devicesLoading } = useNormalizedQuery<
		any,
		DeviceWithConnection[]
	>({
		query: "devices.list",
		input: { include_offline: true, include_details: false },
		resourceType: "device",
	});

	// TODO: Get jobs when mobile supports it
	const allJobs: JobListItem[] = [];

	if (volumesLoading || devicesLoading) {
		return (
			<View className="bg-app-box border border-app-line rounded-xl overflow-hidden mb-6">
				<View className="px-6 py-4 border-b border-app-line">
					<Text className="text-base font-semibold text-ink">
						Storage Volumes
					</Text>
					<Text className="text-sm text-ink-dull mt-1">
						Loading volumes...
					</Text>
				</View>
			</View>
		);
	}

	const volumes = volumesData?.volumes || [];
	const devices = devicesData || [];

	// Filter to only show user-visible volumes
	const userVisibleVolumes = volumes.filter(
		(volume: any) => volume.is_user_visible !== false
	);

	// Group volumes by device_id
	const volumesByDevice = userVisibleVolumes.reduce((acc: any, volume: any) => {
		const deviceId = volume.device_id;
		if (!acc[deviceId]) {
			acc[deviceId] = [];
		}
		acc[deviceId].push(volume);
		return acc;
	}, {} as Record<string, Volume[]>);

	// Group jobs by device_id
	const jobsByDevice = allJobs.reduce((acc: any, job: JobListItem) => {
		const deviceId = job.device_id;
		if (!acc[deviceId]) {
			acc[deviceId] = [];
		}
		acc[deviceId].push(job);
		return acc;
	}, {} as Record<string, JobListItem[]>);

	return (
		<View className="mb-6">

			{devices.map((device: DeviceWithConnection) => {
				const deviceVolumes = volumesByDevice[device.id] || [];
				const deviceJobs = jobsByDevice[device.id] || [];

				return (
					<DeviceCard
						key={device.id}
						device={device}
						volumes={deviceVolumes}
						jobs={deviceJobs}
					/>
				);
			})}

			{devices.length === 0 && (
				<View className="border border-app-line/30 rounded-xl overflow-hidden">
					<View className="py-12 items-center">
						<Text className="text-4xl opacity-20 mb-3">💾</Text>
						<Text className="text-sm text-ink-faint">No devices detected</Text>
						<Text className="text-xs text-ink-faint mt-1">
							Pair a device to get started
						</Text>
					</View>
				</View>
			)}
		</View>
	);
}

interface ConnectionBadgeProps {
	method: "LocalNetwork" | "DirectInternet" | "RelayProxy";
}

function ConnectionBadge({ method }: ConnectionBadgeProps) {
	const labels = {
		LocalNetwork: "Local",
		DirectInternet: "Direct",
		RelayProxy: "Relay",
	};

	return (
		<View className="flex-row items-center gap-1.5">
			<View className="w-2 h-2 rounded-full bg-ink-dull" />
			<Text className="text-ink-dull text-xs font-medium">{labels[method]}</Text>
		</View>
	);
}

interface DeviceCardProps {
	device?: DeviceWithConnection;
	volumes: Volume[];
	jobs: JobListItem[];
}

function DeviceCard({
	device,
	volumes,
	jobs,
}: DeviceCardProps) {
	const deviceName = device?.name || "Unknown Device";
	const deviceIconSrc = device ? getDeviceIcon(device) : null;

	const cpuInfo = device?.cpu_model
		? `${device.cpu_model}${device.cpu_cores_physical ? ` • ${device.cpu_cores_physical}C` : ""}`
		: null;
	const ramInfo = device?.memory_total_bytes
		? formatBytes(device.memory_total_bytes)
		: null;

	const activeJobs = jobs.filter(
		(j) => j.status === "running" || j.status === "paused"
	);

	return (
		<View className="border border-app-line/30 mb-6 rounded-xl overflow-hidden">
			{/* Device Header */}
			<View className="border-b border-app-line/30 px-6 py-4">
				<View className="flex-row items-center gap-4">
					{/* Left: Device icon and name */}
					<View className="flex-1 flex-row items-center gap-3">
						{deviceIconSrc ? (
							<Image
								source={deviceIconSrc}
								className="w-8 h-8 opacity-80"
								style={{ resizeMode: "contain" }}
							/>
						) : (
							<Text className="text-2xl">💻</Text>
						)}
						<View className="flex-1">
							<View className="flex-row items-center gap-2">
								<Text className="text-ink text-base font-semibold flex-shrink">
									{deviceName}
								</Text>
								{device?.connection_method && (
									<ConnectionBadge method={device.connection_method} />
								)}
							</View>
							<Text className="text-ink-dull text-sm">
								{volumes.length} {volumes.length === 1 ? "volume" : "volumes"}
								{device?.is_online === false && " • Offline"}
							</Text>
						</View>
					</View>

					{/* Right: Hardware specs */}
					<View className="gap-1.5">
						{cpuInfo && (
							<Text className="text-ink text-right text-xs font-medium">
								{device?.cpu_model || "CPU"}
							</Text>
						)}
						<View className="flex-row items-center justify-end gap-3">
							{device?.cpu_cores_physical && (
								<View className="flex-row items-center gap-1">
									<Text className="text-ink-dull text-[11px]">
										{Math.max(
											device.cpu_cores_physical || 0,
											device.cpu_cores_logical || 0
										)}C
									</Text>
								</View>
							)}
							{ramInfo && (
								<View className="flex-row items-center gap-1">
									<Text className="text-ink-dull text-[11px]">{ramInfo}</Text>
								</View>
							)}
						</View>
					</View>
				</View>
			</View>

			{/* Active Jobs Section */}
			{activeJobs.length > 0 && (
				<View className="border-b border-app-line/30 px-3 py-3 gap-2">
					{/* TODO: Add JobCard component when ported */}
					<Text className="text-ink-dull text-xs">
						{activeJobs.length} active {activeJobs.length === 1 ? "job" : "jobs"}
					</Text>
				</View>
			)}

			{/* Volumes for this device */}
			<View className="px-3 py-3 gap-3">
				{volumes.length > 0 ? (
					volumes.map((volume, idx) => (
						<VolumeBar key={volume.id} volume={volume} index={idx} />
					))
				) : (
					<View className="py-8 items-center justify-center">
						<Text className="text-xs text-ink-faint">No volumes</Text>
					</View>
				)}
			</View>
		</View>
	);
}

interface VolumeMenuProps {
	volume: Volume;
	visible: boolean;
	onClose: () => void;
}

function VolumeMenu({ volume, visible, onClose }: VolumeMenuProps) {
	const trackVolume = useLibraryAction("volumes.track");
	const untrackVolume = useLibraryAction("volumes.untrack");
	const indexVolume = useLibraryAction("volumes.index");
	const speedTestVolume = useLibraryAction("volumes.speed_test");
	const ejectVolume = useLibraryAction("volumes.eject");

	const isRemovable = volume.mount_type === "External";

	const handleTrack = async () => {
		try {
			await trackVolume.mutateAsync({
				fingerprint: volume.fingerprint,
				display_name: null,
			});
			onClose();
		} catch (error) {
			console.error("Failed to track volume:", error);
		}
	};

	const handleUntrack = async () => {
		try {
			await untrackVolume.mutateAsync({
				volume_id: volume.id,
			});
			onClose();
		} catch (error) {
			console.error("Failed to untrack volume:", error);
		}
	};

	const handleIndex = async () => {
		try {
			const result = await indexVolume.mutateAsync({
				fingerprint: volume.fingerprint,
				scope: "Recursive",
			});
			console.log("Volume indexed:", result.message);
			onClose();
		} catch (error) {
			console.error("Failed to index volume:", error);
		}
	};

	const handleSpeedTest = async () => {
		try {
			const result = await speedTestVolume.mutateAsync({
				fingerprint: volume.fingerprint,
			});
			console.log(
				"Speed test complete:",
				result.read_speed_mbps,
				"MB/s read,",
				result.write_speed_mbps,
				"MB/s write"
			);
			onClose();
		} catch (error) {
			console.error("Failed to run speed test:", error);
		}
	};

	const handleEject = async () => {
		try {
			const result = await ejectVolume.mutateAsync({
				fingerprint: volume.fingerprint,
			});
			if (result.success) {
				console.log("Volume ejected successfully");
			} else {
				console.error("Eject failed:", result.message);
			}
			onClose();
		} catch (error) {
			console.error("Failed to eject volume:", error);
		}
	};

	return (
		<Modal
			visible={visible}
			transparent
			animationType="fade"
			onRequestClose={onClose}
		>
			<Pressable
				className="flex-1 bg-black/50 justify-end"
				onPress={onClose}
			>
				<Pressable
					className="bg-app-darkBox rounded-t-2xl"
					onPress={(e) => e.stopPropagation()}
				>
					<View className="px-4 py-6 gap-2">
						<Text className="text-ink text-lg font-semibold mb-2">
							{volume.display_name || volume.name}
						</Text>

						{!volume.is_tracked && (
							<Pressable
								onPress={handleTrack}
								disabled={trackVolume.isPending}
								className="flex-row items-center gap-3 px-4 py-3 rounded-lg active:bg-app-box"
							>
								<Text className="text-2xl">👁️</Text>
								<Text className="text-ink text-base flex-1">
									Track Volume
								</Text>
								{trackVolume.isPending && (
									<Text className="text-ink-dull text-sm">Loading...</Text>
								)}
							</Pressable>
						)}

						{volume.is_tracked && (
							<Pressable
								onPress={handleUntrack}
								disabled={untrackVolume.isPending}
								className="flex-row items-center gap-3 px-4 py-3 rounded-lg active:bg-app-box"
							>
								<Text className="text-2xl">🚫</Text>
								<Text className="text-red-500 text-base flex-1">
									Untrack Volume
								</Text>
								{untrackVolume.isPending && (
									<Text className="text-ink-dull text-sm">Loading...</Text>
								)}
							</Pressable>
						)}

						<View className="border-t border-app-line my-1" />

						{volume.is_mounted && (
							<Pressable
								onPress={handleIndex}
								disabled={indexVolume.isPending}
								className="flex-row items-center gap-3 px-4 py-3 rounded-lg active:bg-app-box"
							>
								<Text className="text-2xl">🗄️</Text>
								<Text className="text-ink text-base flex-1">
									Index Volume
								</Text>
								{indexVolume.isPending && (
									<Text className="text-ink-dull text-sm">Loading...</Text>
								)}
							</Pressable>
						)}

						{volume.is_mounted && (
							<Pressable
								onPress={handleSpeedTest}
								disabled={speedTestVolume.isPending}
								className="flex-row items-center gap-3 px-4 py-3 rounded-lg active:bg-app-box"
							>
								<Text className="text-2xl">⚡</Text>
								<Text className="text-ink text-base flex-1">
									Speed Test
								</Text>
								{speedTestVolume.isPending && (
									<Text className="text-ink-dull text-sm">Testing...</Text>
								)}
							</Pressable>
						)}

						{isRemovable && volume.is_mounted && (
							<Pressable
								onPress={handleEject}
								disabled={ejectVolume.isPending}
								className="flex-row items-center gap-3 px-4 py-3 rounded-lg active:bg-app-box"
							>
								<Text className="text-2xl">⏏️</Text>
								<Text className="text-ink text-base flex-1">
									Eject
								</Text>
								{ejectVolume.isPending && (
									<Text className="text-ink-dull text-sm">Ejecting...</Text>
								)}
							</Pressable>
						)}

						<View className="border-t border-app-line my-1" />

						<Pressable
							onPress={onClose}
							className="px-4 py-3 rounded-lg bg-app-box active:bg-app-hover"
						>
							<Text className="text-ink text-base text-center">Cancel</Text>
						</Pressable>
					</View>
				</Pressable>
			</Pressable>
		</Modal>
	);
}

interface VolumeBarProps {
	volume: Volume;
	index: number;
}

interface IndexingProgress {
	filesIndexed: number;
	bytesIndexed: number;
	percentage: number;
	rate: number;
}

function VolumeBar({ volume, index }: VolumeBarProps) {
	const [indexingProgress, setIndexingProgress] = useState<IndexingProgress | null>(null);
	const [menuVisible, setMenuVisible] = useState(false);
	const client = useSpacedriveClient();

	const jobId = useVolumeIndexingStore((state) =>
		state.getJobId(volume.fingerprint)
	);

	// Subscribe to job events for this volume
	useEffect(() => {
		if (!client) return;

		let unsubscribe: (() => void) | undefined;
		let isCancelled = false;

		const handleEvent = (event: any) => {
			const eventType = Object.keys(event)[0];

			// Client-side filter: only handle job events
			if (
				![
					'JobProgress',
					'JobCompleted',
					'JobFailed',
					'JobCancelled'
				].includes(eventType)
			) {
				return;
			}

			if ('JobProgress' in event) {
				const progressData = event.JobProgress;
				if (!progressData) return;

				// Read the current job ID from store (avoids stale closure)
				const currentJobId = useVolumeIndexingStore
					.getState()
					.getJobId(volume.fingerprint);

				// Only handle progress for this volume's job
				if (progressData.job_id !== currentJobId) return;

				const generic = progressData.generic_progress;
				if (!generic) return;

				setIndexingProgress({
					filesIndexed: generic.completion?.completed || 0,
					bytesIndexed: generic.completion?.bytes_completed || 0,
					percentage: generic.percentage || 0,
					rate: generic.performance?.rate || 0
				});
			} else if (
				'JobCompleted' in event ||
				'JobFailed' in event ||
				'JobCancelled' in event
			) {
				const eventJobId =
					event.JobCompleted?.job_id ||
					event.JobFailed?.job_id ||
					event.JobCancelled?.job_id;

				const currentJobId = useVolumeIndexingStore
					.getState()
					.getJobId(volume.fingerprint);

				if (eventJobId === currentJobId) {
					setIndexingProgress(null);
				}
			}
		};

		const filter = {
			event_types: [
				'JobProgress',
				'JobCompleted',
				'JobFailed',
				'JobCancelled'
			],
			resource_type: undefined,
			path_scope: undefined,
			library_id: undefined
		};

		client.subscribeFiltered(filter, handleEvent).then((unsub) => {
			if (isCancelled) {
				unsub();
			} else {
				unsubscribe = unsub;
			}
		});

		return () => {
			isCancelled = true;
			unsubscribe?.();
		};
	}, [client, volume.fingerprint]);

	if (!volume.total_capacity) {
		return null;
	}

	const totalCapacity = volume.total_capacity;
	const availableBytes = volume.available_space || 0;
	const usedBytes = totalCapacity - availableBytes;

	const uniqueBytes = volume.unique_bytes ?? Math.floor(usedBytes * 0.7);
	const duplicateBytes = usedBytes - uniqueBytes;

	const uniquePercent = (uniqueBytes / totalCapacity) * 100;
	const duplicatePercent = (duplicateBytes / totalCapacity) * 100;

	// Helper to filter out unknown values
	const filterUnknown = (value: string | null): string | null => {
		if (!value || value.toLowerCase() === 'unknown') return null;
		return value;
	};

	// Convert enum values to strings for safe rendering
	const fileSystem = filterUnknown(
		volume.file_system
			? typeof volume.file_system === "string"
				? volume.file_system
				: (volume.file_system as any)?.Other ||
					JSON.stringify(volume.file_system)
			: null
	);
	const diskType = filterUnknown(
		volume.disk_type
			? typeof volume.disk_type === "string"
				? volume.disk_type
				: (volume.disk_type as any)?.Other ||
					JSON.stringify(volume.disk_type)
			: null
	);

	const iconSrc = getVolumeIcon(volume.volume_type, volume.name);
	const volumeTypeStr = filterUnknown(
		typeof volume.volume_type === "string"
			? volume.volume_type
			: (volume.volume_type as any)?.Other ||
					JSON.stringify(volume.volume_type)
	);

	return (
		<>
			<VolumeMenu
				volume={volume}
				visible={menuVisible}
				onClose={() => setMenuVisible(false)}
			/>

			<View className="bg-app-box border border-app-line/50 rounded-lg overflow-hidden">
				{/* Top row: Info */}
				<View className="flex-row items-center gap-3 px-3 py-2">
					<Image
						source={iconSrc}
						className="w-6 h-6 opacity-80"
						style={{ resizeMode: "contain" }}
					/>

					<View className="flex-1">
						<View className="flex-row items-center gap-2 mb-1">
							<Text className="text-ink text-sm font-semibold flex-shrink">
								{volume.display_name || volume.name}
							</Text>
						</View>

						<View className="flex-row flex-wrap items-center gap-1.5">
							{fileSystem && (
								<View className="px-1.5 py-0.5 bg-app-box border border-app-line rounded">
									<Text className="text-ink-dull text-[10px]">{fileSystem}</Text>
								</View>
							)}
							{diskType && (
								<View className="px-1.5 py-0.5 bg-app-box border border-app-line rounded">
									<Text className="text-ink-dull text-[10px]">
										{getDiskTypeLabel(diskType)}
									</Text>
								</View>
							)}
							{volumeTypeStr && (
								<View className="px-1.5 py-0.5 bg-app-box border border-app-line rounded">
									<Text className="text-ink-dull text-[10px]">{volumeTypeStr}</Text>
								</View>
							)}
							{indexingProgress ? (
								<View className="px-1.5 py-0.5 bg-accent/20 border border-accent/30 rounded">
									<Text className="text-accent text-[10px] font-medium">
										{indexingProgress.filesIndexed.toLocaleString()} files
										{indexingProgress.rate > 0 && (
											<Text className="text-accent/70 ml-1">
												{" "}({Math.round(indexingProgress.rate)}/s)
											</Text>
										)}
									</Text>
								</View>
							) : (
								volume.total_files != null && (
									<View className="px-1.5 py-0.5 bg-accent/10 border border-accent/20 rounded">
										<Text className="text-accent text-[10px]">
											{volume.total_files.toLocaleString()} files
										</Text>
									</View>
								)
							)}
						</View>
					</View>

					<View className="items-end">
						<Text className="text-ink text-sm font-medium">
							{formatBytes(totalCapacity)}
						</Text>
						<Text className="text-ink-dull text-[10px]">
							{formatBytes(availableBytes)} free
						</Text>
					</View>

					<Pressable
						onPress={() => setMenuVisible(true)}
						className="px-1.5 py-1 active:bg-app-hover rounded self-start"
					>
						<Text className="text-ink-dull text-xl leading-none">⋮</Text>
					</Pressable>
				</View>

			{/* Bottom: Capacity bar */}
			<View className="px-3 pb-3 pt-2">
				<View className="bg-app border border-app-line h-8 rounded-md overflow-hidden relative">
					{/* Base capacity visualization */}
					<View className="flex-row h-full">
						<View
							className="bg-accent border-r border-accent-deep"
							style={{ width: `${uniquePercent}%` }}
						/>
						<View
							className="bg-accent/60"
							style={{ width: `${duplicatePercent}%` }}
						/>
					</View>

					{/* Indexing progress overlay */}
					{indexingProgress && (
						<View
							className="bg-accent-deep border-r-2 border-accent-deep absolute inset-y-0 left-0"
							style={{ width: `${(indexingProgress.bytesIndexed / totalCapacity) * 100}%` }}
						/>
					)}

					{/* Center label showing indexing status */}
					{indexingProgress && (
						<View className="absolute inset-0 flex items-center justify-center">
							<Text className="text-ink text-xs font-medium">
								Indexing: {(indexingProgress.percentage * 100).toFixed(1)}%
								<Text className="text-ink-dull ml-2">
									{" "}{formatBytes(indexingProgress.bytesIndexed)} / {formatBytes(totalCapacity)}
								</Text>
							</Text>
						</View>
					)}
				</View>
			</View>
		</View>
		</>
	);
}
