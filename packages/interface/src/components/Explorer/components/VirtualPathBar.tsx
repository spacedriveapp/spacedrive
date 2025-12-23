import clsx from "clsx";
import { motion } from "framer-motion";
import { getDeviceIcon } from "../../../context";
import LaptopIcon from "@sd/assets/icons/Laptop.png";
import type { VirtualView } from "../context";

interface VirtualPathBarProps {
	view: VirtualView;
	devices: Map<string, any>;
}

/**
 * PathBar for virtual views (device listings, all devices, etc.)
 * Shows the view name with an appropriate icon instead of file path breadcrumbs
 */
export function VirtualPathBar({ view, devices }: VirtualPathBarProps) {
	const device = view.id ? devices.get(view.id) : null;
	
	// Determine label and icon based on view type
	const label = (() => {
		if (view.view === "device" && device) {
			return device.name;
		}
		if (view.view === "devices") {
			return "All Devices";
		}
		return "Virtual View";
	})();

	const icon = (() => {
		if (view.view === "device" && device) {
			return getDeviceIcon(device);
		}
		return LaptopIcon;
	})();

	return (
		<motion.div
			initial={{ width: 150 }}
			animate={{ width: Math.min(label.length * 8.5 + 70, 400) }}
			transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
			className={clsx(
				"flex items-center gap-1.5 h-8 px-3 rounded-full",
				"backdrop-blur-xl border border-sidebar-line/30",
				"bg-sidebar-box/20 transition-colors",
			)}
		>
			<img
				src={icon}
				alt=""
				className="size-5 opacity-60 flex-shrink-0"
			/>
			<span className="text-xs font-medium text-sidebar-ink whitespace-nowrap">
				{label}
			</span>
		</motion.div>
	);
}

