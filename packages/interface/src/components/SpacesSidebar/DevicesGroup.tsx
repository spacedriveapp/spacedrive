import { Trash, WifiHigh, WifiSlashIcon } from "@phosphor-icons/react";
import type { Device, ListLibraryDevicesInput } from "@sd/ts-client";
import {
  getDeviceIcon,
  useCoreMutation,
  useNormalizedQuery,
} from "../../contexts/SpacedriveContext";
import { useExplorer } from "../../routes/explorer/context";
import { GroupHeader } from "./GroupHeader";
import { SpaceItem } from "./SpaceItem";

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
  const { navigateToView, loadPreferencesForSpaceItem } = useExplorer();

  // Use normalized query for automatic updates when device events are emitted
  const { data: devices, isLoading } = useNormalizedQuery<
    ListLibraryDevicesInput,
    Device[]
  >({
    wireMethod: "query:devices.list",
    input: {
      include_offline: true,
      include_details: false,
      show_paired: true,
    },
    resourceType: "device",
  });

  // Mutation for unpairing devices
  const revokeDevice = useCoreMutation("network.device.revoke");

  // Handler for device context menu
  const handleDeviceContextMenu =
    (device: Device) => async (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();

      // Only show context menu for non-current devices
      if (device.is_current) return;

      // Create context menu items for this device
      const items = [
        {
          icon: Trash,
          label: "Unpair Device",
          onClick: async () => {
            await revokeDevice.mutateAsync({
              device_id: device.id,
              remove_from_library: false, // Keep device in library
            });
          },
          variant: "default" as const,
        },
        {
          icon: Trash,
          label: "Remove Device Completely",
          onClick: async () => {
            await revokeDevice.mutateAsync({
              device_id: device.id,
              remove_from_library: true, // Remove from library too
            });
          },
          variant: "danger" as const,
        },
      ];

      // Show platform-appropriate context menu
      if (window.__SPACEDRIVE__?.showContextMenu) {
        // Tauri native menu
        await window.__SPACEDRIVE__.showContextMenu(items, {
          x: e.clientX,
          y: e.clientY,
        });
      }
      // For web, we'd need to implement a Radix-based context menu
      // but for now, just call the action directly or show an alert
    };

  return (
    <div>
      <GroupHeader
        isCollapsed={isCollapsed}
        label="Devices"
        onToggle={onToggle}
        sortableAttributes={sortableAttributes}
        sortableListeners={sortableListeners}
      />

      {/* Items */}
      {!isCollapsed && (
        <div className="space-y-0.5">
          {isLoading ? (
            <div className="px-2 py-1 text-sidebar-ink-faint text-xs">
              Loading...
            </div>
          ) : !devices || devices.length === 0 ? (
            <div className="px-2 py-1 text-sidebar-ink-faint text-xs">
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
                  allowInsertion={false}
                  className="text-sidebar-inkDull"
                  customIcon={getDeviceIcon(device)}
                  customLabel={device.name}
                  isLastItem={index === devices.length - 1}
                  item={deviceItem as any}
                  key={device.id}
                  onClick={() => {
                    loadPreferencesForSpaceItem(`device:${device.id}`);
                    navigateToView("device", device.id);
                  }}
                  onContextMenu={handleDeviceContextMenu(device)}
                  rightComponent={
                    <div className="flex items-center gap-1">
                      {!(device.is_current || device.is_connected) && (
                        <WifiSlashIcon
                          className="text-ink-dull"
                          size={12}
                          weight="bold"
                        />
                      )}
                      {!device.is_current && device.is_connected && (
                        <WifiHigh
                          className="text-accent"
                          size={12}
                          weight="bold"
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
