import React from "react";
import { View, Text, Pressable, Image } from "react-native";
import { useLibraryQuery } from "../../../client";
import { getDeviceIcon } from "@sd/ts-client";

export function PairedDevices() {
	const { data: devices, isLoading } = useLibraryQuery(
		"devices.list",
		{
			include_offline: true,
			include_details: false,
			show_paired: true,
		}
	);

	if (isLoading) {
		return (
			<View className="bg-app-box border border-app-line rounded-xl overflow-hidden mb-6">
				<View className="px-6 py-4 border-b border-app-line">
					<Text className="text-base font-semibold text-ink">
						Paired Devices
					</Text>
					<Text className="text-sm text-ink-dull mt-1">
						Loading devices...
					</Text>
				</View>
			</View>
		);
	}

	const devicesList = devices || [];
	const connectedCount = devicesList.filter((d: any) => d.is_connected).length;

	return (
		<View className="bg-app-box border border-app-line rounded-xl overflow-hidden mb-6">
			<View className="px-6 py-4 border-b border-app-line">
				<Text className="text-base font-semibold text-ink">
					Paired Devices
				</Text>
				<Text className="text-sm text-ink-dull mt-1">
					{devicesList.length}{" "}
					{devicesList.length === 1 ? "device" : "devices"} paired
					{connectedCount > 0 && ` • ${connectedCount} connected`}
				</Text>
			</View>

			<View className="p-4">
				{devicesList.map((device: any, idx: number) => (
					<DeviceCard key={device.id} device={device} />
				))}

				{devicesList.length === 0 && (
					<View className="py-12 items-center">
						<Text className="text-ink-faint text-sm">
							No paired devices
						</Text>
						<Text className="text-ink-faint text-xs mt-1">
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
		<View className="p-4 mb-3 bg-app-darkBox rounded-lg border border-app-line">
			<View className="flex-row items-center justify-between mb-2">
				<View className="flex-row items-center gap-3">
					<Image
						source={iconSource}
						className="w-10 h-10"
						style={{ resizeMode: "contain" }}
					/>
					<View>
						<Text className="font-semibold text-ink text-base">
							{device.name}
						</Text>
						<Text className="text-xs text-ink-dull mt-0.5">
							{device.device_type} • {device.os_version}
						</Text>
					</View>
				</View>
				<View
					className={`px-2 py-1 rounded-md ${
						device.is_connected
							? "bg-green-500/10 border border-green-500/30"
							: "bg-app-box border border-app-line"
					}`}
				>
					<Text
						className={`text-xs font-medium ${
							device.is_connected
								? "text-green-500"
								: "text-ink-faint"
						}`}
					>
						{device.is_connected ? "Connected" : "Offline"}
					</Text>
				</View>
			</View>

			<View className="flex-row flex-wrap gap-2">
				<View className="px-2 py-0.5 bg-app-box rounded border border-app-line">
					<Text className="text-ink-dull text-xs">
						v{device.app_version}
					</Text>
				</View>
				{device.last_seen && (
					<View className="px-2 py-0.5 bg-app-box rounded border border-app-line">
						<Text className="text-ink-dull text-xs">
							Last seen: {new Date(device.last_seen).toLocaleDateString()}
						</Text>
					</View>
				)}
			</View>
		</View>
	);
}
