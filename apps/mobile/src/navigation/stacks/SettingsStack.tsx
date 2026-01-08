import { createNativeStackNavigator } from "@react-navigation/native-stack";
import { SettingsScreen } from "../../screens/settings/SettingsScreen";
import type { SettingsStackParamList } from "../types";

const Stack = createNativeStackNavigator<SettingsStackParamList>();

export function SettingsStack() {
  return (
    <Stack.Navigator screenOptions={{ headerShown: false }}>
      <Stack.Screen component={SettingsScreen} name="SettingsHome" />
    </Stack.Navigator>
  );
}
