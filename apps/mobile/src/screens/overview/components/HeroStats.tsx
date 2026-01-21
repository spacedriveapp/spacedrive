import React from "react";
import { View, Text, Image } from "react-native";
import DevicesIcon from "@sd/assets/icons/Devices.png";
import IndexedIcon from "@sd/assets/icons/Indexed.png";
import MobileIcon from "@sd/assets/icons/Mobile.png";
import ComputeIcon from "@sd/assets/icons/Compute.png";

interface HeroStatsProps {
	totalStorage: number; // bytes
	usedStorage: number; // bytes
	totalFiles: number;
	locationCount: number;
	tagCount: number;
	deviceCount: number;
	uniqueContentCount: number;
}

function formatBytes(bytes: number): { value: string; unit: string } {
	if (bytes === 0) return { value: "0", unit: "B" };
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB", "PB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return {
		value: (bytes / Math.pow(k, i)).toFixed(1),
		unit: sizes[i],
	};
}

function getTOPSRank(tops: number): { label: string } {
	if (tops >= 100) return { label: "Extreme" };
	if (tops >= 70) return { label: "Very High" };
	if (tops >= 40) return { label: "High" };
	if (tops >= 20) return { label: "Moderate" };
	return { label: "Low" };
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

	const storageFormatted = formatBytes(totalStorage);
	const usedFormatted = formatBytes(usedStorage);
	const topsValue = 70;
	const topsRank = getTOPSRank(topsValue);

	return (
		<View className="px-8 pt-8 pb-12">
			<View className="flex-row flex-wrap gap-8">
				{/* Total Storage */}
				<StatCard
					icon={DevicesIcon}
					label="Total Storage"
					value={
						<>
							<Text className="text-ink text-3xl font-bold">
								{storageFormatted.value}{" "}
								<Text className="text-ink-faint text-xl">
									{storageFormatted.unit}
								</Text>
							</Text>
						</>
					}
					subtitle={
						<>
							<Text className="text-accent">
								{usedFormatted.value}{" "}
								<Text className="text-accent/70 text-[10px]">
									{usedFormatted.unit}
								</Text>
							</Text>{" "}
							used
						</>
					}
					progress={usagePercent}
				/>

				{/* Files */}
				<StatCard
					icon={IndexedIcon}
					label="Files Indexed"
					value={totalFiles.toLocaleString()}
					subtitle={`${uniqueContentCount.toLocaleString()} unique files`}
				/>

				{/* Devices */}
				<StatCard
					icon={MobileIcon}
					label="Connected Devices"
					value={deviceCount}
					subtitle="registered in library"
				/>

				{/* AI Compute Power */}
				<StatCard
					icon={ComputeIcon}
					label="AI Compute Power"
					value={
						<>
							<Text className="text-ink text-3xl font-bold">
								{topsValue}{" "}
								<Text className="text-ink-faint text-xl">TOPS</Text>
							</Text>
						</>
					}
					subtitle={topsRank.label}
				/>
			</View>
		</View>
	);
}

interface StatCardProps {
	icon: any;
	label: string;
	value: string | number | React.ReactNode;
	subtitle: React.ReactNode | string;
	progress?: number;
}

function StatCard({ icon, label, value, subtitle, progress }: StatCardProps) {
	return (
		<View className="flex-1 min-w-[140px]">
			<View className="mb-1 flex-row items-center gap-3">
				<Image
					source={icon}
					className="w-8 h-8 opacity-80"
					style={{ resizeMode: "contain" }}
				/>
				{typeof value === 'string' || typeof value === 'number' ? (
					<Text className="text-ink text-3xl font-bold">
						{value}
					</Text>
				) : (
					value
				)}
			</View>
			<Text className="text-ink-dull text-xs mb-1">{label}</Text>
			<Text className="text-ink-faint text-xs">{subtitle}</Text>
		</View>
	);
}
