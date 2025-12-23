import React, { useState } from "react";
import { View, ViewProps } from "react-native";
import { GestureHandlerRootView } from "react-native-gesture-handler";
import { SafeAreaProvider } from "react-native-safe-area-context";
import { StatusBar } from "expo-status-bar";
import { SpacedriveProvider } from "./client";
import { RootNavigator } from "./navigation";
import { AppResetContext } from "./contexts";
import "./global.css";

// Type workaround for GestureHandlerRootView children prop
const GestureRoot = GestureHandlerRootView as React.ComponentType<
	ViewProps & { children?: React.ReactNode }
>;

export default function App() {
	const [resetKey, setResetKey] = useState(0);

	const resetApp = () => {
		setResetKey((prev) => prev + 1);
	};

	return (
		<GestureRoot style={{ flex: 1 }}>
			<SafeAreaProvider>
				<StatusBar style="light" />
				<AppResetContext.Provider value={{ resetApp }}>
					<SpacedriveProvider key={resetKey} deviceName="Spacedrive Mobile">
						<RootNavigator />
					</SpacedriveProvider>
				</AppResetContext.Provider>
			</SafeAreaProvider>
		</GestureRoot>
	);
}
