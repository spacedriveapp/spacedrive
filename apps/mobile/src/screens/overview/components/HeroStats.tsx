import React from "react";
import { View, Text } from "react-native";

interface HeroStatsProps {
	totalStorage: number; // bytes
	usedStorage: number; // bytes
	totalFiles: number;
	locationCount: number;
	tagCount: number;
	deviceCount: number;
	uniqueContentCount: number;
}

function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB", "PB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

export function HeroStats({
	totalStorage,
	usedStorage,
	totalFiles,
	locationCount,
	deviceCount,
	uniqueContentCount,
}: HeroStatsProps) {
	const usagePercent =
		totalStorage > 0 ? (usedStorage / totalStorage) * 100 : 0;

	return (
		<View className="bg-app-box border border-app-line rounded-2xl p-6 mb-6">
			<View className="flex-row flex-wrap gap-4">
				{/* Total Storage */}
				<StatCard
					label="Total Storage"
					value={formatBytes(totalStorage)}
					subtitle={`${formatBytes(usedStorage)} used`}
					progress={usagePercent}
				/>

				{/* Files */}
				<StatCard
					label="Files Indexed"
					value={totalFiles.toLocaleString()}
					subtitle={`${uniqueContentCount.toLocaleString()} unique`}
				/>

				{/* Devices */}
				<StatCard
					label="Devices"
					value={deviceCount.toString()}
					subtitle="connected"
				/>

				{/* Locations */}
				<StatCard
					label="Locations"
					value={locationCount.toString()}
					subtitle="tracked"
				/>
			</View>
		</View>
	);
}

interface StatCardProps {
	label: string;
	value: string | number;
	subtitle: string;
	progress?: number;
}

function StatCard({ label, value, subtitle, progress }: StatCardProps) {
	return (
		<View className="flex-1 min-w-[140px]">
			<View className="mb-2">
				<Text className="text-2xl font-bold text-ink">{value}</Text>
				<Text className="text-xs text-ink-dull mt-0.5">{label}</Text>
				<Text className="text-xs text-ink-faint">{subtitle}</Text>
			</View>
			{progress !== undefined && (
				<View className="h-1.5 bg-app-darkBox rounded-full overflow-hidden">
					<View
						className="h-full bg-accent rounded-full"
						style={{ width: `${Math.min(progress, 100)}%` }}
					/>
				</View>
			)}
		</View>
	);
}
