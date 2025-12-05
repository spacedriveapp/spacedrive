import React from "react";
import {
	createDrawerNavigator,
	DrawerContentComponentProps,
} from "@react-navigation/drawer";
import { TabNavigator } from "./TabNavigator";
import { SidebarContent } from "../components/sidebar/SidebarContent";
import type { DrawerParamList } from "./types";

const Drawer = createDrawerNavigator<DrawerParamList>();

export function DrawerNavigator() {
	return (
		<Drawer.Navigator
			drawerContent={(props: DrawerContentComponentProps) => (
				<SidebarContent {...props} />
			)}
			screenOptions={{
				headerShown: false,
				drawerType: "slide",
				drawerStyle: {
					width: 280,
					backgroundColor: "hsl(235, 15%, 16%)",
				},
				overlayColor: "rgba(0, 0, 0, 0.5)",
				swipeEdgeWidth: 50,
			}}
		>
			<Drawer.Screen name="Tabs" component={TabNavigator} />
		</Drawer.Navigator>
	);
}
