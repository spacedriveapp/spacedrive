import type { NavigatorScreenParams } from "@react-navigation/native";

// Root stack contains main app and onboarding
export type RootStackParamList = {
  Main: NavigatorScreenParams<DrawerParamList>;
  Onboarding: undefined;
  Search: undefined;
};

// Drawer contains the sidebar and tab navigator
export type DrawerParamList = {
  Tabs: NavigatorScreenParams<TabParamList>;
};

// Bottom tabs
export type TabParamList = {
  OverviewTab: NavigatorScreenParams<OverviewStackParamList>;
  BrowseTab: NavigatorScreenParams<BrowseStackParamList>;
  NetworkTab: NavigatorScreenParams<NetworkStackParamList>;
  SettingsTab: NavigatorScreenParams<SettingsStackParamList>;
};

// Overview stack
export type OverviewStackParamList = {
  Overview: undefined;
};

// Browse stack
export type BrowseStackParamList = {
  BrowseHome: undefined;
  Location: { locationId: string; name?: string };
  Tag: { tagId: string; name?: string };
  Explorer: { path: string; locationId?: string };
};

// Network stack
export type NetworkStackParamList = {
  Network: undefined;
  Peers: undefined;
  Pairing: undefined;
};

// Settings stack
export type SettingsStackParamList = {
  SettingsHome: undefined;
  GeneralSettings: undefined;
  LibrarySettings: undefined;
  AppearanceSettings: undefined;
  PrivacySettings: undefined;
  About: undefined;
};

// Utility types for typed navigation
declare global {
  namespace ReactNavigation {
    interface RootParamList extends RootStackParamList {}
  }
}
