import { Stack } from "expo-router";
import { useState } from "react";
import { GestureHandlerRootView } from "react-native-gesture-handler";
import { SafeAreaProvider } from "react-native-safe-area-context";
import { SpacedriveProvider } from "../client";
import { AppResetContext } from "../contexts";
import "../global.css";

export default function RootLayout() {
  const [resetKey, setResetKey] = useState(0);

  const resetApp = () => {
    setResetKey((prev) => prev + 1);
  };

  return (
    <GestureHandlerRootView className="bg-sidebar" style={{ flex: 1 }}>
      <SafeAreaProvider>
        <AppResetContext.Provider value={{ resetApp }}>
          <SpacedriveProvider key={resetKey}>
            <Stack screenOptions={{ headerShown: false }}>
              <Stack.Screen name="(drawer)" />
              <Stack.Screen
                name="search"
                options={{
                  presentation: "modal",
                  animation: "slide_from_bottom",
                }}
              />
            </Stack>
          </SpacedriveProvider>
        </AppResetContext.Provider>
      </SafeAreaProvider>
    </GestureHandlerRootView>
  );
}
