import React from "react";
import { View, Text, Pressable } from "react-native";
import { useLibraryQuery, useLibraryMutation } from "../../../client";

function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB", "PB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

function getDiskTypeLabel(diskType: string): string {
	return diskType === "SSD" ? "SSD" : diskType === "HDD" ? "HDD" : diskType;
}

export function StorageOverview() {
	// Fetch all volumes
	const { data: volumesData, isLoading: volumesLoading } = useLibraryQuery(
		"volumes.list",
		{ filter: "All" }
	);

	// Fetch all devices
	const { data: devicesData, isLoading: devicesLoading } = useLibraryQuery(
		"devices.list",
		{ include_offline: true, include_details: false }
	);

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

	return (
		<View className="bg-app-box border border-app-line rounded-xl overflow-hidden mb-6">
			<View className="px-6 py-4 border-b border-app-line">
				<Text className="text-base font-semibold text-ink">
					Storage Volumes
				</Text>
				<Text className="text-sm text-ink-dull mt-1">
					{userVisibleVolumes.length}{" "}
					{userVisibleVolumes.length === 1 ? "volume" : "volumes"}{" "}
					across {devices.length}{" "}
					{devices.length === 1 ? "device" : "devices"}
				</Text>
			</View>

			<View className="p-4">
				{userVisibleVolumes.map((volume: any, idx: number) => (
					<VolumeBar key={volume.id} volume={volume} />
				))}

				{userVisibleVolumes.length === 0 && (
					<View className="py-12 items-center">
						<Text className="text-ink-faint text-sm">
							No volumes detected
						</Text>
						<Text className="text-ink-faint text-xs mt-1">
							Track a volume to see storage information
						</Text>
					</View>
				)}
			</View>
		</View>
	);
}

interface VolumeBarProps {
	volume: any;
}

function VolumeBar({ volume }: VolumeBarProps) {
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

	const fileSystem = volume.file_system || "Unknown";
	const diskType = volume.disk_type || "Unknown";
	const readSpeed = volume.read_speed_mbps;

	return (
		<View className="p-4 mb-3 bg-app-darkBox rounded-lg border border-app-line">
			{/* Header */}
			<View className="flex-row items-center justify-between mb-3">
				<View className="flex-1 flex-row items-center gap-2">
					<Text className="font-semibold text-ink text-base">
						{volume.name}
					</Text>
					{!volume.is_online && (
						<View className="px-2 py-0.5 bg-app-box border border-app-line rounded">
							<Text className="text-ink-faint text-xs">
								Offline
							</Text>
						</View>
					)}
					{!volume.is_tracked && (
						<Pressable
							onPress={handleTrack}
							disabled={trackVolume.isPending}
							className="px-2 py-0.5 bg-accent/10 border border-accent/20 rounded active:bg-accent/20"
						>
							<Text className="text-accent text-xs">
								{trackVolume.isPending ? "Tracking..." : "Track"}
							</Text>
						</Pressable>
					)}
				</View>
				<View className="items-end">
					<Text className="text-sm font-medium text-ink">
						{formatBytes(totalCapacity)}
					</Text>
					<Text className="text-xs text-ink-dull">
						{formatBytes(availableBytes)} free
					</Text>
				</View>
			</View>

			{/* Capacity bar */}
			<View className="h-6 bg-app rounded-md overflow-hidden border border-app-line mb-3">
				<View className="h-full flex-row">
					{/* Unique bytes */}
					<View
						className="bg-blue-500"
						style={{ width: `${uniquePercent}%` }}
					/>
					{/* Duplicate bytes */}
					<View
						className="bg-blue-400"
						style={{ width: `${duplicatePercent}%` }}
					/>
				</View>
			</View>

			{/* Stats */}
			<View className="flex-row flex-wrap gap-2 mb-2">
				<View className="flex-row items-center gap-1.5">
					<View className="size-3 rounded bg-blue-500" />
					<Text className="text-ink-dull text-xs">
						Unique: {formatBytes(uniqueBytes)}
					</Text>
				</View>
				<View className="flex-row items-center gap-1.5">
					<View className="size-3 rounded bg-blue-400" />
					<Text className="text-ink-dull text-xs">
						Duplicate: {formatBytes(duplicateBytes)}
					</Text>
				</View>
				<Text className="text-ink-faint text-xs">•</Text>
				<Text className="text-ink-dull text-xs">
					{usagePercent.toFixed(1)}% used
				</Text>
			</View>

			{/* Tags */}
			<View className="flex-row flex-wrap gap-2">
				<View className="px-2 py-0.5 bg-app-box rounded border border-app-line">
					<Text className="text-ink-dull text-xs">{fileSystem}</Text>
				</View>
				<View className="px-2 py-0.5 bg-app-box rounded border border-app-line">
					<Text className="text-ink-dull text-xs">
						{getDiskTypeLabel(diskType)}
					</Text>
				</View>
				{readSpeed && (
					<View className="px-2 py-0.5 bg-app-box rounded border border-app-line">
						<Text className="text-ink-dull text-xs">
							{readSpeed} MB/s
						</Text>
					</View>
				)}
				<View className="px-2 py-0.5 bg-app-box rounded border border-app-line">
					<Text className="text-ink-dull text-xs">
						{volume.volume_type}
					</Text>
				</View>
			</View>
		</View>
	);
}
