import React, { useState, useCallback, useRef } from "react";
import {
	View,
	Text,
	Image,
	ScrollView,
	Dimensions,
	NativeScrollEvent,
	NativeSyntheticEvent,
} from "react-native";
import DevicesIcon from "@sd/assets/icons/Devices.png";
import IndexedIcon from "@sd/assets/icons/Indexed.png";
import LocationIcon from "@sd/assets/icons/Location.png";
import MobileIcon from "@sd/assets/icons/Mobile.png";
import ComputeIcon from "@sd/assets/icons/Compute.png";
import TagsIcon from "@sd/assets/icons/Tags.png";
import DatabaseIcon from "@sd/assets/icons/Database.png";
import StorageIcon from "@sd/assets/icons/Storage.png";
import { PageIndicator } from "../../../components/PageIndicator";

const SCREEN_WIDTH = Dimensions.get("window").width;

interface HeroStatsProps {
	totalStorage: number; // bytes
	usedStorage: number; // bytes
	totalFiles: number;
	locationCount: number;
	tagCount: number;
	deviceCount: number;
	uniqueContentCount: number;
	databaseSize: number; // bytes
	sidecarCount: number;
	sidecarSize: number; // bytes
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
	tagCount,
	deviceCount,
	uniqueContentCount,
	databaseSize,
	sidecarCount,
	sidecarSize,
}: HeroStatsProps) {
	const [currentPage, setCurrentPage] = useState(0);
	const scrollViewRef = useRef<ScrollView>(null);

	const usagePercent =
		totalStorage > 0 ? (usedStorage / totalStorage) * 100 : 0;

	const storageFormatted = formatBytes(totalStorage);
	const usedFormatted = formatBytes(usedStorage);
	const databaseFormatted = formatBytes(databaseSize);
	const sidecarFormatted = formatBytes(sidecarSize);
	const topsValue = 70;
	const topsRank = getTOPSRank(topsValue);

	const handleScroll = useCallback(
		(event: NativeSyntheticEvent<NativeScrollEvent>) => {
			const offsetX = event.nativeEvent.contentOffset.x;
			const page = Math.round(offsetX / SCREEN_WIDTH);
			setCurrentPage(page);
		},
		[]
	);

	// Define all stats
	const allStats = [
		{
			icon: DevicesIcon,
			label: "Total Storage",
			value: (
				<>
					<Text className="text-ink text-3xl font-bold">
						{storageFormatted.value}{" "}
						<Text className="text-ink-faint text-xl">
							{storageFormatted.unit}
						</Text>
					</Text>
				</>
			),
			subtitle: (
				<>
					<Text className="text-accent">
						{usedFormatted.value}{" "}
						<Text className="text-accent/70 text-[10px]">
							{usedFormatted.unit}
						</Text>
					</Text>{" "}
					used
				</>
			),
			progress: usagePercent,
		},
		{
			icon: IndexedIcon,
			label: "Files Indexed",
			value: totalFiles.toLocaleString(),
			subtitle: `${uniqueContentCount.toLocaleString()} unique files`,
		},
		{
			icon: MobileIcon,
			label: "Connected Devices",
			value: deviceCount,
			subtitle: "registered in library",
		},
		{
			icon: ComputeIcon,
			label: "AI Compute Power",
			value: (
				<>
					<Text className="text-ink text-3xl font-bold">
						{topsValue}{" "}
						<Text className="text-ink-faint text-xl">TOPS</Text>
					</Text>
				</>
			),
			subtitle: topsRank.label,
		},
		// Second page - Storage breakdown
		{
			icon: DatabaseIcon,
			label: "Library Size",
			value: (
				<>
					<Text className="text-ink text-3xl font-bold">
						{databaseFormatted.value}{" "}
						<Text className="text-ink-faint text-xl">
							{databaseFormatted.unit}
						</Text>
					</Text>
				</>
			),
			subtitle: "database on disk",
		},
		{
			icon: StorageIcon,
			label: "Sidecar Storage",
			value: (
				<>
					<Text className="text-ink text-3xl font-bold">
						{sidecarFormatted.value}{" "}
						<Text className="text-ink-faint text-xl">
							{sidecarFormatted.unit}
						</Text>
					</Text>
				</>
			),
			subtitle: `${sidecarCount.toLocaleString()} files generated`,
		},
		{
			icon: LocationIcon,
			label: "Locations",
			value: locationCount,
			subtitle: "indexed folders",
		},
		{
			icon: TagsIcon,
			label: "Tags",
			value: tagCount,
			subtitle: "organization labels",
		},
	];

	// Group stats into pages of 4
	const STATS_PER_PAGE = 4;
	const pages: typeof allStats[] = [];
	for (let i = 0; i < allStats.length; i += STATS_PER_PAGE) {
		pages.push(allStats.slice(i, i + STATS_PER_PAGE));
	}

	return (
		<View className="pt-8 pb-12">
			<ScrollView
				ref={scrollViewRef}
				horizontal
				pagingEnabled
				showsHorizontalScrollIndicator={false}
				onScroll={handleScroll}
				scrollEventThrottle={16}
				decelerationRate="fast"
				nestedScrollEnabled={true}
			>
				{pages.map((pageStats, pageIndex) => (
					<View
						key={pageIndex}
						style={{ width: SCREEN_WIDTH }}
						className="px-8"
					>
						<View className="flex-row flex-wrap gap-8">
							{pageStats.map((stat, statIndex) => (
								<StatCard
									key={statIndex}
									icon={stat.icon}
									label={stat.label}
									value={stat.value}
									subtitle={stat.subtitle}
									progress={stat.progress}
								/>
							))}
						</View>
					</View>
				))}
			</ScrollView>

			{pages.length > 1 && (
				<View className="mt-10">
					<PageIndicator
						currentIndex={currentPage}
						totalPages={pages.length}
					/>
				</View>
			)}
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
