import { DrawerActions, useNavigation } from "@react-navigation/native";
import { Pressable, ScrollView, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { Card } from "../../components/primitive";

export function NetworkScreen() {
  const insets = useSafeAreaInsets();
  const navigation = useNavigation();

  const openDrawer = () => {
    navigation.dispatch(DrawerActions.openDrawer());
  };

  return (
    <ScrollView
      className="flex-1 bg-app"
      contentContainerStyle={{
        paddingTop: insets.top + 16,
        paddingBottom: insets.bottom + 100,
        paddingHorizontal: 16,
      }}
    >
      {/* Header */}
      <View className="mb-6 flex-row items-center justify-between">
        <Pressable className="-ml-2 p-2" onPress={openDrawer}>
          <View className="mb-1.5 h-0.5 w-6 bg-ink" />
          <View className="mb-1.5 h-0.5 w-6 bg-ink" />
          <View className="h-0.5 w-6 bg-ink" />
        </Pressable>
        <Text className="font-bold text-2xl text-ink">Network</Text>
        <View className="w-10" />
      </View>

      {/* Network Status */}
      <Card className="mb-6">
        <View className="flex-row items-center">
          <View className="mr-3 h-3 w-3 rounded-full bg-green-500" />
          <View className="flex-1">
            <Text className="font-medium text-ink">Network Status</Text>
            <Text className="text-ink-dull text-sm">P2P enabled</Text>
          </View>
        </View>
      </Card>

      {/* Devices */}
      <View className="mb-6">
        <Text className="mb-3 font-semibold text-ink text-lg">This Device</Text>
        <Card>
          <View className="flex-row items-center">
            <View className="mr-3 h-10 w-10 items-center justify-center rounded-lg bg-accent/20">
              <Text className="text-accent">📱</Text>
            </View>
            <View>
              <Text className="font-medium text-ink">Spacedrive Mobile</Text>
              <Text className="text-ink-dull text-sm">Connected</Text>
            </View>
          </View>
        </Card>
      </View>

      {/* Nearby Devices */}
      <View className="mb-6">
        <Text className="mb-3 font-semibold text-ink text-lg">
          Nearby Devices
        </Text>
        <Card>
          <Text className="text-ink-dull">Searching for devices...</Text>
          <Text className="mt-1 text-ink-faint text-sm">
            Make sure other devices are on the same network
          </Text>
        </Card>
      </View>

      {/* Sync Status */}
      <View className="mb-6">
        <Text className="mb-3 font-semibold text-ink text-lg">Sync</Text>
        <Card className="flex-row items-center justify-between">
          <View>
            <Text className="font-medium text-ink">Sync Status</Text>
            <Text className="text-ink-dull text-sm">Up to date</Text>
          </View>
          <View className="rounded-full bg-green-500/20 px-3 py-1">
            <Text className="text-green-500 text-sm">Synced</Text>
          </View>
        </Card>
      </View>
    </ScrollView>
  );
}
