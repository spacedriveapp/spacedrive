import { CompositeScreenProps } from '@react-navigation/native';
import { createNativeStackNavigator, NativeStackScreenProps } from '@react-navigation/native-stack';
import Header from '~/components/header/Header';
import BrowseScreen from '~/screens/browse';
import LibraryScreen from '~/screens/browse/Library';
import LocationsScreen from '~/screens/browse/Locations';
import TagsScreen from '~/screens/browse/Tags';
import LocationScreen from '~/screens/Location';
import TagScreen from '~/screens/Tag';

import { TabScreenProps } from '../TabNavigator';

const Stack = createNativeStackNavigator<BrowseStackParamList>();

export default function BrowseStack() {
	return (
		<Stack.Navigator initialRouteName="Browse">
			<Stack.Screen
				name="Browse"
				component={BrowseScreen}
				options={{ header: () => <Header showLibrary title="Browse" /> }}
			/>
			<Stack.Screen
				name="Location"
				component={LocationScreen}
				options={{
					header: (route) => (
						<Header route={route} headerKind="location" routeTitle navBack />
					)
				}}
			/>
			<Stack.Screen
				name="Tags"
				component={TagsScreen}
				options={{
					header: () => <Header navBack title="Tags" />
				}}
			/>
			<Stack.Screen
				name="Locations"
				component={LocationsScreen}
				options={{
					header: () => <Header navBack searchType="location" title="Locations" />
				}}
			/>
			<Stack.Screen
				name="Tag"
				component={TagScreen}
				options={{
					header: (route) => <Header routeTitle route={route} headerKind="tag" navBack />
				}}
			/>
			<Stack.Screen
				name="Library"
				component={LibraryScreen}
				options={{
					header: () => <Header navBack title="Library" />
				}}
			/>
		</Stack.Navigator>
	);
}

export type BrowseStackParamList = {
	Browse: undefined;
	Location: { id: number; path?: string };
	Locations: undefined;
	Tag: { id: number; color: string };
	Tags: undefined;
	Library: undefined;
};

export type BrowseStackScreenProps<Screen extends keyof BrowseStackParamList> =
	CompositeScreenProps<
		NativeStackScreenProps<BrowseStackParamList, Screen>,
		TabScreenProps<'BrowseStack'>
	>;
