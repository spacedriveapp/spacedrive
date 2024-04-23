import { CompositeScreenProps } from '@react-navigation/native';
// import KeysSettingsScreen from '~/screens/settings/library/KeysSettings';

import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import LocationsScreen from '~/screens/browse/Locations';
import AppearanceSettingsScreen from '~/screens/settings/client/AppearanceSettings';
import ExtensionsSettingsScreen from '~/screens/settings/client/ExtensionsSettings';
import GeneralSettingsScreen from '~/screens/settings/client/GeneralSettings';
import LibrarySettingsScreen from '~/screens/settings/client/LibrarySettings';
import PrivacySettingsScreen from '~/screens/settings/client/PrivacySettings';
import AboutScreen from '~/screens/settings/info/About';
import DebugScreen from '~/screens/settings/info/Debug';
import SupportScreen from '~/screens/settings/info/Support';
import EditLocationSettingsScreen from '~/screens/settings/library/EditLocationSettings';
import LibraryGeneralSettingsScreen from '~/screens/settings/library/LibraryGeneralSettings';
import NodesSettingsScreen from '~/screens/settings/library/NodesSettings';
import TagsSettingsScreen from '~/screens/settings/library/TagsSettings';
import SettingsScreen from '~/screens/settings/Settings';

import { TabScreenProps } from '../TabNavigator';

const Stack = createNativeStackNavigator<SettingsStackParamList>();

export default function SettingsStack() {
	return (
		<Stack.Navigator
		screenOptions={{
			headerShown: false
		}}
		initialRouteName="Settings">
			<Stack.Screen
				name="Settings"
			>
				{(props) => <SettingsScreen {...props}/>}
			</Stack.Screen>
			{/* Client */}
			<Stack.Screen
				name="GeneralSettings"
				component={GeneralSettingsScreen}
			/>
			<Stack.Screen
				name="LibrarySettings"
			>
				{(props) => <LibrarySettingsScreen {...props} />}
			</Stack.Screen>
			<Stack.Screen
				name="AppearanceSettings"
				component={AppearanceSettingsScreen}
			/>
			<Stack.Screen
				name="PrivacySettings"
				component={PrivacySettingsScreen}
			/>
			<Stack.Screen
				name="ExtensionsSettings"
				component={ExtensionsSettingsScreen}
			/>
			{/* Library */}
			<Stack.Screen
				name="LibraryGeneralSettings"
				component={LibraryGeneralSettingsScreen}
			/>
			<Stack.Screen
				name="LocationSettings"
				component={LocationsScreen}
			/>
			<Stack.Screen
				name="EditLocationSettings"
			>
				{(props) => <EditLocationSettingsScreen {...props} />}
			</Stack.Screen>
			<Stack.Screen
				name="NodesSettings"
				component={NodesSettingsScreen}
			/>
			<Stack.Screen
				name="TagsSettings"
				component={TagsSettingsScreen}
			/>
			{/* <Stack.Screen
				name="KeysSettings"
				component={KeysSettingsScreen}
				options={{ headerTitle: 'Keys' }}
			/> */}
			{/* Info */}
			<Stack.Screen
				name="About"
				component={AboutScreen}
		/>
			<Stack.Screen
				name="Support"
				component={SupportScreen}
			/>
			<Stack.Screen
				name="Debug"
				component={DebugScreen}
			/>
		</Stack.Navigator>
	);
}

export type SettingsStackParamList = {
	// Home screen for the Settings stack.
	Settings: undefined;
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
	CompositeScreenProps<
		NativeStackScreenProps<SettingsStackParamList, Screen>,
		TabScreenProps<'SettingsStack'>
	>;
