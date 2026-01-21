import React from "react";
import { createNativeStackNavigator } from "@react-navigation/native-stack";
import { BrowseScreen } from "../../screens/browse/BrowseScreen";
import { ExplorerScreen } from "../../screens/explorer/ExplorerScreen";
import type { BrowseStackParamList } from "../types";

const Stack = createNativeStackNavigator<BrowseStackParamList>();

export function BrowseStack() {
	return (
		<Stack.Navigator screenOptions={{ headerShown: false }}>
			<Stack.Screen name="BrowseHome" component={BrowseScreen} />
			<Stack.Screen name="Explorer" component={ExplorerScreen} />
		</Stack.Navigator>
	);
}
