import { createStackNavigator, StackScreenProps } from '@react-navigation/stack';

import { tw } from '~/lib/tailwind';
import AppearanceSettingsScreen from '~/screens/settings/client/AppearanceSettings';
import ExtensionsSettingsScreen from '~/screens/settings/client/ExtensionsSettings';
import GeneralSettingsScreen from '~/screens/settings/client/GeneralSettings';
import LibrarySettingsScreen from '~/screens/settings/client/LibrarySettings';
import PrivacySettingsScreen from '~/screens/settings/client/PrivacySettings';
import AboutScreen from '~/screens/settings/info/About';
import DebugScreen from '~/screens/settings/info/Debug';
import SupportScreen from '~/screens/settings/info/Support';
import EditLocationSettingsScreen from '~/screens/settings/library/EditLocationSettings';
// import KeysSettingsScreen from '~/screens/settings/library/KeysSettings';
import LibraryGeneralSettingsScreen from '~/screens/settings/library/LibraryGeneralSettings';
import LocationSettingsScreen from '~/screens/settings/library/LocationSettings';
import NodesSettingsScreen from '~/screens/settings/library/NodesSettings';
import TagsSettingsScreen from '~/screens/settings/library/TagsSettings';
import SettingsScreen from '~/screens/settings/Settings';

const SettingsStack = createStackNavigator<SettingsStackParamList>();

export default function SettingsNavigator() {
	return (
		<SettingsStack.Navigator
			id="settings"
			initialRouteName="Home"
			screenOptions={{
				headerBackTitleVisible: false,
				headerStyle: tw`bg-app`,
				headerTintColor: tw.color('ink'),
				headerTitleStyle: tw`text-base`,
				headerBackTitleStyle: tw`text-base`
				// headerShadowVisible: false // will disable the white line under
			}}
		>
			<SettingsStack.Screen
				name="Home"
				component={SettingsScreen}
				options={{ headerTitle: 'Settings' }}
			/>
			{/* Client */}
			<SettingsStack.Screen
				name="GeneralSettings"
				component={GeneralSettingsScreen}
				options={{ headerTitle: 'General Settings' }}
			/>
			<SettingsStack.Screen
				name="LibrarySettings"
				component={LibrarySettingsScreen}
				options={{ headerTitle: 'Libraries' }}
			/>
			<SettingsStack.Screen
				name="AppearanceSettings"
				component={AppearanceSettingsScreen}
				options={{ headerTitle: 'Appearance' }}
			/>
			<SettingsStack.Screen
				name="PrivacySettings"
				component={PrivacySettingsScreen}
				options={{ headerTitle: 'Privacy' }}
			/>
			<SettingsStack.Screen
				name="ExtensionsSettings"
				component={ExtensionsSettingsScreen}
				options={{ headerTitle: 'Extensions' }}
			/>
			{/* Library */}
			<SettingsStack.Screen
				name="LibraryGeneralSettings"
				component={LibraryGeneralSettingsScreen}
				options={{ headerTitle: 'Library Settings' }}
			/>
			<SettingsStack.Screen
				name="LocationSettings"
				component={LocationSettingsScreen}
				options={{ headerTitle: 'Locations' }}
			/>
			<SettingsStack.Screen
				name="EditLocationSettings"
				component={EditLocationSettingsScreen}
				options={{ headerTitle: 'Edit Location' }}
			/>
			<SettingsStack.Screen
				name="NodesSettings"
				component={NodesSettingsScreen}
				options={{ headerTitle: 'Nodes' }}
			/>
			<SettingsStack.Screen
				name="TagsSettings"
				component={TagsSettingsScreen}
				options={{ headerTitle: 'Tags' }}
			/>
			{/* <SettingsStack.Screen
				name="KeysSettings"
				component={KeysSettingsScreen}
				options={{ headerTitle: 'Keys' }}
			/> */}
			{/* Info */}
			<SettingsStack.Screen
				name="About"
				component={AboutScreen}
				options={{ headerTitle: 'About' }}
			/>
			<SettingsStack.Screen
				name="Support"
				component={SupportScreen}
				options={{ headerTitle: 'Support' }}
			/>
			<SettingsStack.Screen
				name="Debug"
				component={DebugScreen}
				options={{ headerTitle: 'Debug' }}
			/>
		</SettingsStack.Navigator>
	);
}

export type SettingsStackParamList = {
	// Home screen for the Settings stack.
	Home: undefined;
	// Client
	GeneralSettings: undefined;
	LibrarySettings: undefined;
	AppearanceSettings: undefined;
	PrivacySettings: undefined;
	ExtensionsSettings: undefined;
	// Library
	LibraryGeneralSettings: undefined;

	// Location
	LocationSettings: undefined;
	EditLocationSettings: { id: number };

	NodesSettings: undefined;
	TagsSettings: undefined;
	KeysSettings: undefined;
	// Info
	About: undefined;
	Support: undefined;
	Debug: undefined;
};

export type SettingsStackScreenProps<Screen extends keyof SettingsStackParamList> =
	StackScreenProps<SettingsStackParamList, Screen>;
