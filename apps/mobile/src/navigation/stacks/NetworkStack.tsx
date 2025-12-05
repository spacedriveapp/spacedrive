import React from "react";
import { createNativeStackNavigator } from "@react-navigation/native-stack";
import { NetworkScreen } from "../../screens/network/NetworkScreen";
import type { NetworkStackParamList } from "../types";

const Stack = createNativeStackNavigator<NetworkStackParamList>();

export function NetworkStack() {
	return (
		<Stack.Navigator screenOptions={{ headerShown: false }}>
			<Stack.Screen name="Network" component={NetworkScreen} />
		</Stack.Navigator>
	);
}
