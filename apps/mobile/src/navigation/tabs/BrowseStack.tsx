import { CompositeScreenProps } from '@react-navigation/native';
import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import BrowseScreen from '~/screens/browse/Browse';
import LibraryScreen from '~/screens/browse/Library';
import LocationScreen from '~/screens/browse/Location';
import LocationsScreen from '~/screens/browse/Locations';
import TagScreen from '~/screens/browse/Tag';
import TagsScreen from '~/screens/browse/Tags';

import { TabScreenProps } from '../TabNavigator';

const Stack = createNativeStackNavigator<BrowseStackParamList>();

export default function BrowseStack() {
	return (
		<Stack.Navigator
			screenOptions={{
				headerShown: false
			}}
		initialRouteName="Browse">
			<Stack.Screen
				name="Browse"
				component={BrowseScreen}
			/>
			<Stack.Screen
				name="Location"
			>
				{(props) => <LocationScreen {...props}/>}
			</Stack.Screen>
			<Stack.Screen
				name="Tags"
				component={TagsScreen}
			/>
			<Stack.Screen
				name="Locations"
				component={LocationsScreen}/>
			<Stack.Screen
				name="Tag"
			>
				{(props) => <TagScreen {...props} />}
			</Stack.Screen>
			<Stack.Screen
				name="Library"
				component={LibraryScreen}
			/>
		</Stack.Navigator>
	);
}

export type BrowseStackParamList = {
	Browse: undefined;
	Location: { id: number; path?: string, title?: string | null };
	Locations: undefined;
	Tag: { id: number; color: string, title?: string | null };
	Tags: undefined;
	Library: undefined;
};

export type BrowseStackScreenProps<Screen extends keyof BrowseStackParamList> =
	CompositeScreenProps<
		NativeStackScreenProps<BrowseStackParamList, Screen>,
		TabScreenProps<'BrowseStack'>
	>;
