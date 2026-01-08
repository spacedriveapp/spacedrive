import { createNativeStackNavigator } from "@react-navigation/native-stack";
import { OverviewScreen } from "../../screens/overview/OverviewScreen";
import type { OverviewStackParamList } from "../types";

const Stack = createNativeStackNavigator<OverviewStackParamList>();

export function OverviewStack() {
  return (
    <Stack.Navigator screenOptions={{ headerShown: false }}>
      <Stack.Screen component={OverviewScreen} name="Overview" />
    </Stack.Navigator>
  );
}
