import React from "react";
import { View, Text, ScrollView, Pressable } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { useNavigation, DrawerActions } from "@react-navigation/native";
import { Card } from "../../components/primitive";

export function NetworkScreen() {
	const insets = useSafeAreaInsets();
	const navigation = useNavigation();

	const openDrawer = () => {
		navigation.dispatch(DrawerActions.openDrawer());
	};

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
				<Text className="text-2xl font-bold text-ink">Network</Text>
				<View className="w-10" />
			</View>

			{/* Network Status */}
			<Card className="mb-6">
				<View className="flex-row items-center">
					<View className="w-3 h-3 rounded-full bg-green-500 mr-3" />
					<View className="flex-1">
						<Text className="text-ink font-medium">
							Network Status
						</Text>
						<Text className="text-ink-dull text-sm">
							P2P enabled
						</Text>
					</View>
				</View>
			</Card>

			{/* Devices */}
			<View className="mb-6">
				<Text className="text-lg font-semibold text-ink mb-3">
					This Device
				</Text>
				<Card>
					<View className="flex-row items-center">
						<View className="w-10 h-10 rounded-lg bg-accent/20 items-center justify-center mr-3">
							<Text className="text-accent">📱</Text>
						</View>
						<View>
							<Text className="text-ink font-medium">
								Spacedrive Mobile
							</Text>
							<Text className="text-ink-dull text-sm">
								Connected
							</Text>
						</View>
					</View>
				</Card>
			</View>

			{/* Nearby Devices */}
			<View className="mb-6">
				<Text className="text-lg font-semibold text-ink mb-3">
					Nearby Devices
				</Text>
				<Card>
					<Text className="text-ink-dull">
						Searching for devices...
					</Text>
					<Text className="text-ink-faint text-sm mt-1">
						Make sure other devices are on the same network
					</Text>
				</Card>
			</View>

			{/* Sync Status */}
			<View className="mb-6">
				<Text className="text-lg font-semibold text-ink mb-3">
					Sync
				</Text>
				<Card className="flex-row items-center justify-between">
					<View>
						<Text className="text-ink font-medium">
							Sync Status
						</Text>
						<Text className="text-ink-dull text-sm">
							Up to date
						</Text>
					</View>
					<View className="px-3 py-1 rounded-full bg-green-500/20">
						<Text className="text-green-500 text-sm">Synced</Text>
					</View>
				</Card>
			</View>
		</ScrollView>
	);
}
