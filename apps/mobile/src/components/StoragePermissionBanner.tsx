import React from "react";
import { View, Text, Pressable, Platform } from "react-native";
import { useStoragePermission } from "../hooks/useStoragePermission";

interface StoragePermissionBannerProps {
	/** Whether to show the banner even if permission is granted (for testing) */
	forceShow?: boolean;
}

/**
 * A banner that shows when Android storage permission is required but not granted.
 * This banner explains the issue and provides a button to open settings.
 */
export function StoragePermissionBanner({ forceShow = false }: StoragePermissionBannerProps) {
	const { isRequired, isGranted, isLoading, openSettings } = useStoragePermission();

	// Don't show on iOS or while loading
	if (Platform.OS !== "android") return null;
	if (isLoading) return null;

	// Don't show if permission is granted (unless forceShow is true)
	if (isGranted && !forceShow) return null;

	// Don't show if permission isn't required on this Android version
	if (!isRequired && !forceShow) return null;

	return (
		<View className="mx-4 mb-4 rounded-lg bg-amber-500/20 border border-amber-500/40 p-4">
			<View className="flex-row items-start gap-3">
				<View className="w-8 h-8 rounded-full bg-amber-500/30 items-center justify-center">
					<Text className="text-amber-500 text-lg">!</Text>
				</View>
				<View className="flex-1">
					<Text className="text-amber-200 font-semibold text-base mb-1">
						Storage Permission Required
					</Text>
					<Text className="text-amber-200/80 text-sm mb-3">
						Spacedrive needs "All Files Access" permission to browse files on your device.
						Without it, only folder names will be visible.
					</Text>
					<Pressable
						onPress={openSettings}
						className="self-start bg-amber-500 rounded-md px-4 py-2 active:bg-amber-600"
					>
						<Text className="text-black font-medium text-sm">
							Grant Permission
						</Text>
					</Pressable>
				</View>
			</View>
		</View>
	);
}
