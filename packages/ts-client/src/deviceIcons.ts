import type { LibraryDeviceInfo } from "./generated/types";
import Laptop from "@sd/assets/icons/Laptop.png";
import Mobile from "@sd/assets/icons/Mobile.png";
import MobileAndroid from "@sd/assets/icons/Mobile-Android.png";
import Tablet from "@sd/assets/icons/Tablet.png";
import SilverBox from "@sd/assets/icons/SilverBox.png";
import MiniSilverBox from "@sd/assets/icons/MiniSilverBox.png";
import PC from "@sd/assets/icons/PC.png";
import Server from "@sd/assets/icons/Server.png";

export type DeviceIcon = string;

/**
 * Determines the appropriate device icon based on device information.
 *
 * Priority order:
 * 1. Hardware model (for Mac Studio, Mac Mini, etc.)
 * 2. Operating system
 * 3. Default to Laptop
 */
export function getDeviceIcon(device: LibraryDeviceInfo): DeviceIcon {
	// Check hardware model first for specific Mac devices
	if (device.hardware_model) {
		const model = device.hardware_model.toLowerCase();

		// Mac Studio
		if (model.includes("mac studio") || model.includes("macstudio")) {
			return SilverBox;
		}

		// Mac Mini
		if (model.includes("mac mini") || model.includes("macmini")) {
			return MiniSilverBox;
		}

		// Tablets (iPad, Surface, etc.)
		if (
			model.includes("ipad") ||
			model.includes("tablet") ||
			model.includes("surface")
		) {
			return Tablet;
		}

		// Mobile devices (iPhone, etc.)
		if (
			model.includes("iphone") ||
			model.includes("mobile") ||
			model.includes("phone")
		) {
			return device.os.toLowerCase() === "android"
				? MobileAndroid
				: Mobile;
		}

		// Laptops
		if (
			model.includes("macbook") ||
			model.includes("laptop") ||
			model.includes("notebook")
		) {
			return Laptop;
		}

		// Desktop PCs
		if (
			model.includes("imac") ||
			model.includes("desktop") ||
			model.includes("pc")
		) {
			return PC;
		}
	}

	// Fall back to OS-based detection
	const os = device.os.toLowerCase();

	switch (os) {
		case "ios":
			// iOS could be iPad or iPhone, default to tablet for safety
			return Tablet;

		case "android":
			// Android could be phone or tablet, default to phone
			return MobileAndroid;

		case "macos":
			// macOS could be iMac, MacBook, Mac Studio, or Mac Mini
			// Without hardware_model, default to Laptop (most common)
			return Laptop;

		case "windows":
			// Windows could be desktop or laptop, default to PC
			return PC;

		case "linux":
			// Linux could be desktop, laptop, or server
			// Check for server-like indicators in device name
			if (device.name.toLowerCase().includes("server")) {
				return Server;
			}
			return PC;

		default:
			// Unknown OS, default to Laptop
			return Laptop;
	}
}

/**
 * Get device icon from device slug using the devices map.
 */
export function getDeviceIconBySlug(
	deviceSlug: string,
	devices: Map<string, LibraryDeviceInfo>,
): DeviceIcon {
	const device = devices.get(deviceSlug);
	return device ? getDeviceIcon(device) : Laptop; // Default to Laptop if device not found
}
