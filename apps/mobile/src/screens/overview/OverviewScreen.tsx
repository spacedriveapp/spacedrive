import React from "react";
import { View, Text, ScrollView, Pressable } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { useNavigation, DrawerActions } from "@react-navigation/native";
import { useLibraryQuery } from "../../client";
import { HeroStats, StorageOverview } from "./components";

export function OverviewScreen() {
	const insets = useSafeAreaInsets();
	const navigation = useNavigation();

	// Fetch library info with statistics
	const {
		data: libraryInfo,
		isLoading,
		error,
	} = useLibraryQuery("libraries.info", null);

	const openDrawer = () => {
		navigation.dispatch(DrawerActions.openDrawer());
	};

	if (isLoading || !libraryInfo) {
		return (
			<ScrollView
				className="flex-1 bg-app"
				contentContainerStyle={{
					paddingTop: insets.top + 16,
					paddingBottom: insets.bottom + 100,
					paddingHorizontal: 16,
				}}
			>
				{/* Header */}
				<View className="flex-row items-center justify-between mb-6">
					<Pressable onPress={openDrawer} className="p-2 -ml-2">
						<View className="w-6 h-0.5 bg-ink mb-1.5" />
						<View className="w-6 h-0.5 bg-ink mb-1.5" />
						<View className="w-6 h-0.5 bg-ink" />
					</Pressable>
					<Text className="text-xl font-bold text-ink">
						{libraryInfo?.name || "Loading..."}
					</Text>
					<View className="w-10" />
				</View>

				<View className="items-center justify-center py-12">
					<Text className="text-ink-dull">
						Loading library statistics...
					</Text>
				</View>
			</ScrollView>
		);
	}

	if (error) {
		return (
			<ScrollView
				className="flex-1 bg-app"
				contentContainerStyle={{
					paddingTop: insets.top + 16,
					paddingBottom: insets.bottom + 100,
					paddingHorizontal: 16,
				}}
			>
				{/* Header */}
				<View className="flex-row items-center justify-between mb-6">
					<Pressable onPress={openDrawer} className="p-2 -ml-2">
						<View className="w-6 h-0.5 bg-ink mb-1.5" />
						<View className="w-6 h-0.5 bg-ink mb-1.5" />
						<View className="w-6 h-0.5 bg-ink" />
					</Pressable>
					<Text className="text-xl font-bold text-ink">Overview</Text>
					<View className="w-10" />
				</View>

				<View className="items-center justify-center py-12">
					<Text className="text-red-500 font-semibold">Error</Text>
					<Text className="text-ink-dull mt-2">
						{String(error)}
					</Text>
				</View>
			</ScrollView>
		);
	}

	const stats = libraryInfo.statistics;

	return (
		<ScrollView
			className="flex-1 bg-app"
			contentContainerStyle={{
				paddingTop: insets.top + 16,
				paddingBottom: insets.bottom + 100,
				paddingHorizontal: 16,
			}}
		>
			{/* Header */}
			<View className="flex-row items-center justify-between mb-6">
				<Pressable onPress={openDrawer} className="p-2 -ml-2">
					<View className="w-6 h-0.5 bg-ink mb-1.5" />
					<View className="w-6 h-0.5 bg-ink mb-1.5" />
					<View className="w-6 h-0.5 bg-ink" />
				</Pressable>
				<Text className="text-xl font-bold text-ink">
					{libraryInfo.name}
				</Text>
				<View className="w-10" />
			</View>

			{/* Hero Stats */}
			<HeroStats
				totalStorage={stats.total_capacity}
				usedStorage={stats.total_capacity - stats.available_capacity}
				totalFiles={Number(stats.total_files)}
				locationCount={stats.location_count}
				tagCount={stats.tag_count}
				deviceCount={stats.device_count}
				uniqueContentCount={Number(stats.unique_content_count)}
			/>

			{/* Storage Volumes */}
			<StorageOverview />
		</ScrollView>
	);
}
