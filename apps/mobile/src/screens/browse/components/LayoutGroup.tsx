import React, { useState } from "react";
import { View, Text, Pressable } from "react-native";
import type { LayoutGroup, LayoutItem } from "@sd/ts-client";
import { SettingsGroup } from "../../../components/primitive";
import { LayoutItem as LayoutItemComponent } from "./LayoutItem";
import { DevicesGroup } from "./DevicesGroup";
import { VolumesGroup } from "./VolumesGroup";
import { CaretDown, CaretRight } from "phosphor-react-native";

interface LayoutGroupProps {
	group: LayoutGroup;
	items: LayoutItem[];
}

export function LayoutGroupComponent({ group, items }: LayoutGroupProps) {
	const [isCollapsed, setIsCollapsed] = useState(group.is_collapsed ?? false);

	const handleToggle = () => {
		setIsCollapsed(!isCollapsed);
	};

	// System groups - use existing components
	if (group.group_type === "Devices") {
		return <DevicesGroup />;
	}

	if (group.group_type === "Volumes") {
		return <VolumesGroup />;
	}

	if (group.group_type === "Tags") {
		// Tags group not implemented yet
		return null;
	}

	// Custom/QuickAccess groups - render items
	return (
		<View className="mb-6">
			{/* Group Header */}
			<Pressable
				onPress={handleToggle}
				className="flex-row items-center justify-between px-4 mb-2"
			>
				<Text className="text-xs font-semibold text-ink-dull uppercase tracking-wider">
					{group.name}
				</Text>
				{isCollapsed ? (
					<CaretRight size={16} color="hsl(235, 10%, 55%)" weight="bold" />
				) : (
					<CaretDown size={16} color="hsl(235, 10%, 55%)" weight="bold" />
				)}
			</Pressable>

			{/* Group Items */}
			{!isCollapsed && items.length > 0 && (
				<SettingsGroup>
					{items
						.filter((item) => item.item_type !== "Overview")
						.map((item) => (
							<LayoutItemComponent key={item.id} item={item} />
						))}
				</SettingsGroup>
			)}
		</View>
	);
}
