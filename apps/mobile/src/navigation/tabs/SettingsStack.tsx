import { CompositeScreenProps } from '@react-navigation/native';
// import KeysSettingsScreen from '~/screens/settings/library/KeysSettings';

import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import { useSharedValue } from 'react-native-reanimated';
import Header from '~/components/header/Header';
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
	const scrollY = useSharedValue(0);
	return (
		<Stack.Navigator initialRouteName="Settings">
			<Stack.Screen
				name="Settings"
				options={{
					header: () => (
						<Header scrollY={scrollY} showSearch showDrawer title="Settings" />
					)
				}}
			>
				{(props) => <SettingsScreen {...props} scrollY={scrollY} />}
			</Stack.Screen>
			{/* Client */}
			<Stack.Screen
				name="GeneralSettings"
				options={{ header: () => <Header navBack title="General" /> }}
			>
				{() => <GeneralSettingsScreen />}
			</Stack.Screen>
			<Stack.Screen
				name="LibrarySettings"
				options={{ header: () => <Header scrollY={scrollY} navBack title="Libraries" /> }}
			>
				{(props) => <LibrarySettingsScreen {...props} scrollY={scrollY} />}
			</Stack.Screen>
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
				options={{
					header: () => <Header navBack title="Library Settings" />
				}}
			>
				{() => <LibraryGeneralSettingsScreen />}
			</Stack.Screen>
			<Stack.Screen
				name="LocationSettings"
				options={{
					header: () => (
						<Header scrollY={scrollY} searchType="location" navBack title="Locations" />
					)
				}}
			>
				{() => <LocationsScreen scrollY={scrollY} />}
			</Stack.Screen>
			<Stack.Screen
				name="EditLocationSettings"
				options={{
					header: () => <Header scrollY={scrollY} navBack title="Edit Location" />
				}}
			>
				{(props) => <EditLocationSettingsScreen {...props} scrollY={scrollY} />}
			</Stack.Screen>
			<Stack.Screen
				name="NodesSettings"
				component={NodesSettingsScreen}
				options={{ header: () => <Header scrollY={scrollY} navBack title="Nodes" /> }}
			/>
			<Stack.Screen
				name="TagsSettings"
				options={{ header: () => <Header scrollY={scrollY} navBack title="Tags" /> }}
			>
				{() => <TagsSettingsScreen scrollY={scrollY} />}
			</Stack.Screen>
			{/* <Stack.Screen
				name="KeysSettings"
				component={KeysSettingsScreen}
				options={{ headerTitle: 'Keys' }}
			/> */}
			{/* Info */}
			<Stack.Screen
				name="About"
				options={{ header: () => <Header scrollY={scrollY} navBack title="About" /> }}
			>
				{() => <AboutScreen scrollY={scrollY} />}
			</Stack.Screen>
			<Stack.Screen
				name="Support"
				component={SupportScreen}
				options={{ header: () => <Header scrollY={scrollY} navBack title="Support" /> }}
			/>
			<Stack.Screen
				name="Debug"
				component={DebugScreen}
				options={{ header: () => <Header scrollY={scrollY} navBack title="Debug" /> }}
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
