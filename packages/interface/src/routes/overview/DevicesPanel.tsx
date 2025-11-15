import { motion } from "framer-motion";
import clsx from "clsx";
import { HardDrive, DeviceMobile } from "@phosphor-icons/react";
import LaptopIcon from "@sd/assets/icons/Laptop.png";
import MobileIcon from "@sd/assets/icons/Mobile.png";
import PCIcon from "@sd/assets/icons/PC.png";
import ServerIcon from "@sd/assets/icons/Server.png";

interface DevicesPanelProps {
	devices: any[];
	volumes: any[];
}

function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB", "PB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

function getDeviceIcon(os: string, model?: string): string {
	if (os === "IOs" || os === "Android") return MobileIcon;
	if (os === "Windows") return PCIcon;
	if (model?.includes("Server") || model?.includes("Studio")) return ServerIcon;
	return LaptopIcon; // Default for MacOS and others
}

export function DevicesPanel({ devices, volumes }: DevicesPanelProps) {
	return (
		<div className="bg-app-box border border-app-line rounded-xl overflow-hidden">
			<div className="px-6 py-4 border-b border-app-line">
				<h2 className="text-base font-semibold text-ink">Devices</h2>
				<p className="text-sm text-ink-dull mt-1">
					{devices.filter((d) => d.is_online).length} of {devices.length} online
				</p>
			</div>

			<div className="p-6 space-y-3">
				{devices.map((device, idx) => (
					<DeviceCard
						key={device.id}
						device={device}
						volumes={volumes}
						index={idx}
					/>
				))}

				{devices.length === 0 && (
					<div className="text-center py-12 text-ink-faint">
						<DeviceMobile className="size-12 mx-auto mb-3 opacity-20" />
						<p className="text-sm">No devices connected</p>
					</div>
				)}
			</div>
		</div>
	);
}

interface DeviceCardProps {
	device: any;
	volumes: any[];
	index: number;
}

function DeviceCard({ device, volumes, index }: DeviceCardProps) {
	const deviceIconSrc = getDeviceIcon(device.os, device.hardware_model);

	// MOCK: Calculate storage contribution for this device
	const deviceVolumes = volumes.filter((v) => v.device_id === device.id);
	const storageContribution = deviceVolumes.reduce(
		(sum, v) => sum + v.total_capacity,
		0
	);

	// MOCK: Estimate AI compute based on device
	const aiTops = (() => {
		if (device.os === "MacOS" && device.hardware_model?.includes("M3")) return 35;
		if (device.os === "MacOS") return 18;
		if (device.os === "Windows") return 25;
		if (device.os === "IOs") return 15;
		return 0;
	})();

	const formatTime = (dateStr: string) => {
		const date = new Date(dateStr);
		const now = Date.now();
		const diff = now - date.getTime();
		const minutes = Math.floor(diff / 60000);
		const hours = Math.floor(diff / 3600000);
		const days = Math.floor(diff / 86400000);

		if (minutes < 60) return `${minutes}m ago`;
		if (hours < 24) return `${hours}h ago`;
		return `${days}d ago`;
	};

	return (
		<motion.div
			initial={{ opacity: 0, x: -20 }}
			animate={{ opacity: 1, x: 0 }}
			transition={{ delay: index * 0.05 }}
			className="flex items-center gap-3 p-3 rounded-lg hover:bg-app-hover transition-colors cursor-pointer"
		>
			<img src={deviceIconSrc} alt={device.os} className="size-10 opacity-80" />

			<div className="flex-1 min-w-0">
				<div className="flex items-center gap-2">
					<span className="font-medium text-ink truncate">{device.name}</span>
					{device.is_online && (
						<motion.div
							animate={{ scale: [1, 1.2, 1], opacity: [1, 0.6, 1] }}
							transition={{ duration: 2, repeat: Infinity }}
							className="size-1.5 rounded-full bg-accent"
						/>
					)}
				</div>

				<div className="text-sm text-ink-dull mt-0.5">
					{storageContribution > 0 ? formatBytes(storageContribution) : (
						device.is_online ? "Online" : `Offline • ${formatTime(device.last_seen_at)}`
					)}
				</div>
			</div>
		</motion.div>
	);
}
