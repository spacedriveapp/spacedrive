import { WifiHigh, WifiNoneIcon, WifiSlashIcon } from "@phosphor-icons/react";
import { useNormalizedQuery, getDeviceIcon } from "../../context";
import { useExplorer } from "../Explorer/context";
import { SpaceItem } from "./SpaceItem";
import { GroupHeader } from "./GroupHeader";
import type { ListLibraryDevicesInput, LibraryDeviceInfo } from "@sd/ts-client";

interface DevicesGroupProps {
	isCollapsed: boolean;
	onToggle: () => void;
	sortableAttributes?: any;
	sortableListeners?: any;
}

export function DevicesGroup({
	isCollapsed,
	onToggle,
	sortableAttributes,
	sortableListeners,
}: DevicesGroupProps) {
	const { navigateToView } = useExplorer();

	// Use normalized query for automatic updates when device events are emitted
	const { data: devices, isLoading } = useNormalizedQuery<
		ListLibraryDevicesInput,
		LibraryDeviceInfo[]
	>({
		wireMethod: "query:devices.list",
		input: {
			include_offline: true,
			include_details: false,
			show_paired: true,
		},
		resourceType: "device",
	});

	return (
		<div>
			<GroupHeader
				label="Devices"
				isCollapsed={isCollapsed}
				onToggle={onToggle}
				sortableAttributes={sortableAttributes}
				sortableListeners={sortableListeners}
			/>

			{/* Items */}
			{!isCollapsed && (
				<div className="space-y-0.5">
					{isLoading ? (
						<div className="px-2 py-1 text-xs text-sidebar-ink-faint">
							Loading...
						</div>
					) : !devices || devices.length === 0 ? (
						<div className="px-2 py-1 text-xs text-sidebar-ink-faint">
							No devices
						</div>
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
									onClick={() => navigateToView("device", device.id)}
									allowInsertion={false}
									isLastItem={index === devices.length - 1}
									className="text-sidebar-inkDull"
									rightComponent={
										<div className="flex items-center gap-1">
											{!device.is_current &&
												!device.is_connected && (
													<WifiSlashIcon
														size={12}
														weight="bold"
														className="text-ink-dull"
													/>
												)}
											{!device.is_current &&
												device.is_connected && (
													<WifiHigh
														size={12}
														weight="bold"
														className="text-accent"
													/>
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
