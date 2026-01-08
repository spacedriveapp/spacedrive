import type { DrawerContentComponentProps } from "@react-navigation/drawer";
import type React from "react";
import { Pressable, ScrollView, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { useCoreQuery, useSpacedriveClient } from "../../client";
import { useSidebarStore } from "../../stores";

interface SidebarSectionProps {
  title: string;
  isCollapsed: boolean;
  onToggle: () => void;
  children: React.ReactNode;
}

function SidebarSection({
  title,
  isCollapsed,
  onToggle,
  children,
}: SidebarSectionProps) {
  return (
    <View className="mb-4">
      <Pressable
        className="flex-row items-center justify-between py-2"
        onPress={onToggle}
      >
        <Text className="text-ink-dull text-xs uppercase tracking-wide">
          {title}
        </Text>
        <Text className="text-ink-faint text-xs">
          {isCollapsed ? "▶" : "▼"}
        </Text>
      </Pressable>
      {!isCollapsed && children}
    </View>
  );
}

export function SidebarContent({ navigation }: DrawerContentComponentProps) {
  const insets = useSafeAreaInsets();
  const client = useSpacedriveClient();
  const {
    currentLibraryId,
    setCurrentLibrary: setStoreLibrary,
    isGroupCollapsed,
    toggleGroup,
  } = useSidebarStore();

  // Fetch libraries
  const { data: libraries } = useCoreQuery("libraries.list", {
    include_stats: false,
  });

  // Handler that syncs library ID to both store and client
  const handleSelectLibrary = (libraryId: string) => {
    console.log("[SidebarContent] Selecting library:", libraryId);
    setStoreLibrary(libraryId);
    client.setCurrentLibrary(libraryId);
  };

  const navigateAndClose = (screen: string) => {
    navigation.navigate(screen);
    navigation.closeDrawer();
  };

  return (
    <ScrollView
      className="flex-1 bg-sidebar-box"
      contentContainerStyle={{
        paddingTop: insets.top + 16,
        paddingBottom: insets.bottom + 16,
        paddingHorizontal: 16,
      }}
    >
      {/* Logo/Title */}
      <View className="mb-6">
        <Text className="font-bold text-ink text-xl">Spacedrive</Text>
        <Text className="text-ink-faint text-sm">Mobile V2</Text>
      </View>

      {/* Libraries Section */}
      <SidebarSection
        isCollapsed={isGroupCollapsed("libraries")}
        onToggle={() => toggleGroup("libraries")}
        title="Libraries"
      >
        {libraries && Array.isArray(libraries) && libraries.length > 0 ? (
          libraries.map((lib: any) => (
            <Pressable
              className={`mb-1 rounded-md px-3 py-2.5 ${
                currentLibraryId === lib.id ? "bg-sidebar-button" : ""
              }`}
              key={lib.id}
              onPress={() => handleSelectLibrary(lib.id)}
            >
              <Text
                className={`${
                  currentLibraryId === lib.id ? "text-ink" : "text-ink-dull"
                }`}
              >
                {lib.name}
              </Text>
            </Pressable>
          ))
        ) : (
          <Text className="py-2 text-ink-faint text-sm">No libraries</Text>
        )}

        <Pressable className="mt-2 rounded-md border border-sidebar-line border-dashed px-3 py-2">
          <Text className="text-ink-faint text-sm">+ Create Library</Text>
        </Pressable>
      </SidebarSection>

      {/* Locations Section */}
      <SidebarSection
        isCollapsed={isGroupCollapsed("locations")}
        onToggle={() => toggleGroup("locations")}
        title="Locations"
      >
        <Text className="py-2 text-ink-faint text-sm">
          Select a library to view locations
        </Text>
      </SidebarSection>

      {/* Tags Section */}
      <SidebarSection
        isCollapsed={isGroupCollapsed("tags")}
        onToggle={() => toggleGroup("tags")}
        title="Tags"
      >
        <Text className="py-2 text-ink-faint text-sm">
          Select a library to view tags
        </Text>
      </SidebarSection>

      {/* Divider */}
      <View className="my-4 h-px bg-sidebar-line" />

      {/* Quick Links */}
      <View>
        <Pressable
          className="mb-1 rounded-md px-3 py-2.5"
          onPress={() => navigateAndClose("OverviewTab")}
        >
          <Text className="text-ink-dull">Overview</Text>
        </Pressable>
        <Pressable
          className="mb-1 rounded-md px-3 py-2.5"
          onPress={() => navigateAndClose("NetworkTab")}
        >
          <Text className="text-ink-dull">Network</Text>
        </Pressable>
        <Pressable
          className="mb-1 rounded-md px-3 py-2.5"
          onPress={() => navigateAndClose("SettingsTab")}
        >
          <Text className="text-ink-dull">Settings</Text>
        </Pressable>
      </View>
    </ScrollView>
  );
}
