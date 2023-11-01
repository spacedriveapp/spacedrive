import { createStackNavigator, StackScreenProps } from '@react-navigation/stack';
import { ArrowLeft } from 'phosphor-react-native';
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
				options={{
					headerTitle: 'Settings',
					headerBackImage: () => (
						<ArrowLeft size={23} color={tw.color('ink')} style={tw`ml-2`} />
					)
				}}
			/>
			{/* Client */}
			<SettingsStack.Screen
				name="GeneralSettings"
				component={GeneralSettingsScreen}
				options={{
					headerTitle: 'General Settings',
					headerBackImage: () => (
						<ArrowLeft size={23} color={tw.color('ink')} style={tw`ml-2`} />
					)
				}}
			/>
			<SettingsStack.Screen
				name="LibrarySettings"
				component={LibrarySettingsScreen}
				options={{
					headerTitle: 'Libraries',
					headerBackImage: () => (
						<ArrowLeft size={23} color={tw.color('ink')} style={tw`ml-2`} />
					)
				}}
			/>
			<SettingsStack.Screen
				name="AppearanceSettings"
				component={AppearanceSettingsScreen}
				options={{
					headerTitle: 'Appearance',
					headerBackImage: () => (
						<ArrowLeft size={23} color={tw.color('ink')} style={tw`ml-2`} />
					)
				}}
			/>
			<SettingsStack.Screen
				name="PrivacySettings"
				component={PrivacySettingsScreen}
				options={{
					headerTitle: 'Privacy',
					headerBackImage: () => (
						<ArrowLeft size={23} color={tw.color('ink')} style={tw`ml-2`} />
					)
				}}
			/>
			<SettingsStack.Screen
				name="ExtensionsSettings"
				component={ExtensionsSettingsScreen}
				options={{
					headerTitle: 'Extensions',
					headerBackImage: () => (
						<ArrowLeft size={23} color={tw.color('ink')} style={tw`ml-2`} />
					)
				}}
			/>
			{/* Library */}
			<SettingsStack.Screen
				name="LibraryGeneralSettings"
				component={LibraryGeneralSettingsScreen}
				options={{
					headerTitle: 'Library Settings',
					headerBackImage: () => (
						<ArrowLeft size={23} color={tw.color('ink')} style={tw`ml-2`} />
					)
				}}
			/>
			<SettingsStack.Screen
				name="LocationSettings"
				component={LocationSettingsScreen}
				options={{
					headerTitle: 'Locations',
					headerBackImage: () => (
						<ArrowLeft size={23} color={tw.color('ink')} style={tw`ml-2`} />
					)
				}}
			/>
			<SettingsStack.Screen
				name="EditLocationSettings"
				component={EditLocationSettingsScreen}
				options={{
					headerTitle: 'Edit Location',
					headerBackImage: () => (
						<ArrowLeft size={23} color={tw.color('ink')} style={tw`ml-2`} />
					)
				}}
			/>
			<SettingsStack.Screen
				name="NodesSettings"
				component={NodesSettingsScreen}
				options={{
					headerTitle: 'Nodes',
					headerBackImage: () => (
						<ArrowLeft size={23} color={tw.color('ink')} style={tw`ml-2`} />
					)
				}}
			/>
			<SettingsStack.Screen
				name="TagsSettings"
				component={TagsSettingsScreen}
				options={{
					headerTitle: 'Tags',
					headerBackImage: () => (
						<ArrowLeft size={23} color={tw.color('ink')} style={tw`ml-2`} />
					)
				}}
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
				options={{
					headerTitle: 'About',
					headerBackImage: () => (
						<ArrowLeft size={23} color={tw.color('ink')} style={tw`ml-2`} />
					)
				}}
			/>
			<SettingsStack.Screen
				name="Support"
				component={SupportScreen}
				options={{
					headerTitle: 'Support',
					headerBackImage: () => (
						<ArrowLeft size={23} color={tw.color('ink')} style={tw`ml-2`} />
					)
				}}
			/>
			<SettingsStack.Screen
				name="Debug"
				component={DebugScreen}
				options={{
					headerTitle: 'Debug',
					headerBackImage: () => (
						<ArrowLeft size={23} color={tw.color('ink')} style={tw`ml-2`} />
					)
				}}
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
