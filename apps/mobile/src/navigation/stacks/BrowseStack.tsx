import { createNativeStackNavigator } from "@react-navigation/native-stack";
import { BrowseScreen } from "../../screens/browse/BrowseScreen";
import type { BrowseStackParamList } from "../types";

const Stack = createNativeStackNavigator<BrowseStackParamList>();

export function BrowseStack() {
  return (
    <Stack.Navigator screenOptions={{ headerShown: false }}>
      <Stack.Screen component={BrowseScreen} name="BrowseHome" />
      {/* Add Location, Tag, Explorer screens later */}
    </Stack.Navigator>
  );
}
