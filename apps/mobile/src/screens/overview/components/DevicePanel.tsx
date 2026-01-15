import React, { useState } from "react";
import { View, Text, Image, ScrollView, Pressable } from "react-native";
import DatabaseIcon from "@sd/assets/icons/Database.png";
import DriveAmazonS3Icon from "@sd/assets/icons/Drive-AmazonS3.png";
import DriveDropboxIcon from "@sd/assets/icons/Drive-Dropbox.png";
import DriveGoogleDriveIcon from "@sd/assets/icons/Drive-GoogleDrive.png";
import DriveIcon from "@sd/assets/icons/Drive.png";
import HDDIcon from "@sd/assets/icons/HDD.png";
import LocationIcon from "@sd/assets/icons/Location.png";
import ServerIcon from "@sd/assets/icons/Server.png";
import type {
	Device,
	JobListItem,
	Location,
	VolumeItem,
} from "@sd/ts-client";
import { getDeviceIcon } from "@sd/ts-client";
import { useNormalizedQuery, useCoreQuery, useLibraryAction } from "../../../client";

// Temporary type extension
type DeviceWithConnection = Device & {
	connection_method?: "Direct" | "Relay" | "Mixed" | null;
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

interface DevicePanelProps {
	onLocationSelect?: (location: Location | null) => void;
}

export function DevicePanel({ onLocationSelect }: DevicePanelProps = {}) {
	const [selectedLocationId, setSelectedLocationId] = useState<string | null>(
		null
	);

	// Fetch all volumes
	const { data: volumesData, isLoading: volumesLoading } = useNormalizedQuery<
		any,
		any
	>({
		wireMethod: "query:volumes.list",
		input: { filter: "All" },
		resourceType: "volume",
	});

	// Fetch all devices
	const { data: devicesData, isLoading: devicesLoading } = useNormalizedQuery<
		any,
		DeviceWithConnection[]
	>({
		wireMethod: "query:devices.list",
		input: { include_offline: true, include_details: false },
		resourceType: "device",
	});

	// Fetch all locations
	const { data: locationsData, isLoading: locationsLoading } =
		useNormalizedQuery<any, any>({
			wireMethod: "query:locations.list",
			input: null,
			resourceType: "location",
		});

	// TODO: Get jobs when mobile supports it
	const allJobs: JobListItem[] = [];

	if (volumesLoading || devicesLoading || locationsLoading) {
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
	const locations = locationsData?.locations || [];

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
	}, {} as Record<string, VolumeItem[]>);

	// Group locations by device slug
	const locationsByDeviceSlug = locations.reduce((acc: any, location: Location) => {
		if (
			typeof location.sd_path === "object" &&
			"Physical" in location.sd_path
		) {
			const deviceSlug = (location.sd_path as any).Physical.device_slug;
			if (!acc[deviceSlug]) {
				acc[deviceSlug] = [];
			}
			acc[deviceSlug].push(location);
		}
		return acc;
	}, {} as Record<string, Location[]>);

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
				const deviceLocations = locationsByDeviceSlug[device.slug] || [];

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
				<View className="bg-app-box border border-app-line rounded-xl overflow-hidden">
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
	method: "Direct" | "Relay" | "Mixed";
}

