import React, { useState, useMemo } from "react";
import { View, Text, ScrollView, Pressable } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { useNavigation, DrawerActions } from "@react-navigation/native";
import { useNormalizedQuery } from "../../client";
import type { Library } from "@sd/ts-client";
import { HeroStats, DevicePanel, ActionButtons } from "./components";
import { PairingPanel } from "../../components/PairingPanel";
import { LibrarySwitcherPanel } from "../../components/LibrarySwitcherPanel";

export function OverviewScreen() {
	const insets = useSafeAreaInsets();
	const navigation = useNavigation();
	const [showPairing, setShowPairing] = useState(false);
	const [showLibrarySwitcher, setShowLibrarySwitcher] = useState(false);
	const [selectedLocationId, setSelectedLocationId] = useState<string | null>(
		null
	);

	// Fetch library info with real-time statistics updates
	const {
		data: libraryInfo,
		isLoading,
		error,
	} = useNormalizedQuery<null, Library>({
		wireMethod: "query:libraries.info",
		input: null,
		resourceType: "library",
	});

	// Fetch locations list to get the selected location reactively
	const { data: locationsData } = useNormalizedQuery<any, any>({
		wireMethod: "query:locations.list",
		input: null,
		resourceType: "location",
	});

	// Find the selected location from the list reactively
	const selectedLocation = useMemo(() => {
		if (!selectedLocationId || !locationsData?.locations) return null;
		return (
			locationsData.locations.find(
				(loc: any) => loc.id === selectedLocationId
			) || null
		);
	}, [selectedLocationId, locationsData]);

	const openDrawer = () => {
		navigation.dispatch(DrawerActions.openDrawer());
	};

	if (isLoading || !libraryInfo) {
		return (
			<ScrollView
				className="flex-1 bg-app"
				contentContainerStyle={{
					paddingBottom: insets.bottom + 100,
					paddingHorizontal: 16,
				}}
			>
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
					paddingBottom: insets.bottom + 100,
					paddingHorizontal: 16,
				}}
			>
				<View className="items-center justify-center py-12">
					<Text className="text-red-500 font-semibold">Error</Text>
					<Text className="text-ink-dull mt-2">{String(error)}</Text>
				</View>
			</ScrollView>
		);
	}

	const stats = libraryInfo.statistics;

	return (
		<ScrollView
			className="flex-1 bg-app"
			contentContainerStyle={{
				paddingBottom: insets.bottom + 100,
			}}
		>
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

			{/* Device Panel */}
			<View className="px-4">
				<DevicePanel
					onLocationSelect={(location) =>
						setSelectedLocationId(location?.id || null)
					}
				/>
			</View>

			{/* Action Buttons */}
			<View className="px-4">
				<ActionButtons
					onPairDevice={() => setShowPairing(true)}
					onSetupSync={() => {/* TODO: Open sync setup */}}
					onAddStorage={() => {/* TODO: Open location picker */}}
				/>
			</View>

			{/* Pairing Panel */}
			<PairingPanel
				isOpen={showPairing}
				onClose={() => setShowPairing(false)}
			/>

			{/* Library Switcher Panel */}
			<LibrarySwitcherPanel
				isOpen={showLibrarySwitcher}
				onClose={() => setShowLibrarySwitcher(false)}
			/>
		</ScrollView>
	);
}
