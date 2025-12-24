import { useEffect, useMemo } from "react";
import { useNormalizedQuery } from "../../context";
import { useTabManager } from "./useTabManager";
import type { ListLibraryDevicesInput, LibraryDeviceInfo } from "@sd/ts-client";

/**
 * TabDefaultsSync - Sets the default new tab path to the current device
 *
 * This component fetches the current device and updates the TabManager's
 * default path so new tabs open to the device's virtual view.
 */
export function TabDefaultsSync() {
	const { setDefaultNewTabPath } = useTabManager();

	// Fetch all devices and find the current one
	const { data: devices } = useNormalizedQuery<
		ListLibraryDevicesInput,
		LibraryDeviceInfo[]
	>({
		wireMethod: "query:devices.list",
		input: { include_offline: true, include_details: false },
		resourceType: "device",
	});

	// Find the current device
	const currentDevice = useMemo(() => {
		return devices?.find((d) => d.is_current) ?? null;
	}, [devices]);

	// Set default new tab path when current device is known
	useEffect(() => {
		if (currentDevice?.id) {
			const deviceViewPath = `/explorer?view=device&id=${currentDevice.id}`;
			setDefaultNewTabPath(deviceViewPath);
		}
	}, [currentDevice?.id, setDefaultNewTabPath]);

	return null;
}

