import { CaretRight, Desktop, WifiHigh } from "@phosphor-icons/react";
import clsx from "clsx";
import { useNavigate } from "react-router-dom";
import { useLibraryQuery } from "../../context";
import NodeIcon from "@sd/assets/icons/Node.png";
import LaptopIcon from "@sd/assets/icons/Laptop.png";
import MobileIcon from "@sd/assets/icons/Mobile.png";
import PCIcon from "@sd/assets/icons/PC.png";

interface DevicesGroupProps {
	isCollapsed: boolean;
	onToggle: () => void;
}

// Helper to get icon based on OS
function getDeviceIcon(os: string): string {
	const osLower = os.toLowerCase();
	if (osLower.includes("mac") || osLower.includes("darwin")) {
		return LaptopIcon;
	}
	if (osLower.includes("ios") || osLower.includes("android")) {
		return MobileIcon;
	}
	if (osLower.includes("windows") || osLower.includes("linux")) {
		return PCIcon;
	}
	return NodeIcon;
}

export function DevicesGroup({ isCollapsed, onToggle }: DevicesGroupProps) {
	const navigate = useNavigate();

	const { data: devices, isLoading } = useLibraryQuery({
		type: "devices.list",
		input: {
			include_offline: true,
			include_details: false,
			show_paired: true,
		},
	});

	return (
		<div>
			{/* Header */}
			<button
				onClick={onToggle}
				className="mb-1 flex w-full cursor-default items-center gap-2 px-1 text-xs font-semibold uppercase tracking-wider text-sidebar-ink-faint hover:text-sidebar-ink"
			>
				<CaretRight
					className={clsx("transition-transform", !isCollapsed && "rotate-90")}
					size={10}
					weight="bold"
				/>
				<span>Devices</span>
				{devices && devices.length > 0 && (
					<span className="ml-auto text-sidebar-ink-faint">{devices.length}</span>
				)}
			</button>

			{/* Items */}
			{!isCollapsed && (
				<div className="space-y-0.5">
					{isLoading ? (
						<div className="px-2 py-1 text-xs text-sidebar-ink-faint">Loading...</div>
					) : !devices || devices.length === 0 ? (
						<div className="px-2 py-1 text-xs text-sidebar-ink-faint">No devices</div>
					) : (
						devices.map((device) => (
							<button
								key={device.id}
								onClick={() => navigate(`/device/${device.id}`)}
								className={clsx(
									"flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm font-medium",
									device.is_current
										? "bg-sidebar-selected/30 text-sidebar-ink"
										: "text-sidebar-inkDull hover:text-sidebar-ink hover:bg-sidebar-box transition-colors"
								)}
							>
								{/* Device Icon */}
								<img src={getDeviceIcon(device.os)} alt="" className="size-4" />

								{/* Device Name */}
								<span className="flex-1 truncate text-left">{device.name}</span>

								{/* Status Indicators */}
								<div className="flex items-center gap-1">
									{/* Paired indicator (network icon) */}
									{device.is_paired && (
										<WifiHigh
											size={12}
											weight="bold"
											className="text-accent"
											title="Paired via network"
										/>
									)}

									{/* Offline indicator */}
									{!device.is_online && !device.is_connected && (
										<span className="text-xs text-ink-faint">Offline</span>
									)}

									{/* Connected indicator for paired devices */}
									{device.is_paired && device.is_connected && (
										<span className="text-xs text-accent">Connected</span>
									)}
								</div>
							</button>
						))
					)}
				</div>
			)}
		</div>
	);
}
