import { NavigatorScreenParams } from "@react-navigation/native";

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
	Explorer:
		| { type: "path"; path: string } // JSON.stringify(SdPath)
		| { type: "view"; view: string; id?: string }; // Virtual views
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
