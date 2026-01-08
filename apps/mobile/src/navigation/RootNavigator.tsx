import { DefaultTheme, NavigationContainer } from "@react-navigation/native";
import { createNativeStackNavigator } from "@react-navigation/native-stack";
import { DrawerNavigator } from "./DrawerNavigator";
import type { RootStackParamList } from "./types";

const Stack = createNativeStackNavigator<RootStackParamList>();

// Dark theme for navigation
const SpacedriveTheme = {
  ...DefaultTheme,
  dark: true,
  colors: {
    ...DefaultTheme.colors,
    primary: "hsl(208, 100%, 57%)",
    background: "hsl(235, 15%, 13%)",
    card: "hsl(235, 10%, 6%)",
    text: "hsl(235, 0%, 100%)",
    border: "hsl(235, 15%, 23%)",
    notification: "hsl(208, 100%, 57%)",
  },
};

export function RootNavigator() {
  return (
    <NavigationContainer theme={SpacedriveTheme}>
      <Stack.Navigator
        screenOptions={{
          headerShown: false,
          animation: "fade",
        }}
      >
        <Stack.Screen component={DrawerNavigator} name="Main" />
        {/* Add Onboarding and Search screens later */}
      </Stack.Navigator>
    </NavigationContainer>
  );
}