function ConnectionBadge({ method }: ConnectionBadgeProps) {
	const labels = {
		Direct: "Local",
		Relay: "Relay",
		Mixed: "Mixed",
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
		<View className="bg-app-darkBox border border-app-line mb-4 rounded-xl overflow-hidden">
			{/* Device Header */}
			<View className="bg-app-box border-b border-app-line px-6 py-4">
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
				<View className="border-b border-app-line bg-app/50 px-3 py-3 gap-2">
					{/* TODO: Add JobCard component when ported */}
					<Text className="text-ink-dull text-xs">
						{activeJobs.length} active {activeJobs.length === 1 ? "job" : "jobs"}
					</Text>
				</View>
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
	return (
		<View className="border-b border-app-line px-3 py-3">
			<ScrollView horizontal showsHorizontalScrollIndicator={false} className="gap-2">
				{locations.map((location) => {
					const isSelected = selectedLocationId === location.id;
					return (
						<Pressable
							key={location.id}
							onPress={() => {
								if (isSelected) {
									onLocationSelect?.(null);
								} else {
									onLocationSelect?.(location);
								}
							}}
							className="min-w-[80px] items-center gap-2 p-1"
						>
							<View
								className={`rounded-lg p-2 ${
									isSelected ? "bg-app-box" : "bg-transparent"
								}`}
							>
								<Image
									source={LocationIcon}
									className="w-12 h-12 opacity-80"
									style={{ resizeMode: "contain" }}
								/>
							</View>
							<View className="w-full items-center">
								<View
									className={`px-2 py-0.5 rounded-md ${
										isSelected
											? "bg-accent"
											: "bg-transparent"
									}`}
								>
									<Text
										className={`text-xs ${
											isSelected ? "text-white" : "text-ink"
										}`}
										numberOfLines={1}
									>
										{location.name}
									</Text>
								</View>
							</View>
						</Pressable>
					);
				})}
			</ScrollView>
		</View>
	);
}

interface VolumeBarProps {
	volume: VolumeItem;
	index: number;
}

function VolumeBar({ volume, index }: VolumeBarProps) {
	const trackVolume = useLibraryAction("volumes.track");
	const indexVolume = useLibraryAction("volumes.index");

	const devicesQuery = useNormalizedQuery<any, Device[]>({
		wireMethod: "query:devices.list",
		input: { include_offline: true, include_details: false },
		resourceType: "device",
	});

	const currentDevice = devicesQuery.data?.find(d => d.is_current);

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

	const totalCapacity = volume.total_capacity || 0;
	const availableBytes = volume.available_capacity || 0;
	const usedBytes = totalCapacity > 0 ? totalCapacity - availableBytes : 0;

	const uniqueBytes = volume.unique_bytes ?? Math.floor(usedBytes * 0.7);
	const duplicateBytes = usedBytes - uniqueBytes;

	const uniquePercent = totalCapacity > 0 ? (uniqueBytes / totalCapacity) * 100 : 0;
	const duplicatePercent = totalCapacity > 0 ? (duplicateBytes / totalCapacity) * 100 : 0;

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

	const iconSrc = getVolumeIcon(volume.volume_type, volume.name);
	const volumeTypeStr =
		typeof volume.volume_type === "string"
			? volume.volume_type
			: (volume.volume_type as any)?.Other ||
				JSON.stringify(volume.volume_type);

	return (
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
						{!volume.is_online && (
							<View className="px-1.5 py-0.5 bg-app-box border border-app-line rounded">
								<Text className="text-ink-faint text-[10px]">Offline</Text>
							</View>
						)}
						{!volume.is_tracked && (
							<Pressable
								onPress={handleTrack}
								disabled={trackVolume.isPending}
								className="flex-row items-center gap-1 px-1.5 py-0.5 bg-accent/10 border border-accent/20 rounded active:bg-accent/20"
							>
								<Text className="text-accent text-[10px]">
									{trackVolume.isPending ? "Tracking..." : "+ Track"}
								</Text>
							</Pressable>
						)}
						{currentDevice && volume.device_id === currentDevice.id && (
							<Pressable
								onPress={handleIndex}
								disabled={indexVolume.isPending}
								className="flex-row items-center gap-1 px-1.5 py-0.5 bg-sidebar-box border border-sidebar-line rounded active:opacity-70"
							>
								<Text className="text-sidebar-ink text-[10px]">
									{indexVolume.isPending ? "Indexing..." : "🔍 Index"}
								</Text>
							</Pressable>
						)}
					</View>

					<View className="flex-row flex-wrap items-center gap-1.5">
						<View className="px-1.5 py-0.5 bg-app-box border border-app-line rounded">
							<Text className="text-ink-dull text-[10px]">{fileSystem}</Text>
						</View>
						<View className="px-1.5 py-0.5 bg-app-box border border-app-line rounded">
							<Text className="text-ink-dull text-[10px]">
								{getDiskTypeLabel(diskType)}
							</Text>
						</View>
						<View className="px-1.5 py-0.5 bg-app-box border border-app-line rounded">
							<Text className="text-ink-dull text-[10px]">{volumeTypeStr}</Text>
						</View>
						{volume.total_file_count != null && (
							<View className="px-1.5 py-0.5 bg-accent/10 border border-accent/20 rounded">
								<Text className="text-accent text-[10px]">
									{volume.total_file_count.toLocaleString()} files
								</Text>
							</View>
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
			</View>

			{/* Bottom: Capacity bar */}
			<View className="px-3 pb-3 pt-2">
				<View className="bg-app border border-app-line h-8 rounded-md overflow-hidden">
					<View className="flex-row h-full">
						<View
							className="bg-accent border-r border-accent-deep"
							style={{ width: `${uniquePercent}%` }}
						/>
						<View
							className="bg-accent/60"
							style={{
								width: `${duplicatePercent}%`,
							}}
						/>
					</View>
				</View>
			</View>
		</View>
	);
}
