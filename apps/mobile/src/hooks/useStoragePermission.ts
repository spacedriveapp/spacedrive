import { useEffect, useState, useCallback } from "react";
import { Platform, Alert, AppState, AppStateStatus } from "react-native";
import { SDMobileCore } from "sd-mobile-core";

export interface StoragePermissionState {
	/** Whether storage permission is required on this device */
	isRequired: boolean;
	/** Whether the permission has been granted */
	isGranted: boolean;
	/** Whether we're still checking the permission status */
	isLoading: boolean;
	/** Open the system settings to grant permission */
	openSettings: () => void;
	/** Re-check the permission status (useful after returning from settings) */
	recheckPermission: () => void;
}

/**
 * Hook to check and manage Android storage permission.
 * On Android 11+, apps need "All Files Access" permission to read files
 * outside of app-specific directories when using direct filesystem paths.
 */
export function useStoragePermission(): StoragePermissionState {
	const [isRequired, setIsRequired] = useState(false);
	const [isGranted, setIsGranted] = useState(true);
	const [isLoading, setIsLoading] = useState(true);

	const checkPermission = useCallback(() => {
		if (Platform.OS !== "android") {
			setIsRequired(false);
			setIsGranted(true);
			setIsLoading(false);
			return;
		}

		try {
			const required = SDMobileCore.requiresStoragePermission();
			setIsRequired(required);

			if (required) {
				const granted = SDMobileCore.hasStoragePermission();
				setIsGranted(granted);
			} else {
				setIsGranted(true);
			}
		} catch (error) {
			console.error("Error checking storage permission:", error);
			// Assume granted on error to avoid blocking the user
			setIsGranted(true);
		}

		setIsLoading(false);
	}, []);

	const openSettings = useCallback(() => {
		if (Platform.OS !== "android") return;

		try {
			SDMobileCore.openStoragePermissionSettings();
		} catch (error) {
			console.error("Error opening storage settings:", error);
			Alert.alert(
				"Unable to Open Settings",
				"Please go to Settings > Apps > Spacedrive > Permissions and enable 'All Files Access' manually.",
			);
		}
	}, []);

	// Check permission on mount
	useEffect(() => {
		checkPermission();
	}, [checkPermission]);

	// Re-check permission when app comes back to foreground (user may have granted it in settings)
	useEffect(() => {
		const handleAppStateChange = (nextAppState: AppStateStatus) => {
			if (nextAppState === "active") {
				checkPermission();
			}
		};

		const subscription = AppState.addEventListener("change", handleAppStateChange);
		return () => subscription.remove();
	}, [checkPermission]);

	return {
		isRequired,
		isGranted,
		isLoading,
		openSettings,
		recheckPermission: checkPermission,
	};
}

/**
 * Show an alert prompting the user to grant storage permission.
 * Returns a promise that resolves when the user dismisses the alert.
 */
export function showStoragePermissionAlert(openSettings: () => void): Promise<void> {
	return new Promise((resolve) => {
		Alert.alert(
			"Storage Permission Required",
			"Spacedrive needs 'All Files Access' permission to browse and index files on your device.\n\n" +
				"Without this permission, you'll only see folder names but not the files inside them.",
			[
				{
					text: "Not Now",
					style: "cancel",
					onPress: () => resolve(),
				},
				{
					text: "Open Settings",
					onPress: () => {
						openSettings();
						resolve();
					},
				},
			],
		);
	});
}
