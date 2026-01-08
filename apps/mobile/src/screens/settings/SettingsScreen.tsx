import AsyncStorage from "@react-native-async-storage/async-storage";
import { useState } from "react";
import { Alert, Pressable, ScrollView, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { useCoreAction } from "../../client";
import {
  Button,
  Card,
  Divider,
  Input,
  SettingsGroup,
  SettingsLink,
  SettingsOption,
  SettingsSlider,
  SettingsToggle,
  Switch,
} from "../../components/primitive";
import { useAppReset } from "../../contexts";

export function SettingsScreen() {
  const insets = useSafeAreaInsets();
  const [switchValue, setSwitchValue] = useState(false);
  const [inputValue, setInputValue] = useState("");
  const [notificationsEnabled, setNotificationsEnabled] = useState(true);
  const [darkModeEnabled, setDarkModeEnabled] = useState(false);
  const [sliderValue, setSliderValue] = useState(50);

  const resetData = useCoreAction("core.reset");
  const { resetApp } = useAppReset();

  const handleResetData = () => {
    Alert.alert(
      "Reset All Data",
      "This will permanently delete all libraries, settings, and cached data. The app will refresh automatically. Are you sure?",
      [
        {
          text: "Cancel",
          style: "cancel",
        },
        {
          text: "Reset",
          style: "destructive",
          onPress: async () => {
            resetData.mutate(
              { confirm: true },
              {
                onSuccess: async () => {
                  // Clear AsyncStorage
                  await AsyncStorage.clear();

                  // Refresh the entire app
                  resetApp();
                },
                onError: (error) => {
                  Alert.alert("Error", error.message || "Failed to reset data");
                },
              }
            );
          },
        },
      ]
    );
  };

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
        <Text className="font-bold text-2xl text-ink">
          UI Primitives Showcase
        </Text>
        <Text className="mt-1 text-ink-dull text-sm">
          All available components and variants
        </Text>
      </View>

      {/* Buttons Section */}
      <View className="mb-6">
        <Text className="mb-3 font-semibold text-ink">Buttons</Text>

        <View className="gap-2">
          <Button
            onPress={() => console.log("Accent button")}
            size="md"
            variant="accent"
          >
            Accent Button
          </Button>

          <Button size="md" variant="gray">
            Gray Button
          </Button>

          <Button size="md" variant="outline">
            Outline Button
          </Button>

          <Button size="md" variant="default">
            Default Button
          </Button>

          <Button size="md" variant="subtle">
            Subtle Button
          </Button>

          <Button size="md" variant="dotted">
            Dotted Button
          </Button>

          <View className="flex-row items-center gap-2">
            <Button size="xs" variant="accent">
              XS
            </Button>
            <Button size="sm" variant="gray">
              Small
            </Button>
            <Button size="md" variant="outline">
              Medium
            </Button>
            <Button size="lg" variant="accent">
              Large
            </Button>
          </View>

          <Button disabled size="md" variant="accent">
            Disabled
          </Button>
        </View>
      </View>

      {/* Cards Section */}
      <View className="mb-6">
        <Text className="mb-3 font-semibold text-ink">Cards</Text>

        <Card className="mb-2 bg-app-box">
          <Text className="text-ink">Default Card</Text>
          <Text className="mt-1 text-ink-dull text-sm">With subtitle text</Text>
        </Card>

        <Card className="mb-2 border border-accent/30 bg-accent/10">
          <Text className="font-medium text-accent">Accent Card</Text>
        </Card>

        <Card className="bg-sidebar-box">
          <Text className="text-sidebar-ink">Sidebar Card</Text>
        </Card>
      </View>

      {/* Inputs Section */}
      <View className="mb-6">
        <Text className="mb-3 font-semibold text-ink">Inputs</Text>

        <View className="gap-3">
          <Input
            onChangeText={setInputValue}
            placeholder="Default input"
            value={inputValue}
          />

          <Input
            onChangeText={setInputValue}
            placeholder="Outline variant"
            value={inputValue}
            variant="outline"
          />

          <Input
            onChangeText={setInputValue}
            placeholder="Filled variant"
            value={inputValue}
            variant="filled"
          />

          <Input placeholder="Small size" size="sm" />

          <Input placeholder="Large size" size="lg" />

          <Input disabled placeholder="Disabled input" value="Cannot edit" />
        </View>
      </View>

      {/* Switch Section */}
      <View className="mb-6">
        <Text className="mb-3 font-semibold text-ink">Switches</Text>

        <View className="space-y-3">
          <View className="flex-row items-center justify-between">
            <Text className="text-ink">Enabled Switch</Text>
            <Switch onValueChange={setSwitchValue} value={switchValue} />
          </View>

          <View className="flex-row items-center justify-between">
            <Text className="text-ink">Always On</Text>
            <Switch onValueChange={() => {}} value={true} />
          </View>

          <View className="flex-row items-center justify-between">
            <Text className="text-ink-dull">Always Off</Text>
            <Switch onValueChange={() => {}} value={false} />
          </View>
        </View>
      </View>

      {/* Dividers Section */}
      <View className="mb-6">
        <Text className="mb-3 font-semibold text-ink">Dividers</Text>

        <Text className="text-ink">Section One</Text>
        <Divider />
        <Text className="text-ink">Section Two</Text>
        <Divider />
        <Text className="text-ink">Section Three</Text>
      </View>

      {/* Typography Section */}
      <View className="mb-6">
        <Text className="mb-3 font-semibold text-ink">Typography</Text>

        <View className="space-y-2">
          <Text className="font-bold text-3xl text-ink">Heading 1</Text>
          <Text className="font-bold text-2xl text-ink">Heading 2</Text>
          <Text className="font-semibold text-ink text-xl">Heading 3</Text>
          <Text className="font-medium text-ink text-lg">Heading 4</Text>
          <Text className="text-base text-ink">Body Text</Text>
          <Text className="text-ink-dull text-sm">Secondary Text</Text>
          <Text className="text-ink-faint text-xs uppercase tracking-wider">
            Label Text
          </Text>
        </View>
      </View>

      {/* Colors Section */}
      <View className="mb-6">
        <Text className="mb-3 font-semibold text-ink">Color System</Text>

        <View className="space-y-4">
          {/* Accent Colors */}
          <View>
            <Text className="mb-2 text-ink-dull text-xs uppercase">Accent</Text>
            <View className="flex-row flex-wrap gap-4">
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-accent"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">DEFAULT</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-accent-faint"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">faint</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-accent-deep"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">deep</Text>
              </View>
            </View>
          </View>

          {/* Ink Colors */}
          <View>
            <Text className="mb-2 text-ink-dull text-xs uppercase">
              Ink (Text)
            </Text>
            <View className="flex-row flex-wrap gap-4">
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-ink"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">DEFAULT</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-ink-dull"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">dull</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-ink-faint"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">faint</Text>
              </View>
            </View>
          </View>

          {/* Sidebar Colors */}
          <View>
            <Text className="mb-2 text-ink-dull text-xs uppercase">
              Sidebar
            </Text>
            <View className="flex-row flex-wrap gap-4">
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-sidebar"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">DEFAULT</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-sidebar-box"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">box</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-sidebar-line"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">line</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-sidebar-ink"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">ink</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-sidebar-ink-dull"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">inkDull</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-sidebar-ink-faint"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">inkFaint</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-sidebar-divider"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">divider</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-sidebar-button"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">button</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-sidebar-selected"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">selected</Text>
              </View>
            </View>
          </View>

          {/* App Colors */}
          <View>
            <Text className="mb-2 text-ink-dull text-xs uppercase">App</Text>
            <View className="flex-row flex-wrap gap-4">
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg border border-app-line bg-app"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">DEFAULT</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-app-box"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">box</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-app-dark-box"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">darkBox</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-app-overlay"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">overlay</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-app-line"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">line</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-app-frame"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">frame</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-app-button"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">button</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-app-hover"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">hover</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-app-selected"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">selected</Text>
              </View>
            </View>
          </View>

          {/* Menu Colors */}
          <View>
            <Text className="mb-2 text-ink-dull text-xs uppercase">Menu</Text>
            <View className="flex-row flex-wrap gap-4">
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-menu"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">DEFAULT</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-menu-line"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">line</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-menu-hover"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">hover</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-menu-selected"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">selected</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-menu-shade"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">shade</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-menu-ink"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">ink</Text>
              </View>
              <View className="items-center">
                <View
                  className="mb-2 rounded-lg bg-menu-faint"
                  style={{ width: 80, height: 80 }}
                />
                <Text className="text-[10px] text-ink-faint">faint</Text>
              </View>
            </View>
          </View>
        </View>
      </View>

      {/* Spacing Section */}
      <View className="mb-6">
        <Text className="mb-3 font-semibold text-ink">Spacing Scale</Text>

        <View className="space-y-2">
          <View className="flex-row items-center gap-3">
            <View className="h-4 w-1 bg-accent" />
            <Text className="text-ink-dull text-sm">4px (1)</Text>
          </View>
          <View className="flex-row items-center gap-3">
            <View className="h-4 w-2 bg-accent" />
            <Text className="text-ink-dull text-sm">8px (2)</Text>
          </View>
          <View className="flex-row items-center gap-3">
            <View className="h-4 w-3 bg-accent" />
            <Text className="text-ink-dull text-sm">12px (3)</Text>
          </View>
          <View className="flex-row items-center gap-3">
            <View className="h-4 w-4 bg-accent" />
            <Text className="text-ink-dull text-sm">16px (4)</Text>
          </View>
          <View className="flex-row items-center gap-3">
            <View className="h-4 w-6 bg-accent" />
            <Text className="text-ink-dull text-sm">24px (6)</Text>
          </View>
          <View className="flex-row items-center gap-3">
            <View className="h-4 w-8 bg-accent" />
            <Text className="text-ink-dull text-sm">32px (8)</Text>
          </View>
        </View>
      </View>

      {/* Border Radius Section */}
      <View className="mb-6">
        <Text className="mb-3 font-semibold text-ink">Border Radius</Text>

        <View className="space-y-3">
          <View className="flex-row items-center gap-3">
            <View className="h-12 w-12 rounded-sm bg-accent" />
            <Text className="text-ink">Small (2px)</Text>
          </View>
          <View className="flex-row items-center gap-3">
            <View className="h-12 w-12 rounded-md bg-accent" />
            <Text className="text-ink">Medium (6px)</Text>
          </View>
          <View className="flex-row items-center gap-3">
            <View className="h-12 w-12 rounded-lg bg-accent" />
            <Text className="text-ink">Large (8px)</Text>
          </View>
          <View className="flex-row items-center gap-3">
            <View className="h-12 w-12 rounded-xl bg-accent" />
            <Text className="text-ink">XL (12px)</Text>
          </View>
          <View className="flex-row items-center gap-3">
            <View className="h-12 w-12 rounded-full bg-accent" />
            <Text className="text-ink">Full (9999px)</Text>
          </View>
        </View>
      </View>

      {/* Interactive Demo */}
      <View className="mb-6">
        <Text className="mb-3 font-semibold text-ink">Interactive Demo</Text>

        <View className="space-y-3">
          <Pressable className="rounded-lg bg-app-box p-4 active:bg-app-hover">
            <Text className="text-ink">Pressable Card</Text>
            <Text className="text-ink-dull text-sm">
              Tap to see active state
            </Text>
          </Pressable>

          <Pressable className="rounded-lg bg-accent p-4 active:bg-accent-deep">
            <Text className="font-medium text-white">Accent Pressable</Text>
          </Pressable>
        </View>
      </View>

      {/* Settings Primitives Section */}
      <View className="mb-6">
        <Text className="mb-3 font-semibold text-ink">iOS Settings Style</Text>

        <SettingsGroup header="Account">
          <SettingsLink
            description="View and edit your profile"
            icon={<View className="h-6 w-6 rounded-full bg-accent" />}
            label="Profile"
            onPress={() => console.log("Profile")}
          />
          <SettingsLink
            icon={<View className="h-6 w-6 rounded-full bg-green-500" />}
            label="Security"
            onPress={() => console.log("Security")}
          />
          <SettingsToggle
            description="Push notifications for this library"
            icon={<View className="h-6 w-6 rounded-full bg-orange-500" />}
            label="Notifications"
            onValueChange={setNotificationsEnabled}
            value={notificationsEnabled}
          />
        </SettingsGroup>

        <SettingsGroup
          footer="Dark mode will be applied across all libraries"
          header="Appearance"
        >
          <SettingsToggle
            icon={<View className="h-6 w-6 rounded-full bg-purple-500" />}
            label="Dark Mode"
            onValueChange={setDarkModeEnabled}
            value={darkModeEnabled}
          />
          <SettingsOption
            icon={<View className="h-6 w-6 rounded-full bg-blue-500" />}
            label="Theme"
            onPress={() => console.log("Theme picker")}
            value="System"
          />
        </SettingsGroup>

        <SettingsGroup header="Storage">
          <SettingsSlider
            description="Maximum cache size in GB"
            icon={<View className="h-6 w-6 rounded-full bg-red-500" />}
            label="Cache Size"
            maximumValue={100}
            minimumValue={10}
            onValueChange={setSliderValue}
            value={sliderValue}
          />
          <SettingsLink
            icon={<View className="h-6 w-6 rounded-full bg-yellow-500" />}
            label="Clear Cache"
            onPress={() => console.log("Clear cache")}
          />
          <SettingsLink
            description="Permanently delete all libraries and settings"
            icon={<View className="h-6 w-6 rounded-full bg-red-600" />}
            label="Reset All Data"
            onPress={handleResetData}
          />
        </SettingsGroup>
      </View>

      {/* Footer */}
      <View className="items-center py-6">
        <Text className="text-ink-faint text-sm">Spacedrive Mobile v2</Text>
        <Text className="mt-1 text-ink-faint text-xs">
          UI Primitives Showcase
        </Text>
      </View>
    </ScrollView>
  );
}
