import { Platform, Text } from 'react-native';
import { Tabs } from 'expo-router';
import { NativeTabs, Icon, Label } from 'expo-router/unstable-native-tabs';
import Ionicons from '@expo/vector-icons/Ionicons';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import Animated, {
  useAnimatedStyle,
  withTiming,
  Easing,
} from 'react-native-reanimated';

// Brand colors matching the app theme
const colors = {
  tabBarBackground: 'hsl(235, 15%, 13%)',
  tabBarBorder: 'hsl(235, 15%, 23%)',
  active: 'hsl(208, 100%, 57%)',
  inactive: 'hsl(235, 10%, 55%)',
  // M3 active indicator uses primary color at low opacity
  activeIndicator: 'hsla(208, 100%, 57%, 0.15)',
};

// Animation config for smooth M3-style transitions
const timingConfig = {
  duration: 200,
  easing: Easing.out(Easing.cubic),
};

// M3 Active Indicator wrapper for tab icons with animations
function TabIcon({
  name,
  focusedName,
  focused,
  color,
}: {
  name: keyof typeof Ionicons.glyphMap;
  focusedName: keyof typeof Ionicons.glyphMap;
  focused: boolean;
  color: string;
}) {
  const animatedContainerStyle = useAnimatedStyle(() => ({
    backgroundColor: withTiming(
      focused ? colors.activeIndicator : 'transparent',
      timingConfig
    ),
    marginBottom: withTiming(focused ? 6 : 2, timingConfig),
    transform: [
      { scale: withTiming(focused ? 1 : 0.95, timingConfig) },
    ],
  }));

  return (
    <Animated.View
      style={[
        {
          width: 64,
          height: 32,
          borderRadius: 16,
          alignItems: 'center',
          justifyContent: 'center',
        },
        animatedContainerStyle,
      ]}
    >
      <Ionicons name={focused ? focusedName : name} size={24} color={color} />
    </Animated.View>
  );
}

// Tab label with animated spacing
function TabLabel({
  label,
  focused,
  color,
}: {
  label: string;
  focused: boolean;
  color: string;
}) {
  const animatedStyle = useAnimatedStyle(() => ({
    marginTop: withTiming(focused ? 2 : 0, timingConfig),
    opacity: withTiming(focused ? 1 : 0.8, timingConfig),
  }));

  return (
    <Animated.Text
      style={[
        {
          color,
          fontSize: 12,
          fontWeight: '500',
        },
        animatedStyle,
      ]}
    >
      {label}
    </Animated.Text>
  );
}

function IOSTabs() {
  return (
    <NativeTabs
      // CRITICAL: null background enables liquid glass on iOS!
      backgroundColor={null}
      disableTransparentOnScrollEdge={true}
    >
      <NativeTabs.Trigger name="overview">
        <Label>Overview</Label>
        <Icon sf="square.grid.2x2" />
      </NativeTabs.Trigger>

      <NativeTabs.Trigger name="browse">
        <Label>Browse</Label>
        <Icon sf="folder" />
      </NativeTabs.Trigger>

      <NativeTabs.Trigger name="settings">
        <Label>Settings</Label>
        <Icon sf="gearshape" />
      </NativeTabs.Trigger>
    </NativeTabs>
  );
}

function AndroidTabs() {
  const insets = useSafeAreaInsets();

  return (
    <Tabs
      screenOptions={{
        tabBarStyle: {
          backgroundColor: colors.tabBarBackground,
          borderTopColor: colors.tabBarBorder,
          borderTopWidth: 1,
          height: 80 + insets.bottom,
          paddingBottom: insets.bottom,
        },
        tabBarItemStyle: {
          paddingTop: 12,
          paddingBottom: 12,
          justifyContent: 'center',
        },
        tabBarIconStyle: {},
        tabBarActiveTintColor: colors.active,
        tabBarInactiveTintColor: colors.inactive,
        headerShown: false,
      }}
    >
      <Tabs.Screen
        name="overview"
        options={{
          title: 'Overview',
          tabBarIcon: ({ color, focused }) => (
            <TabIcon
              name="grid-outline"
              focusedName="grid"
              focused={focused}
              color={color}
            />
          ),
          tabBarLabel: ({ color, focused }) => (
            <TabLabel label="Overview" focused={focused} color={color} />
          ),
        }}
      />
      <Tabs.Screen
        name="browse"
        options={{
          title: 'Browse',
          tabBarIcon: ({ color, focused }) => (
            <TabIcon
              name="folder-outline"
              focusedName="folder"
              focused={focused}
              color={color}
            />
          ),
          tabBarLabel: ({ color, focused }) => (
            <TabLabel label="Browse" focused={focused} color={color} />
          ),
        }}
      />
      <Tabs.Screen
        name="settings"
        options={{
          title: 'Settings',
          tabBarIcon: ({ color, focused }) => (
            <TabIcon
              name="settings-outline"
              focusedName="settings"
              focused={focused}
              color={color}
            />
          ),
          tabBarLabel: ({ color, focused }) => (
            <TabLabel label="Settings" focused={focused} color={color} />
          ),
        }}
      />
    </Tabs>
  );
}

export default function TabLayout() {
  return Platform.OS === 'ios' ? <IOSTabs /> : <AndroidTabs />;
}
