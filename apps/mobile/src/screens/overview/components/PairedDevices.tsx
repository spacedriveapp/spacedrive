import { getDeviceIcon } from "@sd/ts-client";
import { Image, Text, View } from "react-native";
import { useLibraryQuery } from "../../../client";

export function PairedDevices() {
  const { data: devices, isLoading } = useLibraryQuery("devices.list", {
    include_offline: true,
    include_details: false,
    show_paired: true,
  });

  if (isLoading) {
    return (
      <View className="mb-6 overflow-hidden rounded-xl border border-app-line bg-app-box">
        <View className="border-app-line border-b px-6 py-4">
          <Text className="font-semibold text-base text-ink">
            Paired Devices
          </Text>
          <Text className="mt-1 text-ink-dull text-sm">Loading devices...</Text>
        </View>
      </View>
    );
  }

  const devicesList = devices || [];
  const connectedCount = devicesList.filter((d: any) => d.is_connected).length;

  return (
    <View className="mb-6 overflow-hidden rounded-xl border border-app-line bg-app-box">
      <View className="border-app-line border-b px-6 py-4">
        <Text className="font-semibold text-base text-ink">Paired Devices</Text>
        <Text className="mt-1 text-ink-dull text-sm">
          {devicesList.length} {devicesList.length === 1 ? "device" : "devices"}{" "}
          paired
          {connectedCount > 0 && ` • ${connectedCount} connected`}
        </Text>
      </View>

      <View className="p-4">
        {devicesList.map((device: any, idx: number) => (
          <DeviceCard device={device} key={device.id} />
        ))}

        {devicesList.length === 0 && (
          <View className="items-center py-12">
            <Text className="text-ink-faint text-sm">No paired devices</Text>
            <Text className="mt-1 text-ink-faint text-xs">
              Pair a device to share files and sync data
            </Text>
          </View>
        )}
      </View>
    </View>
  );
}

interface DeviceCardProps {
  device: any;
}

function DeviceCard({ device }: DeviceCardProps) {
  const iconSource = getDeviceIcon(device);

  return (
    <View className="mb-3 rounded-lg border border-app-line bg-app-darkBox p-4">
      <View className="mb-2 flex-row items-center justify-between">
        <View className="flex-row items-center gap-3">
          <Image
            className="h-10 w-10"
            source={iconSource}
            style={{ resizeMode: "contain" }}
          />
          <View>
            <Text className="font-semibold text-base text-ink">
              {device.name}
            </Text>
            <Text className="mt-0.5 text-ink-dull text-xs">
              {device.device_type} • {device.os_version}
            </Text>
          </View>
        </View>
        <View
          className={`rounded-md px-2 py-1 ${
            device.is_connected
              ? "border border-green-500/30 bg-green-500/10"
              : "border border-app-line bg-app-box"
          }`}
        >
          <Text
            className={`font-medium text-xs ${
              device.is_connected ? "text-green-500" : "text-ink-faint"
            }`}
          >
            {device.is_connected ? "Connected" : "Offline"}
          </Text>
        </View>
      </View>

      <View className="flex-row flex-wrap gap-2">
        <View className="rounded border border-app-line bg-app-box px-2 py-0.5">
          <Text className="text-ink-dull text-xs">v{device.app_version}</Text>
        </View>
        {device.last_seen && (
          <View className="rounded border border-app-line bg-app-box px-2 py-0.5">
            <Text className="text-ink-dull text-xs">
              Last seen: {new Date(device.last_seen).toLocaleDateString()}
            </Text>
          </View>
        )}
      </View>
    </View>
  );
}
