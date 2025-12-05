import React from "react";
import { createBottomTabNavigator } from "@react-navigation/bottom-tabs";
import { View, Text, Platform } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { OverviewStack } from "./stacks/OverviewStack";
import { BrowseStack } from "./stacks/BrowseStack";
import { NetworkStack } from "./stacks/NetworkStack";
import { SettingsStack } from "./stacks/SettingsStack";
import type { TabParamList } from "./types";

const Tab = createBottomTabNavigator<TabParamList>();

// Simple icon components (replace with phosphor-react-native later)
const TabIcon = ({ name, focused }: { name: string; focused: boolean }) => (
	<View
		className={`items-center justify-center ${focused ? "opacity-100" : "opacity-50"}`}
	>
		<View
			className={`h-6 w-6 rounded-md ${focused ? "bg-accent" : "bg-ink-faint"}`}
		/>
		<Text
			className={`text-[10px] mt-1 ${focused ? "text-accent" : "text-ink-faint"}`}
		>
			{name}
		</Text>
	</View>
);

export function TabNavigator() {
	const insets = useSafeAreaInsets();
	const tabBarHeight = Platform.OS === "ios" ? 80 : 60;

	return (
		<Tab.Navigator
			screenOptions={{
				headerShown: false,
				tabBarStyle: {
					height:
						tabBarHeight +
						(Platform.OS === "ios" ? 0 : insets.bottom),
					paddingBottom: Platform.OS === "ios" ? insets.bottom : 8,
					paddingTop: 8,
					backgroundColor: "hsl(235, 10%, 6%)",
					borderTopColor: "hsl(235, 15%, 23%)",
					borderTopWidth: 1,
				},
				tabBarShowLabel: false,
				tabBarActiveTintColor: "hsl(208, 100%, 57%)",
				tabBarInactiveTintColor: "hsl(235, 10%, 55%)",
			}}
		>
			<Tab.Screen
				name="OverviewTab"
				component={OverviewStack}
				options={{
					tabBarIcon: ({ focused }) => (
						<TabIcon name="Overview" focused={focused} />
					),
				}}
			/>
			<Tab.Screen
				name="BrowseTab"
				component={BrowseStack}
				options={{
					tabBarIcon: ({ focused }) => (
						<TabIcon name="Browse" focused={focused} />
					),
				}}
			/>
			<Tab.Screen
				name="NetworkTab"
				component={NetworkStack}
				options={{
					tabBarIcon: ({ focused }) => (
						<TabIcon name="Network" focused={focused} />
					),
				}}
			/>
			<Tab.Screen
				name="SettingsTab"
				component={SettingsStack}
				options={{
					tabBarIcon: ({ focused }) => (
						<TabIcon name="Settings" focused={focused} />
					),
				}}
			/>
		</Tab.Navigator>
	);
}
