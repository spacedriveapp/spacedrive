import { View, ViewProps } from "react-native";
import { GestureHandlerRootView } from "react-native-gesture-handler";
import { SafeAreaProvider } from "react-native-safe-area-context";
import { StatusBar } from "expo-status-bar";
import { SpacedriveProvider } from "./client";
import { RootNavigator } from "./navigation";
import "./global.css";

// Type workaround for GestureHandlerRootView children prop
const GestureRoot = GestureHandlerRootView as React.ComponentType<
	ViewProps & { children?: React.ReactNode }
>;

export default function App() {
	return (
		<GestureRoot style={{ flex: 1 }}>
			<SafeAreaProvider>
				<StatusBar style="light" />
				<SpacedriveProvider deviceName="Spacedrive Mobile">
					<RootNavigator />
				</SpacedriveProvider>
			</SafeAreaProvider>
		</GestureRoot>
	);
}
