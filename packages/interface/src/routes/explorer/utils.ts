import LaptopIcon from "@sd/assets/icons/Laptop.png";
import MobileIcon from "@sd/assets/icons/Mobile.png";
import ServerIcon from "@sd/assets/icons/Server.png";
import PCIcon from "@sd/assets/icons/PC.png";
import type { SdPath } from "@sd/ts-client";

export function formatBytes(bytes: number | bigint | null): string {
	if (bytes === null) return "0 B";
	// Convert BigInt to number for calculation
	const numBytes = typeof bytes === "bigint" ? Number(bytes) : bytes;
	if (numBytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB"];
	const i = Math.floor(Math.log(numBytes) / Math.log(k));
	return Math.round(numBytes / Math.pow(k, i)) + " " + sizes[i];
}

export function formatRelativeTime(date: Date | string): string {
	const d = typeof date === "string" ? new Date(date) : date;
	const now = new Date();
	const diff = now.getTime() - d.getTime();
	const seconds = Math.floor(diff / 1000);
	const minutes = Math.floor(seconds / 60);
	const hours = Math.floor(minutes / 60);
	const days = Math.floor(hours / 24);

	if (days > 7) return d.toLocaleDateString();
	if (days > 0) return `${days}d ago`;
	if (hours > 0) return `${hours}h ago`;
	if (minutes > 0) return `${minutes}m ago`;
	return "Just now";
}

export function getDeviceIcon(os: string, model?: string): string {
	const osLower = os.toLowerCase();

	if (osLower.includes("ios") || osLower.includes("android")) {
		return MobileIcon;
	}

	if (osLower.includes("windows")) {
		return PCIcon;
	}

	if (osLower.includes("server") || model?.toLowerCase().includes("server")) {
		return ServerIcon;
	}

	return LaptopIcon;
}

export function sdPathToUri(sdPath: SdPath): string {
	if ("Physical" in sdPath) {
		const { device_slug, path } = sdPath.Physical;
		return `local://${device_slug}${path}`;
	}

	if ("Cloud" in sdPath) {
		const { service, identifier, path } = sdPath.Cloud;
		const scheme = service.toLowerCase();
		return `${scheme}://${identifier}/${path}`;
	}

	if ("Content" in sdPath) {
		const { content_id } = sdPath.Content;
		return `content://${content_id}`;
	}

	return "";
}