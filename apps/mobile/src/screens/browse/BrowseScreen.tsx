import clsx from "clsx";
import type React from "react";
import { useState } from "react";
import { Pressable, ScrollView, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { useLibraryQuery } from "../../client";
import { Card } from "../../components/primitive";

// Collapsible Group Component
interface CollapsibleGroupProps {
  title: string;
  children: React.ReactNode;
  defaultCollapsed?: boolean;
}

function CollapsibleGroup({
  title,
  children,
  defaultCollapsed = false,
}: CollapsibleGroupProps) {
  const [isCollapsed, setIsCollapsed] = useState(defaultCollapsed);

  return (
    <View className="mb-5">
      <Pressable
        className="mb-2 flex-row items-center px-1"
        onPress={() => setIsCollapsed(!isCollapsed)}
      >
        <Text className="mr-2 font-semibold text-ink-faint text-xs uppercase tracking-wider">
          {isCollapsed ? "▶" : "▼"}
        </Text>
        <Text className="font-semibold text-ink-faint text-xs uppercase tracking-wider">
          {title}
        </Text>
      </Pressable>
      {!isCollapsed && <View className="space-y-1">{children}</View>}
    </View>
  );
}

// Sidebar Item Component
interface SidebarItemProps {
  icon: string;
  label: string;
  onPress?: () => void;
  isActive?: boolean;
  color?: string;
}

function SidebarItem({
  icon,
  label,
  onPress,
  isActive = false,
  color,
}: SidebarItemProps) {
  return (
    <Pressable
      className={clsx(
        "flex-row items-center gap-2 rounded-md px-2 py-2 transition-colors",
        isActive ? "bg-sidebar-selected/30" : "active:bg-sidebar-selected/20"
      )}
      onPress={onPress}
    >
      {color ? (
        <View
          className="h-4 w-4 rounded-full"
          style={{ backgroundColor: color }}
        />
      ) : (
        <Text className="text-base">{icon}</Text>
      )}
      <Text
        className={clsx(
          "flex-1 font-medium text-sm",
          isActive ? "text-sidebar-ink" : "text-sidebar-inkDull"
        )}
      >
        {label}
      </Text>
    </Pressable>
  );
}

// Space Switcher Component
interface Space {
  id: string;
  name: string;
  color: string;
}

function SpaceSwitcher({
  spaces,
  currentSpace,
}: {
  spaces: Space[] | undefined;
  currentSpace: Space | undefined;
}) {
  const [showDropdown, setShowDropdown] = useState(false);

  return (
    <View className="mb-4">
      <Pressable
        className="flex-row items-center gap-2 rounded-lg border border-sidebar-line bg-sidebar-box px-3 py-2"
        onPress={() => setShowDropdown(!showDropdown)}
      >
        <View
          className="h-2 w-2 rounded-full"
          style={{ backgroundColor: currentSpace?.color || "#666" }}
        />
        <Text className="flex-1 font-medium text-sidebar-ink text-sm">
          {currentSpace?.name || "Select Space"}
        </Text>
        <Text className="text-sidebar-inkDull text-xs">
          {showDropdown ? "▲" : "▼"}
        </Text>
      </Pressable>

      {showDropdown && spaces && spaces.length > 0 && (
        <Card className="mt-2">
          {spaces.map((space) => (
            <Pressable
              className="flex-row items-center gap-2 px-2 py-2"
              key={space.id}
              onPress={() => setShowDropdown(false)}
            >
              <View
                className="h-2 w-2 rounded-full"
                style={{ backgroundColor: space.color }}
              />
              <Text className="text-ink text-sm">{space.name}</Text>
            </Pressable>
          ))}
        </Card>
      )}
    </View>
  );
}

export function BrowseScreen() {
  const insets = useSafeAreaInsets();

  // Fetch data using queries
  const { data: locations } = useLibraryQuery("locations.list");
  // TODO: Re-enable when backend supports these queries
  // const { data: tags } = useLibraryQuery("tags.list");
  const { data: spaces } = useLibraryQuery("spaces.list", {});

  // Mock current space (first space if available)
  const currentSpace = spaces && spaces.length > 0 ? spaces[0] : undefined;

  return (
    <ScrollView
      className="flex-1 bg-sidebar"
      contentContainerStyle={{
        paddingTop: insets.top + 16,
        paddingBottom: insets.bottom + 100,
        paddingHorizontal: 16,
      }}
    >
      {/* Header */}
      <View className="mb-6">
        <Text className="font-bold text-2xl text-ink">Browse</Text>
        <Text className="mt-1 text-ink-dull text-sm">
          Your libraries and spaces
        </Text>
      </View>

      {/* Space Switcher */}
      <SpaceSwitcher
        currentSpace={currentSpace as Space | undefined}
        spaces={spaces as Space[] | undefined}
      />

      {/* Quick Access */}
      <CollapsibleGroup title="Quick Access">
        <SidebarItem icon="🏠" isActive={true} label="Overview" />
        <SidebarItem icon="🕒" label="Recents" />
        <SidebarItem icon="❤️" label="Favorites" />
      </CollapsibleGroup>

      {/* Locations */}
      <CollapsibleGroup title="Locations">
        {locations && Array.isArray(locations) && locations.length > 0 ? (
          locations.map((loc: any) => (
            <SidebarItem icon="📁" key={loc.id} label={loc.name || "Unnamed"} />
          ))
        ) : (
          <View className="px-2 py-3">
            <Text className="text-ink-dull text-sm">No locations added</Text>
          </View>
        )}
      </CollapsibleGroup>

      {/* Devices */}
      <CollapsibleGroup title="Devices">
        <SidebarItem icon="💻" label="This Device" />
      </CollapsibleGroup>

      {/* Volumes */}
      <CollapsibleGroup title="Volumes">
        <SidebarItem icon="💾" label="Macintosh HD" />
      </CollapsibleGroup>

      {/* Tags */}
      <CollapsibleGroup title="Tags">
        <View className="px-2 py-3">
          <Text className="text-ink-dull text-sm">No tags created</Text>
        </View>
      </CollapsibleGroup>

      {/* Bottom Section */}
      <View className="mt-6 space-y-1">
        <SidebarItem icon="🔄" label="Sync Monitor" />
        <SidebarItem icon="⚙️" label="Settings" />
      </View>
    </ScrollView>
  );
}
