import { WifiHigh } from "@phosphor-icons/react";
import { useNavigate } from "react-router-dom";
import { useLibraryQuery, getDeviceIcon } from "../../context";
import { SpaceItem } from "./SpaceItem";
import { GroupHeader } from "./GroupHeader";

interface DevicesGroupProps {
	isCollapsed: boolean;
	onToggle: () => void;
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
			<GroupHeader label="Devices" isCollapsed={isCollapsed} onToggle={onToggle} />

			{/* Items */}
			{!isCollapsed && (
				<div className="space-y-0.5">
					{isLoading ? (
						<div className="px-2 py-1 text-xs text-sidebar-ink-faint">Loading...</div>
					) : !devices || devices.length === 0 ? (
						<div className="px-2 py-1 text-xs text-sidebar-ink-faint">No devices</div>
					) : (
						devices.map((device, index) => {
							// Create a minimal SpaceItem structure for the device
							const deviceItem = {
								id: device.id,
								item_type: "Overview" as const,
							};

							return (
								<SpaceItem
									key={device.id}
									item={deviceItem as any}
									customIcon={getDeviceIcon(device)}
									customLabel={device.name}
									allowInsertion={false}
									isLastItem={index === devices.length - 1}
									className="text-sidebar-inkDull"
									rightComponent={
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
									}
								/>
							);
						})
					)}
				</div>
			)}
		</div>
	);
}
