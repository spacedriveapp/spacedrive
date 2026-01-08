import { DrawerActions, useNavigation } from "@react-navigation/native";
import type { LibraryInfoOutput } from "@sd/ts-client";
import { useState } from "react";
import { Pressable, ScrollView, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { useNormalizedQuery } from "../../client";
import { LibrarySwitcherPanel } from "../../components/LibrarySwitcherPanel";
import { PairingPanel } from "../../components/PairingPanel";
import { HeroStats, PairedDevices, StorageOverview } from "./components";

export function OverviewScreen() {
  const insets = useSafeAreaInsets();
  const navigation = useNavigation();
  const [showPairing, setShowPairing] = useState(false);
  const [showLibrarySwitcher, setShowLibrarySwitcher] = useState(false);

  // Fetch library info with real-time statistics updates
  const {
    data: libraryInfo,
    isLoading,
    error,
  } = useNormalizedQuery<null, LibraryInfoOutput>({
    wireMethod: "query:libraries.info",
    input: null,
    resourceType: "library",
  });

  const openDrawer = () => {
    navigation.dispatch(DrawerActions.openDrawer());
  };

  if (isLoading || !libraryInfo) {
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
          <Text className="font-bold text-ink text-xl">
            {libraryInfo?.name || "Loading..."}
          </Text>
          <Pressable
            className="-mr-2 rounded-lg p-2 active:bg-app-hover"
            onPress={() => setShowPairing(true)}
          >
            <Text className="text-accent text-xl">◊</Text>
          </Pressable>
        </View>

        <View className="items-center justify-center py-12">
          <Text className="text-ink-dull">Loading library statistics...</Text>
        </View>
      </ScrollView>
    );
  }

  if (error) {
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
          <Text className="font-bold text-ink text-xl">Overview</Text>
          <Pressable
            className="-mr-2 rounded-lg p-2 active:bg-app-hover"
            onPress={() => setShowPairing(true)}
          >
            <Text className="text-accent text-xl">◊</Text>
          </Pressable>
        </View>

        <View className="items-center justify-center py-12">
          <Text className="font-semibold text-red-500">Error</Text>
          <Text className="mt-2 text-ink-dull">{String(error)}</Text>
        </View>
      </ScrollView>
    );
  }

  const stats = libraryInfo.statistics;

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
        <Pressable
          className="flex-1 items-center active:opacity-70"
          onPress={() => setShowLibrarySwitcher(true)}
        >
          <Text className="font-bold text-ink text-xl">{libraryInfo.name}</Text>
        </Pressable>
        <Pressable
          className="-mr-2 rounded-lg p-2 active:bg-app-hover"
          onPress={() => setShowPairing(true)}
        >
          <Text className="text-accent text-xl">◊</Text>
        </Pressable>
      </View>

      {/* Hero Stats */}
      <HeroStats
        deviceCount={stats.device_count}
        locationCount={stats.location_count}
        tagCount={stats.tag_count}
        totalFiles={Number(stats.total_files)}
        totalStorage={stats.total_capacity}
        uniqueContentCount={Number(stats.unique_content_count)}
        usedStorage={stats.total_capacity - stats.available_capacity}
      />

      {/* Paired Devices */}
      <PairedDevices />

      {/* Storage Volumes */}
      <StorageOverview />

      {/* Pairing Panel */}
      <PairingPanel
        isOpen={showPairing}
        onClose={() => setShowPairing(false)}
      />

      {/* Library Switcher Panel */}
      <LibrarySwitcherPanel
        isOpen={showLibrarySwitcher}
        onClose={() => setShowLibrarySwitcher(false)}
      />
    </ScrollView>
  );
}
