import { CompositeScreenProps } from '@react-navigation/native';
// import KeysSettingsScreen from '~/screens/settings/library/KeysSettings';

import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import Header from '~/components/header/Header';
import SearchHeader from '~/components/header/SearchHeader';
import AccountLogin from '~/screens/settings/client/AccountSettings/AccountLogin';
import AccountProfile from '~/screens/settings/client/AccountSettings/AccountProfile';
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
import LocationSettingsScreen from '~/screens/settings/library/LocationSettings';
import NodesSettingsScreen from '~/screens/settings/library/NodesSettings';
import TagsSettingsScreen from '~/screens/settings/library/TagsSettings';
import SettingsScreen from '~/screens/settings/Settings';

import { TabScreenProps } from '../TabNavigator';

const Stack = createNativeStackNavigator<SettingsStackParamList>();

export default function SettingsStack() {
	return (
		<Stack.Navigator
			screenOptions={{
				fullScreenGestureEnabled: true
			}}
			initialRouteName="Settings"
		>
			<Stack.Screen
				name="Settings"
				component={SettingsScreen}
				options={({ route }) => ({
					header: () => <Header search route={route} />
				})}
			/>
			{/* Client */}
			<Stack.Screen
				name="GeneralSettings"
				component={GeneralSettingsScreen}
				options={{ header: () => <Header navBack title="General" /> }}
			/>
			<Stack.Screen
				name="AccountLogin"
				component={AccountLogin}
				options={{ header: () => <Header navBackTo="Settings" navBack title="Account" /> }}
			/>
			<Stack.Screen
				name="AccountProfile"
				component={AccountProfile}
				options={{ header: () => <Header navBackTo="Settings" navBack title="Account" /> }}
			/>
			<Stack.Screen
				name="LibrarySettings"
				component={LibrarySettingsScreen}
				options={{ header: () => <Header navBack title="Libraries" /> }}
			/>
			<Stack.Screen
				name="AppearanceSettings"
				component={AppearanceSettingsScreen}
				options={{ header: () => <Header navBack title="Appearance" /> }}
			/>
			<Stack.Screen
				name="PrivacySettings"
				component={PrivacySettingsScreen}
				options={{ header: () => <Header navBack title="Privacy" /> }}
			/>
			<Stack.Screen
				name="ExtensionsSettings"
				component={ExtensionsSettingsScreen}
				options={{ header: () => <Header navBack title="Extensions" /> }}
			/>
			{/* Library */}
			<Stack.Screen
				name="LibraryGeneralSettings"
				component={LibraryGeneralSettingsScreen}
				options={{ header: () => <Header navBack title="Library Settings" /> }}
			/>
			<Stack.Screen
				name="LocationSettings"
				component={LocationSettingsScreen}
				options={() => ({
					header: () => <SearchHeader title="Locations" kind="locations" />
				})}
			/>
			<Stack.Screen
				name="EditLocationSettings"
				component={EditLocationSettingsScreen}
				options={{ header: () => <Header navBack title="Edit Location" /> }}
			/>
			<Stack.Screen
				name="NodesSettings"
				component={NodesSettingsScreen}
				options={{ header: () => <Header navBack title="Nodes" /> }}
			/>
			<Stack.Screen
				name="TagsSettings"
				component={TagsSettingsScreen}
				options={{ header: () => <Header navBack title="Tags" /> }}
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
				options={{ header: () => <Header navBack title="About" /> }}
			/>
			<Stack.Screen
				name="Support"
				component={SupportScreen}
				options={{ header: () => <Header navBack title="Support" /> }}
			/>
			<Stack.Screen
				name="Debug"
				component={DebugScreen}
				options={{ header: () => <Header navBack title="Debug" /> }}
			/>
		</Stack.Navigator>
	);
}

export type SettingsStackParamList = {
	// Home screen for the Settings stack.
	Settings: undefined;
	// Client
	GeneralSettings: undefined;
	AccountLogin: undefined;
	AccountProfile: undefined;
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
	SyncSettings: undefined;
	CloudSettings: undefined;
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
